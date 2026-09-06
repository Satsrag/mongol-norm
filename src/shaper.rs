//! The shaper: data indexes, written-unit resolution, the context helpers the rules use, and the
//! public shaping entry points (`shape`, `same_shape`, `shape_detailed`, `trace`).
use std::collections::{HashMap, HashSet};

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::generated::mng_normalize;
use crate::generated::{mch, mng, sib, tod};
use crate::normalize::NormalizeTable;
use crate::rules::{self, Rule};
use crate::tables::{Fvs, Letter, Locale, LocaleData, ParticleSym, Position, Variant};
use crate::token::{assign_positions, tokenize, Token, TokenKind};
use crate::unicode::check_word_chars;
use crate::Error;

/// Per-token breakdown returned by [`Shaper::shape_detailed`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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
    pub(crate) normalize: Option<NormalizeTable>,
}

/// `Shaper` holds only owned tables and `&'static` data, so sharing one behind a reference
/// across threads is sound; this pins that claim at compile time.
const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    let _ = assert_send_sync::<Shaper>;
};

impl Default for Shaper {
    fn default() -> Shaper {
        Shaper::new(Locale::Mng)
    }
}

impl std::fmt::Debug for Shaper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaper")
            .field("locale", &self.locale)
            .field("letters", &self.letters.len())
            .field("variants", &self.variants.len())
            .field("rules", &self.rules.len())
            .finish_non_exhaustive()
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
            normalize: match locale {
                Locale::Mng => Some(NormalizeTable::new(&mng_normalize::DATA)),
                Locale::Tod | Locale::Sib | Locale::Mch => None,
            },
        }
    }

    /// Python's monkeypatched empty normalize table (`python/tests/test_shaper.py`,
    /// `python/tests/test_cli.py`): every chain falls back. No reachable MNG input misses the real
    /// table, so the fallback paths are only testable this way.
    #[cfg(any(test, feature = "testing"))]
    #[doc(hidden)]
    pub fn with_empty_normalize_table(locale: Locale) -> Shaper {
        let mut shaper = Shaper::new(locale);
        let version = shaper
            .normalize
            .as_ref()
            .map_or("mng-canonical/1", |table| table.canonical_version);
        shaper.normalize = Some(NormalizeTable::empty(version));
        shaper
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
    ///
    /// `None` means no variant at `position` carries `condition`; `Some(None)` means the bare
    /// (FVS-less) variant carries it — Python's FVS `0`.
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
    ///
    /// The result is memoised on the token and never invalidated: a rule that changes an
    /// already-resolved token's condition has no effect on its written units (Python parity —
    /// III.4 and III.5 resolve the *previous* token mid-pipeline, freezing it).
    pub(crate) fn resolve_written(&self, token: &mut Token) {
        if token.written.is_some() {
            return;
        }
        if !token.is_letter() {
            token.written = Some(&[]);
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
        token.written = Some(written.unwrap_or(&[]));
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

    /// The prologue shared by [`Shaper::shape`] and [`Shaper::trace`]: validate, tokenize,
    /// assign positions.
    fn prepare(&self, text: &str) -> Result<Vec<Token>, Error> {
        check_word_chars(text)?;
        let mut tokens = self.tokenize(text);
        assign_positions(&mut tokens);
        Ok(tokens)
    }

    /// Resolve every token's written units (idempotent — the rules may have resolved some).
    fn resolve_all(&self, tokens: &mut [Token]) {
        for token in tokens {
            self.resolve_written(token);
        }
    }

    fn run_pipeline(&self, text: &str) -> Result<Vec<Token>, Error> {
        let mut tokens = self.prepare(text)?;
        rules::run_rules(self.rules, &mut tokens, self);
        self.resolve_all(&mut tokens);
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
        let mut out = String::new();
        for unit in units {
            if !out.is_empty() {
                out.push('+');
            }
            out.push_str(unit.as_str());
        }
        Ok(out)
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
                written: token
                    .written
                    .map(<[WrittenUnit]>::to_vec)
                    .unwrap_or_default(),
            })
            .collect())
    }

    /// Run the pipeline one rule at a time and record every condition change.
    pub fn trace(&self, text: &str) -> Result<ShapeTrace, Error> {
        let mut tokens = self.prepare(text)?;
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
        self.resolve_all(&mut tokens);
        Ok(ShapeTrace {
            positions: tokens.iter().map(|token| token.position).collect(),
            transitions,
            final_conditions: tokens.iter().map(|token| token.condition).collect(),
            written_by_token: tokens
                .iter()
                .map(|token| {
                    token
                        .written
                        .map(<[WrittenUnit]>::to_vec)
                        .unwrap_or_default()
                })
                .collect(),
            shape: flatten(&tokens),
        })
    }
}

/// Flatten resolved tokens into the public shape.
pub(crate) fn flatten(tokens: &[Token]) -> Vec<WrittenUnit> {
    let mut shape = Vec::with_capacity(tokens.len() * 2);
    for token in tokens {
        match token.kind {
            TokenKind::Mvs => shape.push(WrittenUnit::Mvs),
            TokenKind::Nirugu => shape.push(WrittenUnit::Nirugu),
            TokenKind::Zwj => shape.push(WrittenUnit::Zwj),
            TokenKind::Letter => {
                if let Some(written) = token.written {
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

/// The token right before `index`. (`_tokens` is unused; it keeps the signature symmetric with
/// the other context helpers.)
pub(crate) fn prev_tok(_tokens: &[Token], index: usize) -> Option<usize> {
    if index > 0 {
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

    /// Tokenize `text` and assign positions, ready for the context helpers.
    fn prepared(shaper: &Shaper, text: &str) -> Vec<Token> {
        let mut tokens = shaper.tokenize(text);
        assign_positions(&mut tokens);
        tokens
    }

    /// One letter token at `position` carrying `condition`, resolved.
    fn resolved(
        shaper: &Shaper,
        cp: u32,
        position: Position,
        condition: Option<Condition>,
    ) -> Token {
        let text = char::from_u32(cp).expect("scalar value").to_string();
        let mut token = shaper.tokenize(&text).remove(0);
        token.position = position;
        token.condition = condition;
        shaper.resolve_written(&mut token);
        token
    }

    #[test]
    fn masc_marker_reaches_g_h_paths() {
        let shaper = Shaper::new(Locale::Mng);
        // Every expectation below was cross-checked against Python's
        // `MongolianShaper('MNG')._masc_marker_reaches_g_h`.

        // (a) Forward (preprocessing.A/B/C): a masc vowel at init/medi before the g/h.
        // `a l g` (ᠠᠯᠭ) — a.init is masculine, so the marker reaches g.fina.
        let tokens = prepared(&shaper, "\u{1820}\u{182F}\u{182D}");
        assert!(shaper.masc_marker_reaches_g_h(&tokens, 2));

        // (b) Backward (preprocessing.G/H/J/K), the witness from the Python docstring:
        // `s i g s i g a` (ᠰᠢᠭᠰᠢᠭᠠ) — the first g reaches the trailing masc `a` through
        // the unbroken init/medi chain g→s→i→g.
        let tokens = prepared(
            &shaper,
            "\u{1830}\u{1822}\u{182D}\u{1830}\u{1822}\u{182D}\u{1820}",
        );
        assert!(shaper.masc_marker_reaches_g_h(&tokens, 2));

        // (c) A feminine vowel blocks both directions.
        let tokens = prepared(&shaper, "\u{1821}\u{182F}\u{182D}"); // `e l g`
        assert!(!shaper.masc_marker_reaches_g_h(&tokens, 2));
        // `e g e n i g t a` — the fem `e` earlier in the word blocks the second g.
        let tokens = prepared(
            &shaper,
            "\u{1821}\u{182D}\u{1821}\u{1828}\u{1822}\u{182D}\u{1832}\u{1820}",
        );
        assert!(!shaper.masc_marker_reaches_g_h(&tokens, 5));

        // (d) The `fina letter + mvs + isol a` terminator of the backward walk:
        // `i g l mvs a` (ᠢᠭᠯ᠎ᠠ) — g.medi, then l.fina, MVS, isolated `a`.
        let tokens = prepared(&shaper, "\u{1822}\u{182D}\u{182F}\u{180E}\u{1820}");
        assert!(shaper.masc_marker_reaches_g_h(&tokens, 1));
        // … and `e` in place of the `a` is not the terminator.
        let tokens = prepared(&shaper, "\u{1822}\u{182D}\u{182F}\u{180E}\u{1821}");
        assert!(!shaper.masc_marker_reaches_g_h(&tokens, 1));
    }

    #[test]
    fn resolve_written_condition_branch() {
        let shaper = Shaper::new(Locale::Mng);

        // (a) The bare (FVS-less) h.fina variant carries `chachlag_onset` and
        // `masculine_devsger`, so `condition_fvs` reports `Some(None)` (Python FVS 0).
        assert_eq!(
            shaper.condition_fvs(0x182C, Position::Fina, Condition::MasculineDevsger),
            Some(None)
        );
        let token = resolved(
            &shaper,
            0x182C,
            Position::Fina,
            Some(Condition::MasculineDevsger),
        );
        assert_eq!(token.written, Some(&[WrittenUnit::H][..]));

        // (b) The first FVS-carrying variant that owns a condition and whose written units
        // differ from that (cp, position)'s default: the condition must pick the variant.
        let (letter, variant, condition) = mng::DATA
            .letters
            .iter()
            .flat_map(|letter| letter.variants.iter().map(move |variant| (letter, variant)))
            .filter_map(|(letter, variant)| {
                let condition = *variant.conditions.first()?;
                variant.fvs?;
                let default = shaper.defaults.get(&(letter.cp, variant.position))?;
                (default.written != variant.written
                    && shaper.condition_fvs(letter.cp, variant.position, condition)
                        == Some(variant.fvs))
                .then_some((letter, variant, condition))
            })
            .next()
            .expect("MNG has an FVS variant selected by a condition");
        let default = shaper.defaults[&(letter.cp, variant.position)].written;
        let token = resolved(&shaper, letter.cp, variant.position, Some(condition));
        assert_eq!(
            token.written,
            Some(variant.written),
            "U+{:04X} {} {}",
            letter.cp,
            variant.position,
            condition.as_str()
        );
        assert_ne!(token.written, Some(default));

        // (c) An FVS on the token wins over the condition: h.init + FVS1 is `Hx`, while the
        // `feminine` condition would select h.init.fvs2 = `G`.
        let mut token = shaper.tokenize("\u{182C}\u{180B}").remove(0);
        token.position = Position::Init;
        token.condition = Some(Condition::Feminine);
        shaper.resolve_written(&mut token);
        assert_eq!(token.written, Some(&[WrittenUnit::Hx][..]));

        // (d) A condition unknown to the position falls back to the default: h.fina has no
        // `feminine` variant, so the default `H` is used.
        assert_eq!(
            shaper.condition_fvs(0x182C, Position::Fina, Condition::Feminine),
            None
        );
        let token = resolved(&shaper, 0x182C, Position::Fina, Some(Condition::Feminine));
        assert_eq!(token.written, Some(&[WrittenUnit::H][..]));
    }

    #[test]
    fn memoised_written_is_never_invalidated() {
        let shaper = Shaper::new(Locale::Mng);
        // `b a MVS nirugu i n`: III.4 resolves the `a` while walking back from the medial `i`,
        // freezing its written units at the default `A`. III.5 then assigns `post_bowed` to the
        // same `a`, which no longer has any effect. Python does exactly the same — verified with
        // `MongolianShaper('MNG').shape_detailed(...)`.
        let text = "\u{182A}\u{1820}\u{180E}\u{180A}\u{1822}\u{1828}";
        let details = shaper.shape_detailed(text).unwrap();
        assert_eq!(details[1].alias, Some(Alias::A));
        assert_eq!(details[1].condition, Some(Condition::PostBowed));
        assert_eq!(details[1].written, vec![WrittenUnit::A]);
        assert_eq!(
            shaper.shape(text).unwrap(),
            vec![
                WrittenUnit::B,
                WrittenUnit::A,
                WrittenUnit::Mvs,
                WrittenUnit::Nirugu,
                WrittenUnit::I,
                WrittenUnit::I,
                WrittenUnit::A,
            ]
        );
        // Without the mid-pipeline freeze the same `b a` resolves through `post_bowed` to `Aa`.
        assert_eq!(
            shaper.shape("\u{182A}\u{1820}").unwrap(),
            vec![WrittenUnit::B, WrittenUnit::Aa]
        );
    }

    #[test]
    fn debug_reports_the_index_sizes() {
        let shaper = Shaper::new(Locale::Mng);
        let text = format!("{shaper:?}");
        assert!(text.starts_with("Shaper { locale: Mng,"), "{text}");
        assert!(text.contains("letters: 35"), "{text}");
        assert!(
            text.contains(&format!("rules: {}", shaper.rule_names().len())),
            "{text}"
        );
        assert!(text.ends_with(".. }"), "{text}");
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
