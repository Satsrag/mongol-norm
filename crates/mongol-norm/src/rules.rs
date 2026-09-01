//! Declarative shaping rules for Traditional Mongolian — a 1:1 port of `mongol_norm/rules.py`,
//! which itself mirrors mongfontbuilder's OpenType pipeline (`otl/iii.py`): one function per
//! Lookup, same name, same intent, same order.
//!
//! Every rule is *first-writer-wins* (`if tok.condition.is_some() { return }`) except
//! `III.2g.g.dotless` and `III.5.post_bowed`, which override earlier conditions to mirror
//! OpenType class-membership semantics (see the Python comments for the font verification).

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::shaper::{
    next_adjacent_letter, next_letter, next_tok, prev_adjacent_letter, prev_letter, prev_tok,
    Shaper,
};
use crate::tables::{Fvs, Locale, ParticleSym, Position};
use crate::token::Token;

/// A single shaping rule = a single mongfontbuilder Lookup.
pub(crate) struct Rule {
    /// The Lookup name, e.g. `III.2f.h_g.harmony` (frozen by the phase-trace golden).
    pub name: &'static str,
    /// Mutates the tokens' `condition` fields in place.
    pub apply: fn(&mut [Token], &Shaper),
}

/// The rule table of a locale (only MNG has rules, exactly as in Python).
pub(crate) fn rules_for(locale: Locale) -> &'static [Rule] {
    match locale {
        Locale::Mng => RULES_MNG,
        Locale::Tod | Locale::Sib | Locale::Mch => &[],
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
// Phase III.1 — Chachlag: isolated a/e after MVS; an FVS on the vowel cancels (GB).
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii1_chachlag => iii1_chachlag_at);

/// Isolated a/e directly after an MVS take `chachlag`; a GB carve-out lets an FVS on the vowel
/// cancel it so the MVS shaping is postponed to III.3 — see rules.py::_iii1_chachlag_at.
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
    if tok.has_fvs {
        return; // GB: FVS → postpone to the particle step
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
    let Some(prev) = prev_letter(tokens, index) else {
        return;
    };
    if !(shaper.is_consonant(&tokens[prev]) && tokens[prev].position == Position::Init) {
        return;
    }
    set(tokens, index, Condition::Marked);
}

per_token!(iii2a_o_u_oe_ue_marked_gb_a => iii2a_o_u_oe_ue_marked_gb_a_at);

/// GB.A: an FVS directly before or after the vowel resets `marked` — observed only on
/// fina-position vowels (verified against DraftNew-Regular.otf: `h fvs3 oe` resets, `h fvs3 oe l`
/// stays marked), so the position gate is load-bearing — see
/// rules.py::_iii2a_o_u_oe_ue_marked_gb_a_at.
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
    if tok.position != Position::Fina {
        return;
    }
    if tok.has_fvs {
        tokens[index].condition = None;
        return;
    }
    if let Some(prev) = prev_letter(tokens, index) {
        if tokens[prev].has_fvs() {
            tokens[index].condition = None;
        }
    }
}

per_token!(iii2a_o_u_oe_ue_marked_gb_b => iii2a_o_u_oe_ue_marked_gb_b_at);

/// GB.B: the single exception to GB.A — initial h/g carrying FVS2/FVS4 + final oe/ue stays
/// `marked` even though an FVS sits between them — see rules.py::_iii2a_o_u_oe_ue_marked_gb_b_at.
fn iii2a_o_u_oe_ue_marked_gb_b_at(tokens: &mut [Token], index: usize, _shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::Oe | Alias::Ue)) || tok.position != Position::Fina {
        return;
    }
    let Some(prev) = prev_letter(tokens, index) else {
        return;
    };
    let prev = view(tokens, prev);
    if !matches!(prev.alias, Some(Alias::H | Alias::G)) || prev.position != Position::Init {
        return;
    }
    if !matches!(prev.first_fvs, Some(Fvs::Fvs2 | Fvs::Fvs4)) {
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
/// init + ≥1 medi consonant chain. Verified against DraftNew-Regular.otf: `g ue l` leaves ue
/// default, `g g ue l` marks it — see rules.py::_iii2a_oe_ue_cluster_marked_at.
fn iii2a_oe_ue_cluster_marked_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter {
        return;
    }
    if !matches!(tok.alias, Some(Alias::O | Alias::U | Alias::Oe | Alias::Ue)) {
        return;
    }
    if tok.position != Position::Medi || tok.has_fvs {
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
    let Some(next) = next_letter(tokens, index) else {
        return;
    };
    if !(shaper.is_vowel(&tokens[next]) && tokens[next].position == Position::Fina) {
        return;
    }
    if tokens[next].has_fvs() {
        return; // GB: FVS on the following vowel also cancels
    }
    set(tokens, index, Condition::Marked);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2c — chachlag_onset for n/j/w/h/g before MVS + isolated a/e
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2c_chachlag_onset => iii2c_chachlag_onset_at);

/// n/j/w before MVS + isolated a/e, and h/g before MVS + isolated a, take `chachlag_onset`;
/// `g` before MVS + isolated e takes the GB-specific condition instead — see
/// rules.py::_iii2c_chachlag_onset_at.
fn iii2c_chachlag_onset_at(tokens: &mut [Token], index: usize, _shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() || !tok.is_letter || tok.has_fvs {
        return;
    }
    let Some(next) = next_tok(tokens, index) else {
        return;
    };
    if !tokens[next].is_mvs() {
        return;
    }
    let Some(after) = next_letter(tokens, next) else {
        return;
    };
    let after = view(tokens, after);
    if after.position != Position::Isol {
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

/// n/t/d: `onset` before a vowel, `devsger` after one — see
/// rules.py::_iii2e_n_t_d_onset_devsger_at.
fn iii2e_n_t_d_onset_devsger_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || !matches!(tok.alias, Some(Alias::N | Alias::T | Alias::D)) {
        return;
    }
    if let Some(next) = next_letter(tokens, index) {
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
    if let Some(prev) = prev_letter(tokens, index) {
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
///
/// Doc/implementation discrepancy: `hudum.mdx` describes four further "remotely follows /
/// remotely precedes" harmony rules, but neither iii.py nor the .fea implements them — the font
/// renders `o l g` as `G`, not `H`. Layer (b) is the only non-adjacent mechanism, and it is
/// strictly gated on prev = `i`.
///
/// Adjacency is deliberately strict: iii2f runs with `IgnoreMarks` (FVS invisible) but MVS is a
/// base glyph that breaks adjacency, hence [`prev_adjacent_letter`]/[`next_adjacent_letter`].
/// Do NOT "simplify" these to `prev_letter`/`next_letter`: an earlier unconstrained scan matched
/// vowels across MVS word boundaries and caused the `o l g → H` regression.
fn iii2f_h_g_harmony_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || !matches!(tok.alias, Some(Alias::H | Alias::G)) || tok.has_fvs {
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

// ═══════════════════════════════════════════════════════════════════════
// Phase III.2g — t devsger / sh dotless / g dotless
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii2g_t_devsger => iii2g_t_devsger_at);

/// t before `ee` or a consonant → `devsger` (it also owns the `t + ee` case III.2e skips) —
/// see rules.py::_iii2g_t_devsger_at.
fn iii2g_t_devsger_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::T) || tok.has_fvs {
        return;
    }
    let Some(next) = next_letter(tokens, index) else {
        return;
    };
    if tokens[next].alias == Some(Alias::Ee) || shaper.is_consonant(&tokens[next]) {
        set(tokens, index, Condition::Devsger);
    }
}

per_token!(iii2g_sh_dotless => iii2g_sh_dotless_at);

/// sh before i → `dotless` (the dot would collide with the following i) — see
/// rules.py::_iii2g_sh_dotless_at.
fn iii2g_sh_dotless_at(tokens: &mut [Token], index: usize, _shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::Sh) || tok.has_fvs {
        return;
    }
    let Some(next) = next_letter(tokens, index) else {
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

/// `s/d + g.medi + masc vowel` and `s/d + g.fina + MVS + chachlag a.isol` → `dotless`.
/// OVERRIDES any earlier condition (no first-writer guard): in iii.py the same effect falls out
/// of OpenType class membership, which rewrites the glyph III.2c/III.2f already substituted —
/// see rules.py::_iii2g_g_dotless_at.
fn iii2g_g_dotless_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if !tok.is_letter || tok.alias != Some(Alias::G) || tok.has_fvs {
        return;
    }
    let Some(prev) = prev_letter(tokens, index) else {
        return;
    };
    if !matches!(tokens[prev].alias, Some(Alias::S | Alias::D)) {
        return;
    }
    // Rule (1): g.medi + immediately following masc vowel
    if tok.position == Position::Medi {
        if let Some(next) = next_letter(tokens, index) {
            if shaper.is_masc_vowel(&tokens[next]) {
                set(tokens, index, Condition::Dotless);
                return;
            }
        }
    }
    // Rule (2): g.fina + MVS + chachlag a.isol. The chachlag check encodes iii.py's literal
    // `u1820.Aa.isol` context — the glyph name *after* III.1 fired. An FVS that cancelled
    // chachlag leaves the default `a.isol` glyph, which iii.py's rule would not match, so this
    // rule must not fire either — see rules.py::_iii2g_g_dotless_at.
    if tok.position == Position::Fina {
        if let Some(next) = next_tok(tokens, index) {
            if tokens[next].is_mvs() {
                if let Some(after) = next_letter(tokens, next) {
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
        // UseMarkFilteringSet @fvs: any FVS in the segment breaks base-glyph adjacency.
        if indices
            .iter()
            .any(|&idx| tokens[idx].is_letter() && tokens[idx].has_fvs())
        {
            continue;
        }
        // The lookup keys on position-specific classes: a nirugu between mvs and the first
        // letter shifts that letter off init/isol and the particle no longer fires.
        let first_letter = indices.iter().copied().find(|&idx| tokens[idx].is_letter());
        if let Some(first) = first_letter {
            if !matches!(tokens[first].position, Position::Init | Position::Isol) {
                continue;
            }
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
/// `vowel_devsger` form — see rules.py::_iii4_vowel_devsger_at.
fn iii4_vowel_devsger_at(tokens: &mut [Token], index: usize, shaper: &Shaper) {
    let tok = view(tokens, index);
    if tok.condition.is_some() {
        return;
    }
    if !tok.is_letter || tok.alias != Some(Alias::I) || tok.position != Position::Medi {
        return;
    }
    if tok.has_fvs {
        return;
    }
    let Some(prev) = prev_letter(tokens, index) else {
        return;
    };
    if !shaper.is_vowel(&tokens[prev]) {
        return;
    }
    shaper.resolve_written(&mut tokens[prev]);
    if tokens[prev].written_ends_with(WrittenUnit::I) {
        return; // already ends in I, no double tooth needed
    }
    set(tokens, index, Condition::VowelDevsger);
}

// ═══════════════════════════════════════════════════════════════════════
// Phase III.5 — Post-bowed: a final vowel after a bowed consonant. OVERRIDES earlier conditions
//   (notably particle) but defers to `marked`; FVS on the vowel does not block (IgnoreMarks).
// ═══════════════════════════════════════════════════════════════════════

per_token!(iii5_post_bowed => iii5_post_bowed_at);

/// A final vowel after a bowed consonant (`G`, `Gx`, `B`, `P`, `F`, `K`, `K2`) takes
/// `post_bowed`. Like III.2g.g.dotless it OVERRIDES earlier conditions (notably III.3's
/// `particle`), but it defers to `marked`, mirroring iii.py's leading ignore pattern for the
/// post-marked oe/ue.fina glyphs — see rules.py::_iii5_post_bowed_at.
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
    let Some(prev) = prev_letter(tokens, index) else {
        return;
    };
    shaper.resolve_written(&mut tokens[prev]);
    let Some(last_unit) = tokens[prev]
        .written
        .and_then(|written| written.last().copied())
    else {
        return;
    };
    let prev = view(tokens, prev);
    let is_bowed_g = matches!(last_unit, WrittenUnit::G | WrittenUnit::Gx);
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
        // fires; G (h/g.init + fvs2/4 + oe/ue → marked) is already covered by
        // [`iii2a_o_u_oe_ue_marked_gb_b_at`] — see rules.py::_iii5_post_bowed_at.
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
