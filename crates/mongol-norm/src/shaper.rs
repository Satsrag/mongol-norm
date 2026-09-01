//! The shaper: data indexes, written-unit resolution, the context helpers the rules use, and the
//! public shaping entry points (`shape`, `same_shape`, `shape_detailed`, `trace`).
#![allow(dead_code)] // TEMPORARY — removed in Task 6 when the rules use the helpers.

use std::collections::{HashMap, HashSet};

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::generated::{mch, mng, sib, tod};
use crate::rules::{self, Rule};
use crate::tables::{Fvs, Letter, Locale, LocaleData, ParticleSym, Position, Variant};
use crate::token::{assign_positions, tokenize, Token, TokenKind};
use crate::unicode::check_word_chars;
use crate::Error;

/// Per-token breakdown returned by [`Shaper::shape_detailed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenDetail {
    /// The token's code point (MVS for NNBSP input).
    pub cp: char,
    /// The letter's alias in this locale, if any (`None` for structural tokens).
    pub alias: Option<Alias>,
    /// Structural position (`Isol` for structural tokens).
    pub position: Position,
    /// The first FVS attached to the letter (Python reports only the first one).
    pub fvs: Option<Fvs>,
    /// The condition assigned by the rule pipeline.
    pub condition: Option<Condition>,
    /// The resolved written units (empty for structural tokens).
    pub written: Vec<WrittenUnit>,
}

/// One condition change recorded by [`Shaper::trace`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionChange {
    /// Token index.
    pub token: usize,
    /// Condition before the rule ran.
    pub before: Option<Condition>,
    /// Condition after the rule ran.
    pub after: Option<Condition>,
}

/// The changes one rule made, in [`Shaper::trace`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleTransition {
    /// The rule name (see [`Shaper::rule_names`]).
    pub rule: &'static str,
    /// Every token whose condition changed, in token order.
    pub changes: Vec<ConditionChange>,
}

/// A per-rule trace of the shaping pipeline — the verifier for
/// `tests/golden/mng-phase-trace-v1.json`. Every vector covers *all* tokens, structural ones
/// included (position `Isol`, condition `None`, no written units).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeTrace {
    /// Position of every token.
    pub positions: Vec<Position>,
    /// Rules that changed at least one condition, in rule order.
    pub transitions: Vec<RuleTransition>,
    /// Condition of every token after all rules ran.
    pub final_conditions: Vec<Option<Condition>>,
    /// Resolved written units of every token.
    pub written_by_token: Vec<Vec<WrittenUnit>>,
    /// The flattened shape (what [`Shaper::shape`] returns).
    pub shape: Vec<WrittenUnit>,
}

/// The UTN #57 shaping engine (and, for MNG, the canonical normalizer) for one locale.
///
/// Construction is cheap (microseconds) and the value is `Send + Sync`, so one instance can be
/// shared behind a reference for the lifetime of a program.
pub struct Shaper {
    locale: Locale,
    letters: HashMap<u32, &'static Letter>,
    variants: HashMap<(u32, Position, Option<Fvs>), &'static Variant>,
    defaults: HashMap<(u32, Position), &'static Variant>,
    vowels: HashSet<Alias>,
    consonants: HashSet<Alias>,
    masculine: HashSet<Alias>,
    feminine: HashSet<Alias>,
    neuter: HashSet<Alias>,
    particles: HashMap<&'static [ParticleSym], &'static [usize]>,
    rules: &'static [Rule],
}

impl Default for Shaper {
    fn default() -> Shaper {
        Shaper::new(Locale::Mng)
    }
}

impl Shaper {
    /// Build the shaper for `locale` from the generated tables.
    pub fn new(locale: Locale) -> Shaper {
        let data: &'static LocaleData = match locale {
            Locale::Mng => &mng::DATA,
            Locale::Tod => &tod::DATA,
            Locale::Sib => &sib::DATA,
            Locale::Mch => &mch::DATA,
        };
        let mut letters = HashMap::new();
        let mut variants = HashMap::new();
        let mut defaults = HashMap::new();
        for letter in data.letters {
            letters.insert(letter.cp, letter);
            for variant in letter.variants {
                variants.insert((letter.cp, variant.position, variant.fvs), variant);
                if variant.default {
                    defaults.insert((letter.cp, variant.position), variant);
                }
            }
        }
        let set = |aliases: &'static [Alias]| aliases.iter().copied().collect::<HashSet<Alias>>();
        Shaper {
            locale,
            letters,
            variants,
            defaults,
            vowels: set(data.categories.vowel),
            consonants: set(data.categories.consonant),
            masculine: set(data.categories.vowel_masculine),
            feminine: set(data.categories.vowel_feminine),
            neuter: set(data.categories.vowel_neuter),
            particles: data
                .particles
                .iter()
                .map(|particle| (particle.key, particle.indices))
                .collect(),
            rules: rules::rules_for(locale),
        }
    }

    /// The locale this shaper was built for.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// The names of the shaping rules, in the order they run (empty for locales without rules).
    pub fn rule_names(&self) -> Vec<&'static str> {
        self.rules.iter().map(|rule| rule.name).collect()
    }

    // ── data access ─────────────────────────────────────────────────────────────────────────

    pub(crate) fn alias_of(&self, cp: u32) -> Option<Alias> {
        self.letters.get(&cp).map(|letter| letter.alias)
    }

    pub(crate) fn tokenize(&self, text: &str) -> Vec<Token> {
        tokenize(text, |cp| self.alias_of(cp))
    }

    /// Particle-dictionary lookup by exact symbol sequence.
    pub(crate) fn particle(&self, key: &[ParticleSym]) -> Option<&'static [usize]> {
        self.particles.get(key).copied()
    }

    /// Python `_get_condition_fvs`: the FVS of the first variant (in table order) at `position`
    /// whose conditions include `condition`.
    fn condition_fvs(
        &self,
        cp: u32,
        position: Position,
        condition: Condition,
    ) -> Option<Option<Fvs>> {
        self.letters
            .get(&cp)?
            .variants
            .iter()
            .find(|variant| variant.position == position && variant.conditions.contains(&condition))
            .map(|variant| variant.fvs)
    }

    /// Python `_resolve_token_written` (memoised on the token):
    /// 1. the first FVS in stream order that names an existing variant,
    /// 2. else the variant carrying the rule-assigned condition,
    /// 3. else the default variant. Structural tokens resolve to nothing.
    pub(crate) fn resolve_written(&self, token: &mut Token) {
        if token.written.is_some() {
            return;
        }
        if !token.is_letter() {
            token.written = Some(Vec::new());
            return;
        }
        let mut written: Option<&'static [WrittenUnit]> = None;
        for &fvs in &token.fvs {
            if let Some(variant) = self.variants.get(&(token.cp, token.position, Some(fvs))) {
                written = Some(variant.written);
                break;
            }
        }
        if written.is_none() {
            if let Some(condition) = token.condition {
                if let Some(fvs) = self.condition_fvs(token.cp, token.position, condition) {
                    written = self
                        .variants
                        .get(&(token.cp, token.position, fvs))
                        .map(|variant| variant.written);
                }
            }
        }
        if written.is_none() {
            written = self
                .defaults
                .get(&(token.cp, token.position))
                .map(|variant| variant.written);
        }
        token.written = Some(written.map(<[WrittenUnit]>::to_vec).unwrap_or_default());
    }

    // ── predicates used by the rules ────────────────────────────────────────────────────────

    pub(crate) fn is_vowel(&self, token: &Token) -> bool {
        token.is_letter()
            && token
                .alias
                .is_some_and(|alias| self.vowels.contains(&alias))
    }

    pub(crate) fn is_consonant(&self, token: &Token) -> bool {
        token.is_letter()
            && token
                .alias
                .is_some_and(|alias| self.consonants.contains(&alias))
    }

    pub(crate) fn is_masc_vowel(&self, token: &Token) -> bool {
        token
            .alias
            .is_some_and(|alias| self.masculine.contains(&alias))
    }

    pub(crate) fn is_fem_vowel(&self, token: &Token) -> bool {
        token
            .alias
            .is_some_and(|alias| self.feminine.contains(&alias))
    }

    pub(crate) fn is_neut_vowel(&self, token: &Token) -> bool {
        token
            .alias
            .is_some_and(|alias| self.neuter.contains(&alias))
    }

    /// Python `_masc_marker_reaches_g_h`: would mongfontbuilder's MASC marker sit immediately
    /// after the g/h at `idx` after the full preprocessing chain? Forward scan
    /// (preprocessing.A/B/C) then backward scan (preprocessing.G/H/J/K). Nirugu is transparent,
    /// MVS blocks.
    pub(crate) fn masc_marker_reaches_g_h(&self, tokens: &[Token], idx: usize) -> bool {
        // ── Forward (preprocessing.A/B/C) ──
        let mut j = idx;
        while j > 0 {
            j -= 1;
            let token = &tokens[j];
            if !token.is_letter() {
                if token.is_nirugu() {
                    continue;
                }
                break; // mvs blocks; fall through to the backward check
            }
            if self.is_fem_vowel(token) {
                break;
            }
            if self.is_masc_vowel(token)
                && matches!(token.position, Position::Init | Position::Medi)
            {
                return true;
            }
        }

        // ── Backward (preprocessing.G/H/J/K) ──
        if !matches!(tokens[idx].position, Position::Init | Position::Medi) {
            return false;
        }
        // A fem vowel earlier in the word blocks the backward chain.
        let mut j = idx;
        while j > 0 {
            j -= 1;
            let token = &tokens[j];
            if !token.is_letter() {
                if token.is_nirugu() {
                    continue;
                }
                break;
            }
            if self.is_fem_vowel(token) {
                return false;
            }
        }
        // Walk forward through an unbroken chain of non-fem init/medi letters, terminating at
        // a masc vowel or at `fina letter + mvs + isol a`.
        let mut j = idx + 1;
        while j < tokens.len() {
            let next = &tokens[j];
            if !next.is_letter() {
                if next.is_nirugu() {
                    j += 1;
                    continue;
                }
                return false;
            }
            if self.is_masc_vowel(next) {
                return true;
            }
            if self.is_fem_vowel(next) {
                return false;
            }
            if matches!(next.position, Position::Init | Position::Medi) {
                j += 1;
                continue;
            }
            let mut k = j + 1;
            while k < tokens.len() && tokens[k].is_nirugu() {
                k += 1;
            }
            return next.position == Position::Fina
                && k < tokens.len()
                && tokens[k].is_mvs()
                && k + 1 < tokens.len()
                && tokens[k + 1].is_letter()
                && tokens[k + 1].alias == Some(Alias::A)
                && tokens[k + 1].position == Position::Isol;
        }
        false
    }

    // ── shaping ─────────────────────────────────────────────────────────────────────────────

    fn run_pipeline(&self, text: &str) -> Result<Vec<Token>, Error> {
        check_word_chars(text)?;
        let mut tokens = self.tokenize(text);
        assign_positions(&mut tokens);
        rules::run_rules(self.rules, &mut tokens, self);
        for token in &mut tokens {
            self.resolve_written(token);
        }
        Ok(tokens)
    }

    /// Shape `text` into its written-unit sequence. Structural characters appear verbatim as
    /// [`WrittenUnit::Mvs`], [`WrittenUnit::Nirugu`] and [`WrittenUnit::Zwj`].
    ///
    /// Errors with [`Error::NonMongolianChar`] on anything but Mongolian letters, FVS, MVS,
    /// NNBSP, nirugu and ZWJ — use [`Shaper::normalize_text`] for mixed-script text.
    pub fn shape(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        Ok(flatten(&self.run_pipeline(text)?))
    }

    /// [`Shaper::shape`] joined with `+` (`S+A+I+I+A`), the CLI's output format.
    pub fn shape_str(&self, text: &str) -> Result<String, Error> {
        let units = self.shape(text)?;
        Ok(units
            .iter()
            .map(|unit| unit.as_str())
            .collect::<Vec<_>>()
            .join("+"))
    }

    /// Do `a` and `b` render the same glyph sequence?
    pub fn same_shape(&self, a: &str, b: &str) -> Result<bool, Error> {
        Ok(self.shape(a)? == self.shape(b)?)
    }

    /// Per-token shaping breakdown (Python `shape_detailed`).
    pub fn shape_detailed(&self, text: &str) -> Result<Vec<TokenDetail>, Error> {
        let tokens = self.run_pipeline(text)?;
        Ok(tokens
            .iter()
            .map(|token| TokenDetail {
                cp: char::from_u32(token.cp).expect("token code points are scalar values"),
                alias: token.alias,
                position: token.position,
                fvs: token.first_fvs(),
                condition: token.condition,
                written: token.written.clone().unwrap_or_default(),
            })
            .collect())
    }

    /// Run the pipeline one rule at a time and record every condition change.
    pub fn trace(&self, text: &str) -> Result<ShapeTrace, Error> {
        check_word_chars(text)?;
        let mut tokens = self.tokenize(text);
        assign_positions(&mut tokens);
        let mut transitions = Vec::new();
        for rule in self.rules {
            let before: Vec<Option<Condition>> =
                tokens.iter().map(|token| token.condition).collect();
            (rule.apply)(&mut tokens, self);
            let changes: Vec<ConditionChange> = before
                .iter()
                .zip(&tokens)
                .enumerate()
                .filter(|(_, (old, token))| **old != token.condition)
                .map(|(index, (old, token))| ConditionChange {
                    token: index,
                    before: *old,
                    after: token.condition,
                })
                .collect();
            if !changes.is_empty() {
                transitions.push(RuleTransition {
                    rule: rule.name,
                    changes,
                });
            }
        }
        for token in &mut tokens {
            self.resolve_written(token);
        }
        Ok(ShapeTrace {
            positions: tokens.iter().map(|token| token.position).collect(),
            transitions,
            final_conditions: tokens.iter().map(|token| token.condition).collect(),
            written_by_token: tokens
                .iter()
                .map(|token| token.written.clone().unwrap_or_default())
                .collect(),
            shape: flatten(&tokens),
        })
    }
}

/// Flatten resolved tokens into the public shape.
pub(crate) fn flatten(tokens: &[Token]) -> Vec<WrittenUnit> {
    let mut shape = Vec::with_capacity(tokens.len());
    for token in tokens {
        match token.kind {
            TokenKind::Mvs => shape.push(WrittenUnit::Mvs),
            TokenKind::Nirugu => shape.push(WrittenUnit::Nirugu),
            TokenKind::Zwj => shape.push(WrittenUnit::Zwj),
            TokenKind::Letter => {
                if let Some(written) = &token.written {
                    shape.extend_from_slice(written);
                }
            }
        }
    }
    shape
}

// ── context helpers shared with the rules (Python `_prev_letter` & co.), returning indices ──

/// Nearest letter before `index` (structural tokens skipped).
pub(crate) fn prev_letter(tokens: &[Token], index: usize) -> Option<usize> {
    (0..index).rev().find(|&j| tokens[j].is_letter())
}

/// Nearest letter after `index` (structural tokens skipped).
pub(crate) fn next_letter(tokens: &[Token], index: usize) -> Option<usize> {
    (index + 1..tokens.len()).find(|&j| tokens[j].is_letter())
}

/// The token right before `index`.
pub(crate) fn prev_tok(tokens: &[Token], index: usize) -> Option<usize> {
    if index > 0 && index <= tokens.len() {
        Some(index - 1)
    } else {
        None
    }
}

/// The token right after `index`.
pub(crate) fn next_tok(tokens: &[Token], index: usize) -> Option<usize> {
    if index + 1 < tokens.len() {
        Some(index + 1)
    } else {
        None
    }
}

/// Nearest preceding letter reached without crossing an MVS; nirugu is transparent.
pub(crate) fn prev_adjacent_letter(tokens: &[Token], index: usize) -> Option<usize> {
    let mut j = index;
    while j > 0 {
        j -= 1;
        let token = &tokens[j];
        if token.is_mvs() {
            return None;
        }
        if token.is_letter() {
            return Some(j);
        }
        if token.is_nirugu() {
            continue;
        }
        return None;
    }
    None
}

/// Mirror image of [`prev_adjacent_letter`].
pub(crate) fn next_adjacent_letter(tokens: &[Token], index: usize) -> Option<usize> {
    let mut j = index + 1;
    while j < tokens.len() {
        let token = &tokens[j];
        if token.is_mvs() {
            return None;
        }
        if token.is_letter() {
            return Some(j);
        }
        if token.is_nirugu() {
            j += 1;
            continue;
        }
        return None;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_forms_without_rules() {
        let shaper = Shaper::new(Locale::Mng);
        assert_eq!(
            shaper.shape("\u{1820}").unwrap(),
            vec![WrittenUnit::A, WrittenUnit::A]
        );
        assert_eq!(
            shaper.shape("\u{1820}\u{180B}").unwrap(),
            vec![WrittenUnit::A]
        );
        assert_eq!(shaper.shape("\u{180E}").unwrap(), vec![WrittenUnit::Mvs]);
        assert_eq!(
            shaper.shape("\u{180A}\u{1823}").unwrap(),
            vec![WrittenUnit::Nirugu, WrittenUnit::U]
        );
        assert_eq!(
            shaper.shape("\u{200D}\u{1833}").unwrap(),
            vec![WrittenUnit::Zwj, WrittenUnit::Dd]
        );
        assert_eq!(shaper.shape("").unwrap(), Vec::<WrittenUnit>::new());
        assert_eq!(shaper.shape_str("\u{1820}").unwrap(), "A+A");
        assert!(shaper.same_shape("\u{1820}", "\u{1820}").unwrap());
        assert_eq!(
            shaper.shape("\u{1820} "),
            Err(Error::NonMongolianChar { ch: ' ', index: 1 })
        );
        for locale in [Locale::Tod, Locale::Sib, Locale::Mch] {
            assert!(!Shaper::new(locale).shape("\u{1820}").unwrap().is_empty());
            assert!(Shaper::new(locale).rule_names().is_empty());
        }
    }

    #[test]
    fn detailed_and_trace_cover_every_token() {
        let shaper = Shaper::new(Locale::Mng);
        let details = shaper
            .shape_detailed("\u{1832}\u{1820}\u{182F}\u{202F}\u{1820}")
            .unwrap();
        let positions: Vec<Position> = details.iter().map(|d| d.position).collect();
        assert_eq!(
            positions,
            vec![
                Position::Init,
                Position::Medi,
                Position::Fina,
                Position::Isol,
                Position::Isol
            ]
        );
        assert_eq!(details[3].cp, '\u{180E}'); // NNBSP folded into MVS
        assert_eq!(details[3].written, Vec::<WrittenUnit>::new());
        assert_eq!(details[0].alias, Some(Alias::T));
        let trace = shaper.trace("\u{1820}").unwrap();
        assert_eq!(trace.positions, vec![Position::Isol]);
        assert!(trace.transitions.is_empty());
        assert_eq!(trace.final_conditions, vec![None]);
        assert_eq!(
            trace.written_by_token,
            vec![vec![WrittenUnit::A, WrittenUnit::A]]
        );
        assert_eq!(trace.shape, shaper.shape("\u{1820}").unwrap());
    }

    #[test]
    fn unknown_letters_shape_to_nothing() {
        // U+181A is inside the block but has no MNG variants (Python parity).
        let shaper = Shaper::new(Locale::Mng);
        assert_eq!(shaper.shape("\u{181A}").unwrap(), Vec::<WrittenUnit>::new());
        let details = shaper.shape_detailed("\u{181A}").unwrap();
        assert_eq!(details[0].alias, None);
    }
}
