//! Declarative shaping rules for Traditional Mongolian — a 1:1 port of `mongol_norm/rules.py`,
//! which itself mirrors mongfontbuilder's OpenType pipeline (`otl/iii.py`): one function per
//! Lookup, same name, same intent, same order.
//!
//! Every rule is *first-writer-wins* (`if tok.condition.is_some() { return }`) except
//! `III.2g.g.dotless` and `III.5.post_bowed`, which override earlier conditions to mirror
//! OpenType class-membership semantics (see the Python comments for the font verification).
//!
//! Context lookups stay inside the MVS segment. To the font an MVS is a base glyph that no lookup
//! sees past, while nirugu, ZWJ and FVS are marks it steps over. So `next_letter_in_segment`,
//! `prev_adjacent_letter` and `next_adjacent_letter` stop at an MVS and step over the joiners.
//! The rules that do read an MVS name it: III.1 (`prev_tok`), III.2c, III.2f and III.2g's g.dotless
//! rule (2) (`letter_after_mvs`, the letter right after it) and III.3 (whole segments).
//!
//! An FVS locks a letter against the context rules only when it selects a variant there
//! ([`Shaper::has_valid_fvs`]), and that variant wins whatever condition a rule sets. An FVS that
//! selects nothing is transparent: GB/T 25914-2023 gives the FVS "双向传递语法上下文特性" (it
//! passes grammatical context both ways) except where Appendix B writes it out, and shows an
//! invalid one as an error glyph; the font carries it through phase III as `fvsN.ignored`.
//! So `a s t fvs3 r a` is `A A S T R A`, as without the FVS. The rules that read an FVS as a signal
//! of their own still count any FVS: III.2c, for an FVS on an n/h/g before an MVS (their
//! Appendix B tables write out `n + FVS1 + MVS` and the like; EAC XIM11-46 keeps
//! `a n fvs3 mvs e` at the first form) and on the vowel after the MVS (GB postpones the MVS
//! shaping), GB.A/GB.B, the cluster and d.marked rules, and the particles.

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::shaper::{next_adjacent_letter, next_tok, prev_adjacent_letter, prev_tok, Shaper};
use crate::tables::{Fvs, Locale, ParticleSym, Position};
use crate::token::Token;

/// A single shaping rule = a single mongfontbuilder Lookup.
pub(crate) struct Rule {
    /// The Lookup name, e.g. `III.2f.h_g.harmony` (frozen by the phase-trace golden).
    pub name: &'static str,
    /// Mutates the tokens' `condition` fields in place.
    pub apply: fn(&mut [Token], &Shaper),
}

/// The rule table of a locale.
pub(crate) fn rules_for(locale: Locale) -> &'static [Rule] {
    match locale {
        Locale::Mng => RULES_MNG,
    }
}

/// Execute the rules in declared order.
pub(crate) fn run_rules(rules: &[Rule], tokens: &mut [Token], shaper: &Shaper) {
    for rule in rules {
        (rule.apply)(tokens, shaper);
    }
}

/// A copy of the token fields a rule inspects, so the token slice can be mutated afterwards.
#[derive(Clone, Copy)]
struct View {
    is_letter: bool,
    alias: Option<Alias>,
    position: Position,
    has_fvs: bool,
    first_fvs: Option<Fvs>,
    condition: Option<Condition>,
}

fn view(tokens: &[Token], index: usize) -> View {
    let token = &tokens[index];
    View {
        is_letter: token.is_letter(),
        alias: token.alias,
        position: token.position,
        has_fvs: token.has_fvs(),
        first_fvs: token.first_fvs(),
        condition: token.condition,
    }
}

fn set(tokens: &mut [Token], index: usize, condition: Condition) {
    tokens[index].condition = Some(condition);
}

macro_rules! per_token {
    ($name:ident => $at:ident) => {
        fn $name(tokens: &mut [Token], shaper: &Shaper) {
            for index in 0..tokens.len() {
                $at(tokens, index, shaper);
            }
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.1 — Chachlag: isolated a/e after MVS, whatever FVS follows the vowel.
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii1_chachlag => iii1_chachlag_at);

/// Isolated a/e directly after an MVS take `chachlag` — see rules.py::_iii1_chachlag_at. An FVS
/// on the vowel does not stop it, as in the font (`III.a_e.chachlag`):
/// - a valid FVS still picks its own variant, which wins over any condition: B.8 gives the
///   isolated a FVS1–3 and B.9 the isolated e FVS1–2;
/// - any other FVS changes nothing, so `mvs a fvs4` and `mvs e fvs3` show the chachlag form,
///   as `[MVS] + a/e` reads with the invalid FVS transparent (but see the upstream
///   inconsistency below).
///
/// What an FVS on the vowel, valid or not, does change is the MVS shaping, which GB postpones:
/// the font's `III.a_e.chachlag.GB` resets the MVS, so the consonant before it takes no
/// `chachlag_onset` (III.2c).
///
/// **Upstream inconsistency** (mongfontbuilder c695f96, found with #50). The docstring of `iii1`
/// says `III.a_e.chachlag.GB` also turns the vowel back into the "isolated default a", and
/// UTN #57 Table 6 agrees ("a e — if follows an MVS and precedes an FVS: Default"). The code
/// resets only the MVS, so the vowel keeps `Aa`.
/// - With a valid FVS this makes no difference: `III.fvs.MNG` replaces `Aa` with the FVS's
///   variant at the end anyway.
/// - Only an invalid FVS shows it. The same FVS then counts for the consonant (no onset) but not
///   for the vowel (still `Aa`).
///
/// The engine follows the code. This is not reported upstream: it takes an invalid FVS, and real
/// text never puts an FVS on the separated vowel. `tests/font_vs_engine.rs` (`WATCHED`) fails
/// when a font bump changes it. If upstream makes the code match its docstring, undo it here:
/// 1. here, return before `set` when the vowel carries any FVS (`if tok.has_fvs { return; }`,
///    the code before #50). A valid FVS still gives its variant, and an invalid one leaves the
///    default form (`A A` for a, `A` for e);
/// 2. III.2c needs no change: it skips a vowel with an FVS either way;
/// 3. III.2g g.dotless rule (2) reads this chachlag, so it stops firing before a vowel with an
///    FVS on its own. Check `s g mvs a fvs1` against the font;
/// 4. flip the invalid-FVS cases of `test_step1_chachlag_ignores_invalid_fvs` to the default
///    form, and recheck `a n mvs a fvs4` and `s g mvs a fvs1` in
///    `test_step2_chachlag_onset_postponed_by_fvs_on_vowel`. Both tests have Python twins.
///
/// An FVS right after the MVS (kept on the MVS token) changes nothing here: the font's lookup
/// ignores it, so `ch mvs fvs2 a` is `Ch Mvs Aa` like `ch mvs a`. It does stop a particle (III.3),
/// as in the font, although GB/T 25914-2023 writes both rules alike (`[MVS] + a`, `[MVS] + i`):
/// the engine follows the reference font here and will follow it if that changes.
fn iii1_chachlag_at(tokens: &mut [Token], index: usize, _shaper: &Shaper) {
    let tok = view(tokens, index);
    if !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::A | Alias::E)) {
        return;
    }
    let Some(prev) = prev_tok(tokens, index) else {
        return;
    };
    if !tokens[prev].is_mvs() {
        return;
    }
    set(tokens, index, Condition::Chachlag);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2a — o/u/oe/ue `marked`, d `marked`
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2a_o_u_oe_ue_marked => iii2a_o_u_oe_ue_marked_at);

/// iii2a.MAIN: o/u/oe/ue directly after an *initial* consonant take the tall-stem `marked`
/// form — see rules.py::_iii2a_o_u_oe_ue_marked_at.
fn iii2a_o_u_oe_ue_marked_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::O | Alias::U | Alias::Oe | Alias::Ue)) {
        return;
    }
    let Some(prev) = prev_adjacent_letter(tokens, index) else {
        return;
    };
    if !(shaper.is_consonant(&tokens[prev]) && tokens[prev].position == Position::Init) {
        return;
    }
    set(tokens, index, Condition::Marked);
}

per_token!(iii2a_o_u_oe_ue_marked_gb_a => iii2a_o_u_oe_ue_marked_gb_a_at);

/// An FVS right after the letter as GB.A's lookup sees it: the letter's own, or one carried by a
/// ZWJ or nirugu right after it (phase III turns the joiners into marks the lookup skips).
fn fvs_follows(tokens: &[Token], index: usize) -> bool {
    tokens[index].has_fvs()
        || tokens[index + 1..]
            .iter()
            .take_while(|token| token.is_nirugu())
            .any(Token::has_fvs)
}

/// An FVS right before the letter as GB.A's lookup sees it: one carried by a ZWJ or nirugu right
/// before it, or the previous letter's own.
fn fvs_precedes(tokens: &[Token], index: usize) -> bool {
    for token in tokens[..index].iter().rev() {
        if token.has_fvs() {
            return token.is_nirugu() || token.is_letter();
        }
        if !token.is_nirugu() {
            return false;
        }
    }
    false
}

/// The FVS between the letters at `from` and `to`, on the first one or on the joiners between,
/// when there is exactly one: what a lookup with UseMarkFilteringSet @fvs reads there, the
/// joiners being marks it skips. With two FVS such a context does not match.
fn single_fvs_between(tokens: &[Token], from: usize, to: usize) -> Option<Fvs> {
    let mut fvs = tokens[from..to]
        .iter()
        .flat_map(|token| token.fvs.iter().copied());
    let first = fvs.next()?;
    fvs.next().is_none().then_some(first)
}

/// GB.A: an FVS next to the vowel resets `marked`. The two sides differ:
///
/// * an FVS *after* the vowel (its own, or one after a ZWJ or nirugu right after it: the
///   font's lookup skips the joiners) resets it at any position — medial too: `h ue fvs4 p`
///   is `G O P`, not `G O I P`. mongfontbuilder resets medial and final vowels alike
///   (`III.o_u_oe_ue.marked.GB.A`, checked on the pinned hudum.otf) and documents it as a GB
///   requirement (`iii2a`: "the `marked` will be skipped if the vowel precedes or follows an
///   FVS"). Under GB/T 25914-2023's general FVS clause an FVS that selects nothing would be
///   transparent (see the module docs) and leave the vowel marked; mongfontbuilder reads the
///   o/u/oe/ue rules as a case Appendix B writes out, and no EAC row tells the two apart;
/// * an FVS *before* the vowel (on the preceding letter, or after a joiner in between) resets
///   it at final position only — verified against DraftNew-Regular.otf: `h fvs3 oe` resets,
///   `h fvs3 oe l` stays marked.
///
/// A marked vowel follows a consonant, so it is never `isol` or `init`. The FVS's own variant
/// still wins where the FVS is valid; the reset only shows where it selects nothing.
fn iii2a_o_u_oe_ue_marked_gb_a_at(tokens: &mut [Token], index: usize, _shaper: &Shaper) {
    let tok = view(tokens, index);
    if !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::O | Alias::U | Alias::Oe | Alias::Ue)) {
        return;
    }
    if tok.condition != Some(Condition::Marked) {
        return;
    }
    if fvs_follows(tokens, index) {
        tokens[index].condition = None;
        return;
    }
    if tok.position != Position::Fina {
        return;
    }
    if fvs_precedes(tokens, index) {
        tokens[index].condition = None;
    }
}

per_token!(iii2a_o_u_oe_ue_marked_gb_b => iii2a_o_u_oe_ue_marked_gb_b_at);

/// GB.B: the single exception to GB.A — initial h/g carrying FVS2/FVS4 + final oe/ue stays
/// `marked` even though an FVS sits between them — see rules.py::_iii2a_o_u_oe_ue_marked_gb_b_at.
/// The font marks it in two places, and the engine takes either:
/// - GB.B itself (UseMarkFilteringSet @fvs) reads the one FVS between the h/g and the vowel, so
///   one carried by a joiner counts too: `g zwj fvs4 ue` is `G Ue`;
/// - III.5's GB rule G (`III.vowel.post_bowed.MNG.GB`) reads the first FVS right after the h/g,
///   skipping the rest: `g fvs2 fvs4 ue` is `G Ue` too, although GB.B sees two FVS there.
fn iii2a_o_u_oe_ue_marked_gb_b_at(tokens: &mut [Token], index: usize, _shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::Oe | Alias::Ue)) || tok.position != Position::Fina {
        return;
    }
    let Some(prev_index) = prev_adjacent_letter(tokens, index) else {
        return;
    };
    let prev = view(tokens, prev_index);
    if !matches!(prev.alias, Some(Alias::H | Alias::G)) || prev.position != Position::Init {
        return;
    }
    let fvs24 = |fvs: Option<Fvs>| matches!(fvs, Some(Fvs::Fvs2 | Fvs::Fvs4));
    if !fvs24(single_fvs_between(tokens, prev_index, index)) && !fvs24(prev.first_fvs) {
        return;
    }
    set(tokens, index, Condition::Marked);
}

per_token!(iii2a_oe_ue_cluster_marked => iii2a_oe_ue_cluster_marked_at);

/// iii2a.GB.A/B/C combined: a medial o/u/oe/ue at the end of an unbroken init→medi consonant
/// cluster (≥ 1 medi consonant) gets `marked`. Nirugu is transparent.
///
/// `saw_medi` is required, not incidental: the single-init case (`consonant.init + vowel.medi`)
/// belongs to iii2a.MAIN ([`iii2a_o_u_oe_ue_marked_at`]), so this rule must fire only for an
/// init + ≥1 medi consonant chain (`g ue l` is marked by iii2a.MAIN, not here). Verified against
/// DraftNew-Regular.otf: `g g ue l` marks ue, `b a g ue l` leaves it default (the vowel `a` breaks
/// the chain) — see rules.py::_iii2a_oe_ue_cluster_marked_at.
fn iii2a_oe_ue_cluster_marked_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::O | Alias::U | Alias::Oe | Alias::Ue)) {
        return;
    }
    if tok.position != Position::Medi || fvs_follows(tokens, index) {
        return;
    }
    let mut j = index;
    let mut saw_medi = false;
    while j > 0 {
        j -= 1;
        let t = &tokens[j];
        if !t.is_letter() {
            if t.is_nirugu() {
                continue;
            }
            return; // mvs/etc. blocks
        }
        if !shaper.is_consonant(t) {
            return; // vowel blocks the chain
        }
        if t.position == Position::Init {
            if saw_medi {
                set(tokens, index, Condition::Marked);
            }
            return;
        }
        if t.position != Position::Medi {
            return; // fina / isol — not a propagation source
        }
        saw_medi = true;
    }
}

per_token!(iii2a_d_marked => iii2a_d_marked_at);

/// Initial d before a final vowel (Twelve Syllabaries form); an FVS on d or the vowel cancels.
/// Gated on d.init (mirrors iii.py / .fea, not the UTN prose: the data ships a marked variant
/// only for d.init, and without the gate a d.medi would claim the slot and block III.2e's
/// `onset`) — see rules.py::_iii2a_d_marked_at.
///
/// So does an FVS carried by a joiner between them: the font's reset (`III.d.marked.GB`,
/// UseMarkFilteringSet @fvs) skips the joiners, so `d nirugu fvs4 ue` keeps the default `T`.
/// The reset wants exactly one FVS there, so the font keeps `D` for two (`d fvs3 fvs2 oe`).
/// The engine cancels for any FVS, as GB has it, and keeps that difference on purpose.
fn iii2a_d_marked_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::D) {
        return;
    }
    if tok.position != Position::Init || tok.has_fvs {
        return;
    }
    let Some(next) = next_letter_in_segment(tokens, index) else {
        return;
    };
    if !(shaper.is_vowel(&tokens[next]) && tokens[next].position == Position::Fina) {
        return;
    }
    if tokens[index + 1..=next].iter().any(Token::has_fvs) {
        return; // GB: an FVS on the following vowel, or on a joiner before it, also cancels
    }
    set(tokens, index, Condition::Marked);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2c — chachlag_onset for n/j/w/h/g before MVS + isolated a/e
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2c_chachlag_onset => iii2c_chachlag_onset_at);

/// n/j/w before MVS + isolated a/e, and h/g before MVS + isolated a, take `chachlag_onset`;
/// `g` before MVS + isolated e takes the GB-specific condition instead — see
/// rules.py::_iii2c_chachlag_onset_at. Not when the vowel carries an FVS, valid or not: GB
/// postpones the MVS shaping then, and the font's `III.a_e.chachlag.GB` resets the narrow MVS
/// this lookup needs, so `a n mvs a fvs1` keeps the default n (`A A A Mvs A`). The EAC suite
/// backs this: every row with an FVS on that vowel expects the wide MVS, e.g. `a b u mvs a fvs3`
/// → `A A B O - A A`, so the vowel is not a separated final vowel there. UTN #57 Table 6
/// (step 2) does not mention the case. The vowel must follow the MVS directly
/// (`letter_after_mvs`): `a n mvs mvs a` keeps the default n, as in the font.
///
/// An FVS on the consonant itself depends on the letter. GB/T 25914-2023 writes out an FVS
/// before the MVS for n, h and g (B.16 `n + FVS1 + MVS + a/e`, B.20 `h + FVS1 + MVS`, B.21
/// `g + FVS3 + MVS` and `g + FVS1 + MVS`). There the FVS is one of the cases "附录 B 中明确定义的",
/// so any FVS counts: an undefined one matches no rule, and the letter falls back to its first
/// form (EAC XIM11-46: `e n fvs3 mvs e` → `A A Fvs3 _ Aa`). B.29 (j) and B.32 (w) write no
/// such rule, so the FVS clause applies and only a valid FVS counts: `a j fvs3 mvs a` takes the
/// onset `I`, as in the font. The font counts only a valid FVS for every letter; mongfontbuilder
/// marks XIM11-46 xfail.
fn iii2c_chachlag_onset_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter {
        return;
    }
    let fvs_blocks = match tok.alias {
        Some(Alias::J | Alias::W) => shaper.has_valid_fvs(&tokens[index]),
        _ => tok.has_fvs,
    };
    if fvs_blocks {
        return;
    }
    let Some(next) = next_tok(tokens, index) else {
        return;
    };
    if !tokens[next].is_mvs() {
        return;
    }
    let Some(after) = letter_after_mvs(tokens, next) else {
        return;
    };
    let after = view(tokens, after);
    // An FVS on the vowel postpones the MVS shaping. One on the MVS itself does not, because
    // the font's reset reads the vowel right after the MVS: `g mvs fvs3 a fvs1` keeps the onset.
    if after.position != Position::Isol || (after.has_fvs && !tokens[next].has_fvs()) {
        return;
    }
    if matches!(tok.alias, Some(Alias::N | Alias::J | Alias::W))
        && matches!(after.alias, Some(Alias::A | Alias::E))
    {
        set(tokens, index, Condition::ChachlagOnset);
        return;
    }
    if matches!(tok.alias, Some(Alias::H | Alias::G)) && after.alias == Some(Alias::A) {
        set(tokens, index, Condition::ChachlagOnset);
        return;
    }
    if tok.alias == Some(Alias::G) && after.alias == Some(Alias::E) {
        // GB variant: the data exposes this as a distinct condition.
        set(tokens, index, Condition::ChachlagOnsetGb);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2e — n/t/d: `onset` before a vowel, `devsger` after a vowel
//   (`t + ee` is owned by III.2g.t.devsger and skipped here; IgnoreMarks → FVS does not block)
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2e_n_t_d_onset_devsger => iii2e_n_t_d_onset_devsger_at);

/// The next letter before the segment ends at an MVS; ZWJ and nirugu are transparent.
fn next_letter_in_segment(tokens: &[Token], index: usize) -> Option<usize> {
    tokens[index + 1..]
        .iter()
        .take_while(|token| !token.is_mvs())
        .position(Token::is_letter)
        .map(|offset| index + 1 + offset)
}

/// The letter right after the MVS at `mvs`. A second MVS in between does not count: the font
/// matches `mvs` + letter as neighbours, so `g nnbsp nnbsp a` is no chachlag context for g.
fn letter_after_mvs(tokens: &[Token], mvs: usize) -> Option<usize> {
    next_tok(tokens, mvs).filter(|&next| tokens[next].is_letter())
}

/// n/t/d: `onset` before a vowel, `devsger` after one — see
/// rules.py::_iii2e_n_t_d_onset_devsger_at.
///
/// A medial one with no letter after it in its segment (only a ZWJ or a nirugu, then the word
/// end or an MVS) takes neither: it closes no syllable. The font's devsger needs a following
/// consonant, and GB/T 25914-2023 B.26 gives `t + NIRUGU` the first form, so `a t zwj` is
/// `A A D Zwj`, not `A A T Zwj` (n and d look the same either way: their devsger form is
/// their default).
///
/// Both vowels must be in the letter's own segment: `a d nirugu mvs a` keeps the default `Dd`,
/// not the onset `D`, as in the font.
fn iii2e_n_t_d_onset_devsger_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || !matches!(tok.alias, Some(Alias::N | Alias::T | Alias::D)) {
        return;
    }
    if let Some(next) = next_letter_in_segment(tokens, index) {
        // Carve-out: III.2g.t.devsger owns `t + ee`. In iii.py it overwrites this lookup's
        // substitution through OpenType class membership, which first-writer-wins cannot
        // replicate, so III.2e bows out and falls through to the devsger check below. It is
        // t-only: `d + ee` still goes `onset` — see rules.py::_iii2e_n_t_d_onset_devsger_at.
        if shaper.is_vowel(&tokens[next])
            && !(tok.alias == Some(Alias::T) && tokens[next].alias == Some(Alias::Ee))
        {
            set(tokens, index, Condition::Onset);
            return;
        }
    }
    if tok.position == Position::Medi && next_letter_in_segment(tokens, index).is_none() {
        return;
    }
    if let Some(prev) = prev_adjacent_letter(tokens, index) {
        if shaper.is_vowel(&tokens[prev]) {
            set(tokens, index, Condition::Devsger);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2f — h/g harmony: (a) adjacent vowel, (b) `i + g/h` with a reachable MASC marker,
//   (c) g.init/h.init + consonant → feminine. Adjacency stops at MVS (nirugu transparent).
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2f_h_g_harmony => iii2f_h_g_harmony_at);

/// h/g harmony in three layers: (a) the adjacent vowel decides, (b) `i + g/h` with a reachable
/// MASC marker, (c) g.init/h.init + consonant → feminine — see rules.py::_iii2f_h_g_harmony_at.
/// None of them applies before an MVS and an isolated a/e (`precedes_mvs_and_isolated_a_e`).
///
/// Doc/implementation discrepancy: `hudum.mdx` describes four further "remotely follows /
/// remotely precedes" harmony rules, but neither iii.py nor the .fea implements them — the font
/// renders `o l g` as `G`, not `H`. Layer (b) is the only non-adjacent mechanism, and it is
/// strictly gated on prev = `i`.
///
/// Adjacency is deliberately strict: iii2f runs with `IgnoreMarks` (FVS invisible) but MVS is a
/// base glyph that breaks adjacency, hence [`prev_adjacent_letter`]/[`next_adjacent_letter`].
/// Do NOT widen them to scan past an MVS: an earlier unconstrained scan matched vowels across
/// MVS word boundaries and caused the `o l g → H` regression.
fn iii2f_h_g_harmony_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter
        || !matches!(tok.alias, Some(Alias::H | Alias::G))
        || shaper.has_valid_fvs(&tokens[index])
    {
        return;
    }
    if precedes_mvs_and_isolated_a_e(tokens, index) {
        return;
    }
    let next = next_adjacent_letter(tokens, index);
    let prev = prev_adjacent_letter(tokens, index);

    // ── (a) the adjacent vowel decides ──
    if let Some(next) = next {
        if shaper.is_masc_vowel(&tokens[next]) {
            set(tokens, index, Condition::MasculineOnset);
            return;
        }
        if shaper.is_fem_vowel(&tokens[next]) || shaper.is_neut_vowel(&tokens[next]) {
            set(tokens, index, Condition::Feminine);
            return;
        }
    }
    if let Some(prev) = prev {
        if shaper.is_masc_vowel(&tokens[prev]) {
            set(tokens, index, Condition::MasculineDevsger);
            return;
        }
        if shaper.is_fem_vowel(&tokens[prev]) {
            set(tokens, index, Condition::Feminine);
            return;
        }
    }

    // ── (b) i + g/h with a reachable MASC marker (iii2f.A) ──
    if let Some(prev) = prev {
        if tokens[prev].alias == Some(Alias::I) {
            if shaper.masc_marker_reaches_g_h(tokens, index) {
                set(tokens, index, Condition::MasculineDevsger);
                return;
            }
            if tok.alias == Some(Alias::G) {
                // iii2f.A pattern 6: g only, not h
                set(tokens, index, Condition::Feminine);
                return;
            }
        }
    }

    // ── (c) g.init/h.init + consonant (iii2f.B) ──
    if tok.position == Position::Init {
        if let Some(next) = next {
            if shaper.is_consonant(&tokens[next]) {
                set(tokens, index, Condition::Feminine);
            }
        }
    }
    // No layer fired — the resolver uses the default variant.
}

/// g/h + MVS + isolated a/e, with any joiners before the MVS: the context III.2c's
/// `chachlag_onset` owns. The font's gender lookups (`III.k_g_h.onset_and_devsger_and_gender`,
/// `III.g_h.onset_and_devsger_and_gender.A`) open with an ignore rule for it, so no gender
/// applies there. When III.2c does not fire either, g/h keeps its default form:
/// - with an FVS on the vowel (see `iii2c_chachlag_onset_at`), `a g mvs a fvs1` is
///   `A A G Mvs A`;
/// - with a joiner before the MVS, `a g nirugu mvs a` is `A A G Nirugu Mvs Aa`.
///
/// Followed for consistency with the font, not reported upstream. With the FVS, the wide MVS
/// ends the word, and GB/T 25914-2023 B.21 (final No. 1 ①, `[MCS] + [GA] {Masc}`) would give a
/// masculine g its final form `H`, as the font does before an MVS and a consonant (`a g mvs n`).
/// If upstream starts doing that, drop the FVS case here. `tests/font_vs_engine.rs` watches it.
fn precedes_mvs_and_isolated_a_e(tokens: &[Token], index: usize) -> bool {
    let Some(mvs) = (index + 1..tokens.len()).find(|&j| !tokens[j].is_nirugu()) else {
        return false;
    };
    if !tokens[mvs].is_mvs() {
        return false;
    }
    let Some(vowel) = letter_after_mvs(tokens, mvs) else {
        return false;
    };
    let vowel = view(tokens, vowel);
    matches!(vowel.alias, Some(Alias::A | Alias::E)) && vowel.position == Position::Isol
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2g — t devsger / sh dotless / g dotless
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2g_t_devsger => iii2g_t_devsger_at);

/// t before `ee` or a consonant → `devsger` (it also owns the `t + ee` case III.2e skips) —
/// see rules.py::_iii2g_t_devsger_at. The consonant must be in t's own segment: the font's
/// lookup does not see past an MVS, so `a t nirugu mvs l a` keeps the default `D`.
fn iii2g_t_devsger_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::T) || shaper.has_valid_fvs(&tokens[index]) {
        return;
    }
    let Some(next) = next_letter_in_segment(tokens, index) else {
        return;
    };
    if tokens[next].alias == Some(Alias::Ee) || shaper.is_consonant(&tokens[next]) {
        set(tokens, index, Condition::Devsger);
    }
}

per_token!(iii2g_sh_dotless => iii2g_sh_dotless_at);

/// sh before i → `dotless` (the dot would collide with the following i) — see
/// rules.py::_iii2g_sh_dotless_at.
fn iii2g_sh_dotless_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::Sh) || shaper.has_valid_fvs(&tokens[index]) {
        return;
    }
    let Some(next) = next_letter_in_segment(tokens, index) else {
        return;
    };
    let next = view(tokens, next);
    if next.alias != Some(Alias::I) {
        return;
    }
    if tok.position == Position::Init && next.position == Position::Medi {
        set(tokens, index, Condition::Dotless);
        return;
    }
    if tok.position == Position::Medi {
        set(tokens, index, Condition::Dotless);
    }
}

per_token!(iii2g_g_dotless => iii2g_g_dotless_at);

/// `s/d.medi + g.medi + masc vowel` and `s/d + g.fina + MVS + chachlag a.isol` → `dotless`.
/// OVERRIDES any earlier condition (no first-writer guard): in iii.py the same effect falls out
/// of OpenType class membership, which rewrites the glyph III.2c/III.2f already substituted —
/// see rules.py::_iii2g_g_dotless_at.
///
/// GB/T 25914-2023 Table B.21 writes the two cases differently. Before a vowel (medial No. 1 ②)
/// the s/d follows `[ExtMCS]`, at least one character, so after a word-initial s/d the g keeps
/// the dotted form of No. 2 ①: `s g u s` is `S Hx O S`. Before an MVS (No. 6 ①) the s/d follows
/// `[ExtMCS | WB]`, so a word-initial one counts there. mongfontbuilder reads it the same way
/// since c695f96 (Kushim-Jiang/mongolian#61).
fn iii2g_g_dotless_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if !tok.is_letter || tok.alias != Some(Alias::G) || shaper.has_valid_fvs(&tokens[index]) {
        return;
    }
    let Some(prev) = prev_adjacent_letter(tokens, index) else {
        return;
    };
    if !matches!(tokens[prev].alias, Some(Alias::S | Alias::D)) {
        return;
    }
    // Rule (1): medial s/d + g.medi + the next letter in g's segment a masc vowel
    if tok.position == Position::Medi && tokens[prev].position == Position::Medi {
        if let Some(next) = next_letter_in_segment(tokens, index) {
            if shaper.is_masc_vowel(&tokens[next]) {
                set(tokens, index, Condition::Dotless);
                return;
            }
        }
    }
    // Rule (2): g.fina + MVS + chachlag a.isol. The chachlag check encodes iii.py's literal
    // `u1820.Aa.isol` context, which the vowel shows from III.1 on even before an FVS (a valid
    // FVS picks its variant only at the end, in `III.fvs.MNG`), so `s g mvs a fvs1` is dotless
    // too, as in the font — see rules.py::_iii2g_g_dotless_at. That rests on an upstream
    // inconsistency documented at `iii1_chachlag_at`, which says what changes if it is fixed.
    if tok.position == Position::Fina {
        if let Some(next) = next_tok(tokens, index) {
            if tokens[next].is_mvs() {
                if let Some(after) = letter_after_mvs(tokens, next) {
                    let after = view(tokens, after);
                    if after.alias == Some(Alias::A)
                        && after.position == Position::Isol
                        && after.condition == Some(Condition::Chachlag)
                    {
                        set(tokens, index, Condition::Dotless);
                    }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.3 — Particle: MVS-headed segments matched against the particle dictionary
// ═══════════════════════════════════════════════════════════════════════

/// Match each MVS-headed segment against the particle dictionary; the hit indices take
/// `particle`, for the alias subset that actually has a particle variant in the data — see
/// rules.py::_iii3_particle.
fn iii3_particle(tokens: &mut [Token], shaper: &Shaper) {
    for (syms, indices) in build_mvs_segments(tokens) {
        // Try the segment as-is first, then (for mvs-headed segments) with mvs stripped —
        // entries like `u u` / `b ue ue` match anywhere (iii.py:785).
        let mut particle_indices = shaper.particle(&syms);
        let mut used: &[usize] = &indices;
        if particle_indices.is_none() && syms.first() == Some(&ParticleSym::Mvs) {
            particle_indices = shaper.particle(&syms[1..]);
            used = &indices[1..];
        }
        let Some(particle_indices) = particle_indices else {
            continue;
        };
        // UseMarkFilteringSet @fvs: any FVS in the matched run breaks base-glyph adjacency:
        // one right after its MVS (`mvs fvs2 i` is no particle: `Mvs A I`), and one carried
        // by a joiner between its letters, which the lookup skips (`u zwj fvs4 u`).
        let (Some(&first), Some(&last)) = (used.first(), used.last()) else {
            continue;
        };
        if tokens[first..=last].iter().any(Token::has_fvs) {
            continue;
        }
        // The lookup keys on position-specific classes, the particle's letters filling their
        // segment: a lone letter isol, else init, medi…, fina. A nirugu or ZWJ before the
        // first letter or after the last shifts it off that position and the particle no
        // longer fires (`mvs u nirugu` is `Mvs A O Nirugu`).
        let letters: Vec<usize> = used
            .iter()
            .copied()
            .filter(|&idx| tokens[idx].is_letter())
            .collect();
        let expected = |k: usize| match (k, letters.len()) {
            (_, 1) => Position::Isol,
            (0, _) => Position::Init,
            (k, n) if k + 1 == n => Position::Fina,
            _ => Position::Medi,
        };
        if letters
            .iter()
            .enumerate()
            .any(|(k, &idx)| tokens[idx].position != expected(k))
        {
            continue;
        }
        for &pidx in particle_indices {
            if pidx >= used.len() {
                continue; // defensive; never expected to fire
            }
            let token = &mut tokens[used[pidx]];
            if token.is_letter()
                && matches!(
                    token.alias,
                    Some(
                        Alias::A | Alias::E | Alias::I | Alias::U | Alias::Ue | Alias::D | Alias::Y
                    )
                )
            {
                token.condition = Some(Condition::Particle);
            }
        }
    }
}

/// `(symbols, token indices)` per MVS-headed segment; nirugu is skipped, and a letter without an
/// alias becomes `ParticleSym::Unknown` (never matches, exactly like Python's empty alias) —
/// see rules.py::_build_mvs_segments.
fn build_mvs_segments(tokens: &[Token]) -> Vec<(Vec<ParticleSym>, Vec<usize>)> {
    let mut segments = Vec::new();
    let mut syms: Vec<ParticleSym> = Vec::new();
    let mut indices: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.is_mvs() {
            if !syms.is_empty() {
                segments.push((std::mem::take(&mut syms), std::mem::take(&mut indices)));
            }
            syms.push(ParticleSym::Mvs);
            indices.push(i);
        } else if token.is_letter() {
            syms.push(token.alias.map_or(ParticleSym::Unknown, ParticleSym::Alias));
            indices.push(i);
        }
    }
    if !syms.is_empty() {
        segments.push((syms, indices));
    }
    segments
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.4 — Devsger: medial i after a vowel whose shape does not already end in `I`
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii4_vowel_devsger => iii4_vowel_devsger_at);

/// A medial `i` after a vowel whose shape does not already end in `I` takes the double-tooth
/// `vowel_devsger` form — see rules.py::_iii4_vowel_devsger_at. The vowel must be in the i's own
/// segment: `e nnbsp zwj i ue` keeps the single `I`, as in the font.
fn iii4_vowel_devsger_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::I) || tok.position != Position::Medi {
        return;
    }
    if shaper.has_valid_fvs(&tokens[index]) {
        return;
    }
    let Some(prev) = prev_adjacent_letter(tokens, index) else {
        return;
    };
    if !shaper.is_vowel(&tokens[prev]) {
        return;
    }
    // GB's table (`III.i.devsger.MNG.GB`, UseMarkFilteringSet @fvs) reads the one FVS between an
    // oe/ue and the i, on the vowel or on a joiner: after a medial oe/ue, FVS3 gives the double
    // tooth and FVS1/FVS2 the single one; after an initial ue, FVS1 gives the double tooth and
    // FVS2 the single one. So `ue nirugu fvs1 i e ue` has `I I`, like `ue fvs1 i e ue`.
    let oe_ue = matches!(tokens[prev].alias, Some(Alias::Oe | Alias::Ue));
    if let Some(fvs) = single_fvs_between(tokens, prev, index).filter(|_| oe_ue) {
        let ue_init =
            tokens[prev].alias == Some(Alias::Ue) && tokens[prev].position == Position::Init;
        let devsger = match (tokens[prev].position, fvs) {
            (Position::Medi, Fvs::Fvs3) => Some(true),
            (Position::Medi, Fvs::Fvs1 | Fvs::Fvs2) => Some(false),
            (Position::Init, Fvs::Fvs1) if ue_init => Some(true),
            (Position::Init, Fvs::Fvs2) if ue_init => Some(false),
            _ => None,
        };
        if let Some(devsger) = devsger {
            if devsger {
                set(tokens, index, Condition::VowelDevsger);
            }
            return;
        }
    }
    // Otherwise the vowel's form decides, as the font sees it before any FVS applies: with two
    // FVS GB's table matches nothing, and `oe fvs2 fvs1 i` has the double tooth of a bare `oe`.
    if shaper.contextual_written(&tokens[prev]).last() == Some(&WrittenUnit::I) {
        return; // already ends in I, no double tooth needed
    }
    set(tokens, index, Condition::VowelDevsger);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.5 — Post-bowed: a final vowel after a bowed consonant. OVERRIDES earlier conditions
//   (notably particle) but defers to `marked`; FVS on the vowel does not block (IgnoreMarks).
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii5_post_bowed => iii5_post_bowed_at);

/// A final vowel after a bowed consonant takes `post_bowed`: a letter ending in `B`, `P`, `F`,
/// `K` or `K2` (b, p, f, k, k2), or one written as nothing but `G`/`Gx` (g, h). Like
/// III.2g.g.dotless it OVERRIDES earlier conditions (notably III.3's `particle`), but it defers
/// to `marked`, mirroring iii.py's leading ignore pattern for the post-marked oe/ue.fina glyphs —
/// see rules.py::_iii5_post_bowed_at.
///
/// ŋ (`A G`) ends in the same `G` but is no bowed consonant: mongfontbuilder's class is
/// `writtens("MNG", ["G", "Gx"])`, the whole written form, and GB/T 25914-2023 (3.21–3.23)
/// lists ŋ as a non-combining consonant (NCC), after which a final vowel keeps its first form
/// (`[NCC | Vow | NIRUGU] + e` → `A`, `[ExtMCS] + [Vow | NCC | MCCC] + o` → `U`).
///
/// The bowed consonant must be in the vowel's own segment: `b mvs nirugu o` is
/// `B Mvs Nirugu U`, as in the font.
fn iii5_post_bowed_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition == Some(Condition::Marked) {
        return;
    }
    if !tok.is_letter {
        return;
    }
    if !matches!(
        tok.alias,
        Some(Alias::O | Alias::U | Alias::Oe | Alias::Ue | Alias::A | Alias::E)
    ) {
        return;
    }
    if tok.position != Position::Fina {
        return;
    }
    let Some(prev) = prev_adjacent_letter(tokens, index) else {
        return;
    };
    shaper.resolve_written(&mut tokens[prev]);
    let Some(written) = tokens[prev].written else {
        return;
    };
    let Some(&last_unit) = written.last() else {
        return;
    };
    let prev = view(tokens, prev);
    let is_bowed_g = matches!(written, [WrittenUnit::G] | [WrittenUnit::Gx]);
    let is_bowed_bk = matches!(
        last_unit,
        WrittenUnit::B | WrittenUnit::P | WrittenUnit::F | WrittenUnit::K | WrittenUnit::K2
    );
    if !(is_bowed_g || is_bowed_bk) {
        return;
    }
    // bowedG + a does NOT fire post_bowed (`h fvs2 a → G A`).
    if is_bowed_g && tok.alias == Some(Alias::A) {
        return;
    }
    // ── iii5.GB rules (FVS-aware) ──
    if let Some(fvs) = prev.first_fvs {
        // Rule E: g/h + fvs2/fvs4 + [o,u].fina → reset (suppress post_bowed).
        if is_bowed_g
            && matches!(prev.alias, Some(Alias::G | Alias::H))
            && matches!(fvs, Fvs::Fvs2 | Fvs::Fvs4)
            && matches!(tok.alias, Some(Alias::O | Alias::U))
        {
            return;
        }
        // Rule C: bowedB/bowedK.init + fvs + [oe,ue].fina → MARKED (not post_bowed).
        if is_bowed_bk
            && prev.position == Position::Init
            && matches!(tok.alias, Some(Alias::Oe | Alias::Ue))
        {
            set(tokens, index, Condition::Marked);
            return;
        }
        // iii.py's remaining GB rules need no code here: B/D (h/g + fvs1/3 → reset) cannot
        // trigger because those render `Hx`/`H`, which are not bowed, so the main rule never
        // fires; G (h/g.init + fvs2/4 + oe/ue → marked) is already covered by GB.B, which takes
        // the h/g's first FVS as well (`iii2a_o_u_oe_ue_marked_gb_b_at`) — see
        // rules.py::_iii5_post_bowed_at.
    }
    set(tokens, index, Condition::PostBowed);
}

// ═══════════════════════════════════════════════════════════════════════
// Locale rule tables
// ═══════════════════════════════════════════════════════════════════════

/// Hudum rule table, in mongfontbuilder `iii.py` order (frozen by the phase-trace golden).
pub(crate) static RULES_MNG: &[Rule] = &[
    Rule {
        name: "III.1.chachlag",
        apply: iii1_chachlag,
    },
    Rule {
        name: "III.2a.o_u_oe_ue.marked",
        apply: iii2a_o_u_oe_ue_marked,
    },
    Rule {
        name: "III.2a.o_u_oe_ue.marked.GB.A",
        apply: iii2a_o_u_oe_ue_marked_gb_a,
    },
    Rule {
        name: "III.2a.o_u_oe_ue.marked.GB.B",
        apply: iii2a_o_u_oe_ue_marked_gb_b,
    },
    Rule {
        name: "III.2a.oe_ue.cluster.marked",
        apply: iii2a_oe_ue_cluster_marked,
    },
    Rule {
        name: "III.2a.d.marked",
        apply: iii2a_d_marked,
    },
    Rule {
        name: "III.2c.chachlag_onset",
        apply: iii2c_chachlag_onset,
    },
    Rule {
        name: "III.2e.n_t_d.onset_devsger",
        apply: iii2e_n_t_d_onset_devsger,
    },
    Rule {
        name: "III.2f.h_g.harmony",
        apply: iii2f_h_g_harmony,
    },
    Rule {
        name: "III.2g.t.devsger",
        apply: iii2g_t_devsger,
    },
    Rule {
        name: "III.2g.sh.dotless",
        apply: iii2g_sh_dotless,
    },
    Rule {
        name: "III.2g.g.dotless",
        apply: iii2g_g_dotless,
    },
    Rule {
        name: "III.3.particle",
        apply: iii3_particle,
    },
    Rule {
        name: "III.4.vowel_devsger",
        apply: iii4_vowel_devsger,
    },
    Rule {
        name: "III.5.post_bowed",
        apply: iii5_post_bowed,
    },
];
