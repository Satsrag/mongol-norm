//! The shaper: data indexes, written-unit resolution, the context helpers the rules use, and the
//! public shaping entry points (`shape`, `same_shape`, `shape_detailed`, `trace`).
use std::collections::{HashMap, HashSet};

use crate::duplicates::collapse;
use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::generated::mng;
use crate::generated::mng_normalize;
use crate::normalize::NormalizeTable;
use crate::redundant_zwj::drop_redundant_zwj;
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
    /// The first FVS attached to the letter or MVS (Python reports only the first one).
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

/// The UTN #57 shaping engine and the canonical normalizer for Hudum ([`Locale::Mng`], the only
/// locale).
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
    pub(crate) normalize: NormalizeTable,
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
        let (data, normalize): (&'static LocaleData, _) = match locale {
            Locale::Mng => (&mng::DATA, &mng_normalize::DATA),
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
            normalize: NormalizeTable::new(normalize),
        }
    }

    /// Python's monkeypatched empty normalize table (`python/tests/test_shaper.py`,
    /// `python/tests/test_cli.py`): every chain falls back. No reachable MNG input misses the real
    /// table, so the fallback paths are only testable this way.
    #[cfg(any(test, feature = "testing"))]
    #[doc(hidden)]
    pub fn with_empty_normalize_table(locale: Locale) -> Shaper {
        let mut shaper = Shaper::new(locale);
        shaper.normalize = NormalizeTable::empty(shaper.normalize.canonical_version);
        shaper
    }

    /// The locale this shaper was built for.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// The names of the shaping rules, in the order they run.
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
    /// 1. the variant the FVS right after the letter names, if it names one,
    /// 2. else the variant carrying the rule-assigned condition,
    /// 3. else the default variant. Structural tokens resolve to nothing.
    ///
    /// Only the first FVS of a run can select: GB/T 25914-2023 counts a repeated FVS as invalid
    /// ("重复使用了规定的自由变体选择符"), EAC XIM11-662 shows `sh fvs3 fvs1` with both FVSes as
    /// error glyphs although FVS1 alone would be valid, and the font binds only the adjacent one.
    /// So `i fvs4 fvs1` is a plain `i`, not FVS1's variant.
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
        let written = token
            .first_fvs()
            .and_then(|fvs| self.variants.get(&(token.cp, token.position, Some(fvs))))
            .map(|variant| variant.written)
            .unwrap_or_else(|| self.contextual_written(token));
        token.written = Some(written);
    }

    /// The written units of a letter before any FVS applies: those of its condition, else its
    /// default. The font's phase III rules see this form, since the font applies an FVS only at
    /// the end (`III.fvs.MNG`).
    pub(crate) fn contextual_written(&self, token: &Token) -> &'static [WrittenUnit] {
        token
            .condition
            .and_then(|condition| self.condition_fvs(token.cp, token.position, condition))
            .and_then(|fvs| self.variants.get(&(token.cp, token.position, fvs)))
            .or_else(|| self.defaults.get(&(token.cp, token.position)))
            .map_or(&[], |variant| variant.written)
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

    /// The FVS right after the letter selects a variant at its position — the one
    /// [`Shaper::resolve_written`] takes (any further FVS is a repeat and selects nothing). An
    /// FVS that selects nothing locks nothing: the font carries it through phase III as
    /// `fvsN.ignored`, a mark the context lookups skip.
    pub(crate) fn has_valid_fvs(&self, token: &Token) -> bool {
        token.is_letter()
            && token.first_fvs().is_some_and(|fvs| {
                self.variants
                    .contains_key(&(token.cp, token.position, Some(fvs)))
            })
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
    ///
    /// A ZWNJ is an ignored glyph to the font, but the marker lookups are keyed on joining
    /// positions, and a ZWNJ changes those. The forward markers (masculine A/B, feminine D/E)
    /// are emitted after an `init`/`medi` vowel and move only onto `medi`/`fina` glyphs: they
    /// cross a ZWNJ exactly when the letter right after it is still `medi`/`fina` (a joiner
    /// follows the break), and a vowel of the other gender, which is on neither propagation
    /// list, stops them whatever its position. The backward chain (G–K) runs through letters of
    /// every position and skips ignored glyphs, so it does cross a ZWNJ — but it needs the
    /// `false` marker that G hands only to `init`/`medi`/`fina` glyphs, so an `isol` letter
    /// breaks it, and H turns a masculine vowel into its source only at `medi`/`fina`. Without a
    /// ZWNJ none of these distinctions can arise (every letter before a g/h in its segment is
    /// `init`/`medi`, every letter after an `init`/`medi` g/h is `medi`/`fina`), so the walk is
    /// unchanged for ZWNJ-free text.
    pub(crate) fn masc_marker_reaches_g_h(&self, tokens: &[Token], idx: usize) -> bool {
        // Can a forward marker cross the ZWNJ at `tokens[j]`, coming from `last_position` (the
        // position of the nearest letter on the g/h side)? Only onto a joined glyph.
        let crosses_break = |j: usize, last_position: Position| {
            tokens[j].is_transparent() && matches!(last_position, Position::Medi | Position::Fina)
        };

        // ── Forward (preprocessing.A/B/C and D/E) ──
        let mut j = idx;
        let mut last_position = tokens[idx].position;
        while j > 0 {
            j -= 1;
            let token = &tokens[j];
            if !token.is_letter() {
                if crosses_break(j, last_position) {
                    continue;
                }
                break; // mvs (or unjoined break) blocks; fall through to the backward check
            }
            if self.is_fem_vowel(token) {
                if matches!(token.position, Position::Init | Position::Medi) {
                    return false; // fem marker reaches `idx`, blocking the backward chain too
                }
                break; // an unjoined feminine vowel neither emits nor passes the marker
            }
            if self.is_masc_vowel(token) {
                if matches!(token.position, Position::Init | Position::Medi) {
                    return true;
                }
                break; // an unjoined masculine vowel neither emits nor passes the marker
            }
            last_position = token.position;
        }

        // ── Backward (preprocessing.G/H/J/K) ──
        // `preprocessing.G` only attaches the backward-propagation placeholder to joined
        // (`init`/`medi`/`fina`) glyphs, so an isolated g/h can never receive a later MASC marker
        // (and for a `fina` g/h, the scan below at `idx + 1` only continues when a transparent
        // ZWNJ follows it).
        if tokens[idx].position == Position::Isol {
            return false;
        }
        // Walk forward through an unbroken chain of non-fem letters, terminating at a
        // `medi`/`fina` masc vowel or at `fina letter + mvs + isol a`.
        let mut j = idx + 1;
        while j < tokens.len() {
            let next = &tokens[j];
            if !next.is_letter() {
                if next.is_transparent() {
                    j += 1;
                    continue;
                }
                return false;
            }
            if self.is_masc_vowel(next) && matches!(next.position, Position::Medi | Position::Fina)
            {
                return true;
            }
            if self.is_fem_vowel(next) {
                return false;
            }
            if matches!(next.position, Position::Init | Position::Medi) {
                j += 1;
                continue;
            }
            if next.position != Position::Fina {
                return false; // an isolated letter has no marker to pass on
            }
            let Some(k) = (j + 1..tokens.len()).find(|&k| !tokens[k].is_transparent()) else {
                return false;
            };
            if tokens[k].is_letter() {
                j = k; // only transparent tokens — a ZWNJ among them — were crossed
                continue;
            }
            return tokens[k].is_mvs()
                && next_tok_through_zwnj(tokens, k).is_some_and(|a| {
                    tokens[a].is_letter()
                        && tokens[a].alias == Some(Alias::A)
                        && tokens[a].position == Position::Isol
                });
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

    /// Shape `text` into its written-unit sequence. MVS and nirugu appear verbatim as
    /// [`WrittenUnit::Mvs`] and [`WrittenUnit::Nirugu`]; a ZWJ appears as [`WrittenUnit::Zwj`]
    /// only where it changes the ink — between two letters that already join, next to a nirugu,
    /// doubled, or at a word edge whose joined and unjoined forms are the same shape, it is
    /// dropped (see `redundant_zwj.rs` for the rules and the reviewed edge table). An MVS is a
    /// segment edge, so a ZWJ next to it is judged like one at the word edge.
    ///
    /// A ZWNJ breaks the cursive chain — the letter before it is written `fina` (or `isol`), the
    /// letter after it `init` (or `isol`) — while the context rules look through it, as they do
    /// in the reference font. It appears as [`WrittenUnit::Zwnj`], a segment edge for the ZWJ
    /// rules and for duplicate unification.
    ///
    /// Nine written units render as exactly the same ink as a sequence of other units, and each
    /// is unified with that sequence here. Five expand — `Dd` (both positions), medial `H`,
    /// medial `Hx` and initial `Cr` come out as `O A`, `A A`, `N N` and `O O`. The other four
    /// contract, because their expansion ends in `Aa`, which is itself a duplicate: a chain-final
    /// `A Aa` becomes `Aa` (or `A` when it is the whole chain), `O Aa` becomes `B2` and `I Aa`
    /// becomes `G`. That is what makes `shape` a fingerprint of the visible word — ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ
    /// are one word and shape identically. The standard's own unit sequence, which keeps all
    /// nine apart, is [`Shaper::shape_raw`]. `docs/internals.md` ("Duplicate encodings") has the
    /// evidence and the termination argument.
    ///
    /// Errors with [`Error::NonMongolianChar`] on anything but Mongolian letters, FVS, MVS,
    /// NNBSP, nirugu, ZWJ and ZWNJ — use [`Shaper::normalize_text`] for mixed-script text.
    pub fn shape(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        Ok(self.canonical_units(&self.shape_raw(text)?))
    }

    /// The public shape of an engine-level unit sequence ([`canonical_units`]). Shared by
    /// [`Shaper::shape`], [`Shaper::trace`] and the written-unit encoders, which is what keeps
    /// `normalize_written_units([L, I, Zwj, Aa])` and `normalize("ᠯᠢ\u{200D}ᠡ᠋")` on one
    /// contract.
    pub(crate) fn canonical_units(&self, raw: &[WrittenUnit]) -> Vec<WrittenUnit> {
        canonical_units(raw)
    }

    /// The redundant-ZWJ filter alone (no duplicate unification).
    pub(crate) fn without_redundant_zwj(&self, units: &[WrittenUnit]) -> Vec<WrittenUnit> {
        drop_redundant_zwj(units)
    }

    /// The engine's written-unit sequence before redundant ZWJs are dropped and duplicate
    /// encodings are unified — the sequence UTN #57 / GB/T 25914-2023 describe, and the one
    /// their EAC conformance vectors are checked against. All nine duplicates and every ZWJ and
    /// ZWNJ of the input can appear here.
    ///
    /// Not part of the public contract: it exists so the conformance suites and the table
    /// generator can compare against the standard verbatim. Everything user-facing goes through
    /// [`Shaper::shape`].
    #[doc(hidden)]
    pub fn shape_raw(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
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
            shape: self.canonical_units(&flatten(&tokens)),
        })
    }
}

/// The public shape of an engine-level unit sequence: redundant ZWJs dropped, then the duplicate
/// encodings unified — in that order, so a joiner that separated `I Aa` no longer blocks the `G`
/// contraction. Both backends fold their raw units with it, so [`Shaper::shape`] and
/// `FontShaper::shape` compare unit for unit.
pub(crate) fn canonical_units(raw: &[WrittenUnit]) -> Vec<WrittenUnit> {
    collapse(&drop_redundant_zwj(raw))
}

/// Flatten resolved tokens into the public shape.
pub(crate) fn flatten(tokens: &[Token]) -> Vec<WrittenUnit> {
    let mut shape = Vec::with_capacity(tokens.len() * 2);
    for token in tokens {
        match token.kind {
            TokenKind::Mvs => shape.push(WrittenUnit::Mvs),
            TokenKind::Nirugu => shape.push(WrittenUnit::Nirugu),
            TokenKind::Zwj => shape.push(WrittenUnit::Zwj),
            TokenKind::Zwnj => shape.push(WrittenUnit::Zwnj),
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

/// The token right before `index` for the rules that want an MVS *directly* beside a letter
/// (chachlag, chachlag onset, dotless g, the MASC marker) — looking through a ZWNJ.
///
/// The font's lookups ignore nirugu, ZWJ and ZWNJ alike; the engine keeps nirugu and ZWJ opaque
/// here because they extend the joining chain, so the letter is no longer the isolated/final
/// glyph those lookups name, and [`prev_tok`] encodes that. A ZWNJ ends the chain instead, so
/// the glyph can be the right one and the rule sees through it: when the run of transparent
/// tokens right before `index` holds a ZWNJ (and ZWNJs do not block context), the result is the
/// first token before that run, and the rule's own position check decides — `MVS ZWJ ZWNJ a` is
/// chachlag `Aa` like `MVS ZWNJ a`, while in `MVS ZWNJ ZWJ a` the `a` is final. Otherwise it is
/// [`prev_tok`], unchanged.
pub(crate) fn prev_tok_through_zwnj(tokens: &[Token], index: usize) -> Option<usize> {
    let start = (0..index)
        .rev()
        .find(|&j| !tokens[j].is_transparent())
        .map_or(0, |j| j + 1);
    if tokens[start..index]
        .iter()
        .any(|t| t.is_zwnj() && t.is_transparent())
    {
        start.checked_sub(1)
    } else {
        prev_tok(tokens, index)
    }
}

/// Mirror image of [`prev_tok_through_zwnj`].
pub(crate) fn next_tok_through_zwnj(tokens: &[Token], index: usize) -> Option<usize> {
    let end = (index + 1..tokens.len())
        .find(|&j| !tokens[j].is_transparent())
        .unwrap_or(tokens.len());
    if tokens[index + 1..end]
        .iter()
        .any(|t| t.is_zwnj() && t.is_transparent())
    {
        (end < tokens.len()).then_some(end)
    } else {
        next_tok(tokens, index)
    }
}

/// Nearest preceding letter reached without crossing an MVS; nirugu, ZWJ and (unless it blocks
/// context) ZWNJ are transparent.
pub(crate) fn prev_adjacent_letter(tokens: &[Token], index: usize) -> Option<usize> {
    let mut j = index;
    while j > 0 {
        j -= 1;
        let token = &tokens[j];
        if token.is_letter() {
            return Some(j);
        }
        if token.is_transparent() {
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
        if token.is_letter() {
            return Some(j);
        }
        if token.is_transparent() {
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
        // The engine keeps `Dd` and the ZWJ; the public shape drops the joiner (a lone `Dd:fina`
        // is the same shape as `Dd:isol`) and folds the duplicate: `O A`.
        assert_eq!(
            shaper.shape_raw("\u{200D}\u{1833}").unwrap(),
            vec![WrittenUnit::Zwj, WrittenUnit::Dd]
        );
        assert_eq!(
            shaper.shape("\u{200D}\u{1833}").unwrap(),
            vec![WrittenUnit::O, WrittenUnit::A]
        );
        assert_eq!(shaper.shape("").unwrap(), Vec::<WrittenUnit>::new());
        assert_eq!(shaper.shape_str("\u{1820}").unwrap(), "A+A");
        assert!(shaper.same_shape("\u{1820}", "\u{1820}").unwrap());
        assert_eq!(
            shaper.shape("\u{1820} "),
            Err(Error::NonMongolianChar { ch: ' ', index: 1 })
        );
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
        // `resolve_written` memoises: once resolved, a token keeps its written units whatever
        // condition a later rule sets.
        let mut token = shaper.tokenize("\u{1820}").remove(0);
        token.position = Position::Fina;
        shaper.resolve_written(&mut token);
        assert_eq!(token.written, Some(&[WrittenUnit::A][..]));
        token.condition = Some(Condition::PostBowed);
        shaper.resolve_written(&mut token);
        assert_eq!(token.written, Some(&[WrittenUnit::A][..]));

        // No rule pair runs into this now that the context lookups stay inside the MVS segment:
        // III.4 resolves the vowel before a medial `i`, which is never final, and III.5 only
        // touches final vowels. `b a MVS nirugu i n` used to: III.4 walked back from the `i`
        // across the MVS, froze the `a` at `A`, and III.5's `post_bowed` came too late. It now
        // shapes as the font does, with `post_bowed` `Aa` and a single `I`.
        assert_eq!(
            shaper
                .shape("\u{182A}\u{1820}\u{180E}\u{180A}\u{1822}\u{1828}")
                .unwrap(),
            vec![
                WrittenUnit::B,
                WrittenUnit::Aa,
                WrittenUnit::Mvs,
                WrittenUnit::Nirugu,
                WrittenUnit::I,
                WrittenUnit::A,
            ]
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
