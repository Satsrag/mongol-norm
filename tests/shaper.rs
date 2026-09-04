//! Hand-written shaping tests — a 1:1 port of `python/tests/test_shaper.py` (`TestShape`,
//! `TestSameShape`, and the tokenization/shape half of `TestNNBSP`). Test names and order follow
//! the Python module so the two files can be read side by side.

mod common;

use common::{mgl, unit_names};
use mongol_norm::{Alias, Condition, Locale, Position, Shaper, TokenDetail};

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

/// `shape()` as contract names, so expectations can be written as `&["S", "A", "I", "I", "A"]`.
#[track_caller]
fn shape(text: &str) -> Vec<String> {
    unit_names(
        &shaper()
            .shape(text)
            .unwrap_or_else(|e| panic!("shape({text:?}) failed: {e}")),
    )
}

#[track_caller]
fn assert_shape(text: &str, expected: &[&str]) {
    assert_eq!(
        shape(text),
        expected,
        "shape of {text:?} ({})",
        common::hex(text)
    );
}

/// `shape_raw()` as contract names.
///
/// The rule tests below pin *which UTN #57 rule fired*, and the only evidence for that is the
/// written unit the engine assigned. For `Dd` and medial `H` / `Hx` that evidence survives only
/// in the raw sequence — the public `shape` folds all three into `O A` / `A A` / `N N`, exactly
/// as `tests/eac_hud.rs` and `tests/core_hud.rs` do for the standard's own vectors. The public
/// shape of the same words is pinned in `duplicates_are_folded_out_of_the_public_shape`.
#[track_caller]
fn shape_raw(text: &str) -> Vec<String> {
    unit_names(
        &shaper()
            .shape_raw(text)
            .unwrap_or_else(|e| panic!("shape_raw({text:?}) failed: {e}")),
    )
}

#[track_caller]
fn assert_shape_raw(text: &str, expected: &[&str]) {
    assert_eq!(
        shape_raw(text),
        expected,
        "shape_raw of {text:?} ({})",
        common::hex(text)
    );
}

#[track_caller]
fn details(text: &str) -> Vec<TokenDetail> {
    shaper()
        .shape_detailed(text)
        .unwrap_or_else(|e| panic!("shape_detailed({text:?}) failed: {e}"))
}

/// The first token whose alias is `alias` (Python: `[d for d in details if d["alias"] == alias][0]`).
#[track_caller]
fn token_with_alias(text: &str, alias: Alias) -> TokenDetail {
    details(text)
        .into_iter()
        .find(|d| d.alias == Some(alias))
        .unwrap_or_else(|| panic!("no token with alias {alias} in {text:?}"))
}

/// Letter positions in order (Python: `[t.position for t in tokens if t.is_letter]`).
#[track_caller]
fn letter_positions(text: &str) -> Vec<Position> {
    details(text)
        .into_iter()
        .filter(|d| mongol_norm::is_mongolian_letter(d.cp))
        .map(|d| d.position)
        .collect()
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// TestShape — shape() returns the correct written-unit sequence.
// ════════════════════════════════════════════════════════════════════════════════════════════

// ── sain (good) — 5 encodings of the same word ──

#[test]
fn test_sain_base() {
    // Encoding 1: S + A + I + I + A (base form, simplest encoding).
    assert_shape(&mgl("s a i n"), &["S", "A", "I", "I", "A"]);
}

#[test]
fn test_sain_e_variant() {
    // Encoding 2: S + E + I + I + A (E instead of A — same glyph in medial position).
    assert_shape(&mgl("s e i n"), &["S", "A", "I", "I", "A"]);
}

#[test]
fn test_sain_na_fvs2() {
    // Encoding 3: S + N+FVS2 + I + I + N (N+FVS2 produces the 'A' glyph).
    assert_shape(&mgl("s n fvs2 i i n"), &["S", "A", "I", "I", "A"]);
}

#[test]
fn test_sain_ya_fvs1_i() {
    // Encoding 4: S + A + Y+FVS1 + I + N (Y+FVS1 produces a single 'I' tooth).
    assert_shape(&mgl("s a y fvs1 i n"), &["S", "A", "I", "I", "A"]);
}

#[test]
fn test_sain_ya_fvs1_ya_fvs1() {
    // Encoding 5: S + A + Y+FVS1 + Y+FVS1 + N (two Y+FVS1 = two teeth).
    assert_shape(&mgl("s a y fvs1 y fvs1 n"), &["S", "A", "I", "I", "A"]);
}

// ── Step 1 · Chachlag — MVS-triggered suffix forms ──

#[test]
fn test_step1_chachlag_tala() {
    // tal (t a l) + MVS + a -> chachlag "Aa"
    assert_shape(&mgl("t a l mvs a"), &["T", "A", "L", "Mvs", "Aa"]);
}

#[test]
fn test_step1_chachlag_talayin() {
    // Two MVS: chachlag on a, particle on y.
    assert_shape(
        &mgl("t a l mvs a mvs y i n"),
        &["T", "A", "L", "Mvs", "Aa", "Mvs", "I", "I", "A"],
    );
}

#[test]
fn test_step1_chachlag_mvs_a_fvs_default() {
    // a/e after MVS + FVS -> default (no chachlag): tal + MVS + a+FVS1.
    assert_shape(&mgl("t a l mvs a fvs1"), &["T", "A", "L", "Mvs", "A"]);
}

// ── Step 2 · Syllabic — consonant/vowel context rules ──

#[test]
fn test_step2_vowel_marked() {
    // o/u/oe/ue after initial consonant -> marked
    assert_shape(&mgl("ch u"), &["Ch", "O"]);
    assert_shape(&mgl("s ue"), &["S", "Ue"]);
    assert_shape(&mgl("d u"), &["D", "O"]);
    assert_shape(&mgl("d ue"), &["D", "Ue"]);
    assert_shape(&mgl("t ue l"), &["T", "O", "I", "L"]);
}

#[test]
fn test_step2_vowel_fvs_default() {
    // FVS on the vowel itself -> default.
    assert_shape(&mgl("h u fvs2"), &["H", "U"]);
    assert_shape(&mgl("r ue fvs3"), &["R", "U"]);
    // FVS on the preceding letter -> vowel also default.
    assert_shape(&mgl("d fvs1 ue"), &["D", "U"]);
    assert_shape(&mgl("d fvs1 u"), &["D", "U"]);
}

#[test]
fn test_step2_oe_ue_fina_hg_init_fvs_marked() {
    // Per GB/T 25914-2023 table E.3 (U+182C) / E.4 (U+182D): oe/ue.fina preceded by h/g AT
    // INIT POSITION with FVS2/FVS4 -> marked (exception). UTN #57 and the mongfontbuilder
    // docs (web/docs/hudum.mdx) describe this rule without the init constraint and with
    // reversed "precedes" wording — both are inaccurate.
    // See https://github.com/Kushim-Jiang/mongfontbuilder/issues/47
    assert_shape(&mgl("h fvs2 ue"), &["G", "Ue"]);
    assert_shape(&mgl("g fvs4 ue"), &["Gx", "Ue"]);
}

#[test]
fn test_step2_oe_ue_fina_hg_medi_fvs_default() {
    // Regression guard: h/g at MEDI position + FVS2/FVS4 does NOT trigger marked — must
    // render the default O form (matches real font output, e.g. enehue "this").
    assert_shape(&mgl("e n e h fvs2 ue"), &["A", "N", "A", "G", "O"]);
    assert_shape(&mgl("e n e g fvs4 ue"), &["A", "N", "A", "Gx", "O"]);
}

#[test]
fn test_step2_oe_marked_after_cc() {
    // mnoege (m n oe g e): oe after init m + medi n -> marked
    let oe = token_with_alias(&mgl("m n oe g e"), Alias::Oe);
    assert_eq!(oe.condition, Some(Condition::Marked));
    assert_shape(&mgl("m n oe g e"), &["M", "N", "O", "I", "G", "Aa"]);
}

#[test]
fn test_step2_d_marked() {
    // d.init + a.fina (no FVS) -> marked "D" (Twelve Syllabaries). Without the marked rule,
    // d.init default would be "T" (onset).
    assert_shape(&mgl("d a"), &["D", "A"]);
    // d.init + u.fina -> marked "D"; u.fina renders as "O" (post-bowed/marked context).
    assert_shape(&mgl("d u"), &["D", "O"]);
}

#[test]
fn test_step2_d_marked_gb_fvs_cancels() {
    // GB exception: an FVS adjacent to d or to the following vowel cancels d.marked
    // (mongolian/.fea III.eac.d.marked `ignore sub @d-hud.init @hud.vowel @fvs`).
    // FVS on the vowel -> marked bails -> d.init falls to default "T".
    assert_shape(&mgl("d a fvs1"), &["T", "Aa"]);
}

#[test]
fn test_step2_d_medi_intervocalic_onset() {
    // d.medi between vowels is NOT covered by d.marked (init-only); iii2e takes it instead
    // and assigns `onset` -> "D" (tooth form).
    assert_shape(&mgl("o d u"), &["A", "O", "D", "U"]);
    // d.fina (no following vowel) keeps its devsger default "Dd" (public shape: `A O O A`).
    assert_shape_raw(&mgl("o d"), &["A", "O", "Dd"]);
}

#[test]
fn test_step2_chachlag_onset_n() {
    // n before MVS + isolated a -> chachlag_onset "N"
    assert_shape(
        &mgl("s a i n mvs a"),
        &["S", "A", "I", "I", "N", "Mvs", "Aa"],
    );
    // w before MVS + isolated a -> chachlag_onset "U"
    assert_shape(&mgl("h o r w mvs a"), &["H", "O", "R", "U", "Mvs", "Aa"]);
    // j before MVS + isolated a -> chachlag_onset "I"
    assert_shape(&mgl("j mvs a"), &["I", "Mvs", "Aa"]);
    // j.fina before MVS + isolated e -> chachlag_onset "I"
    assert_shape(&mgl("e j mvs e"), &["A", "I", "Mvs", "Aa"]);
}

#[test]
fn test_step2_chachlag_onset_g_a() {
    // g before MVS + isolated a -> chachlag_onset
    assert_shape(
        &mgl("y a b u g mvs a"),
        &["Y", "A", "B", "O", "Hx", "Mvs", "Aa"],
    );
    // h before MVS + isolated a -> chachlag_onset
    assert_shape(&mgl("h a b h mvs a"), &["H", "A", "B", "H", "Mvs", "Aa"]);
}

#[test]
fn test_step2_chachlag_onset_g_e() {
    // g before MVS + isolated e -> chachlag_onset
    assert_shape(&mgl("e g mvs e"), &["A", "H", "Mvs", "Aa"]);
}

#[test]
fn test_step2_n_onset() {
    // n/d before vowel -> onset; n/t/d after vowel or before consonant -> devsger. (t has no
    // observable onset variant in MNG.json — iii2e still tags it but the resolver falls back
    // to default, so onset is a visual no-op for t.)
    assert_eq!(shape(&mgl("a n a r"))[2], "N"); // n before vowel -> onset
    assert_eq!(shape(&mgl("d a l a"))[0], "T"); // d.init before vowel -> onset
    let out = shape(&mgl("a n d a"));
    assert_eq!(out[3], "D"); // d.medi onset (before vowel)
    assert_eq!(out[2], "A"); // n.medi devsger (next is consonant d)
    assert_eq!(shape(&mgl("b a n"))[2], "A"); // n.fina after vowel -> devsger
    let out = shape_raw(&mgl("d a d g mvs a"));
    assert_eq!(out[2], "Dd"); // d.medi after vowel, before consonant g -> devsger
    let out = shape_raw(&mgl("a t d"));
    assert_eq!(out[2], "T"); // t.medi after vowel a, before consonant d -> devsger
    assert_eq!(out[3], "Dd"); // d.fina devsger (default)
}

#[test]
fn test_step2_h_masculine_onset() {
    // h(QA) before masculine vowel a -> masculine_onset "H"
    assert_shape(&mgl("h a s"), &["H", "A", "S"]);
}

#[test]
fn test_step2_g_masculine_onset() {
    // g(GA) before masculine vowel a -> masculine_onset "Hx"
    assert_shape(&mgl("g a r"), &["Hx", "A", "R"]);
}

#[test]
fn test_step2_g_feminine() {
    // g(GA) before feminine vowel e -> feminine "G"
    assert_shape(&mgl("g e r"), &["G", "A", "R"]);
}

#[test]
fn test_step2_g_masculine_devsger() {
    // g(GA) after masculine vowel a -> masculine_devsger "H"
    assert_shape(&mgl("a g"), &["A", "A", "H"]);
}

#[test]
fn test_step2_g_feminine_after_fem() {
    // g(GA) after feminine vowel oe -> feminine "G"
    assert_shape(&mgl("oe g"), &["A", "O", "I", "G"]);
}

#[test]
fn test_step2_oegn_adjacent_feminine() {
    // oe (fem) IMMEDIATELY before g -> check 4 fires -> feminine "G" (iii2f main rule, NOT
    // remote harmony).
    assert_shape(&mgl("oe g n"), &["A", "O", "I", "G", "A"]);
}

#[test]
fn test_step2_g_i_with_marker_masc() {
    // i + g/h with a reachable MASC marker -> masculine_devsger "H" (mirrors iii.py's
    // `III.g_h.onset_and_devsger_and_gender.A.MNG` pattern 5, `i + g/h + MASC`). Per
    // preprocessing.A/B/C, MASC ends up immediately after every h/g preceded by a masc vowel
    // (init/medi) with no fem vowel in between. What follows g/h does NOT matter — any
    // letters after g/h have their MASC stripped by preprocessing.C, but g/h's own trailing
    // MASC is preserved. All values verified against DraftNew-Regular.otf via hb-shape.
    assert_shape(&mgl("a i g"), &["A", "A", "I", "I", "H"]); // g.fina, prev=i, masc a reaches g
    assert_shape(&mgl("o l i g"), &["A", "O", "L", "I", "H"]); // marker threads through l + i

    // The medial cases below are raw: `H:medi` is `A A` in the public shape.
    assert_shape_raw(&mgl("a i g r"), &["A", "A", "I", "I", "H", "R"]); // g.medi; last position not required
    assert_shape_raw(&mgl("a l i g r"), &["A", "A", "L", "I", "H", "R"]); // marker through l+i, then g.medi
    assert_shape_raw(&mgl("a i g r e"), &["A", "A", "I", "I", "H", "R", "A"]); // fem e *after* g doesn't block
}

#[test]
fn test_step2_g_i_marker_blocked() {
    // i + g/h with the marker BLOCKED or NOT REACHING -> pattern 6 -> feminine.
    assert_shape(&mgl("a e i g"), &["A", "A", "A", "I", "I", "G"]); // fem e blocks masc a from reaching g
    assert_shape(&mgl("i g"), &["A", "I", "G"]); // prev=i but no masc vowel anywhere
}

#[test]
fn test_step2_h_i_with_marker_masc() {
    // Same scenario as g, but with h: h.fina default already equals masc_devsger "H", so
    // h.fina alone can't distinguish "rule fired" from "fell to default" — h.medi is the
    // only observable position (default "G" vs masc_devsger "H").
    // Raw: `H:medi` is `A A` in the public shape, which cannot tell H from a plain vowel pair.
    assert_shape_raw(&mgl("a i h r"), &["A", "A", "I", "I", "H", "R"]); // h.medi, marker reaches -> H
    assert_shape(&mgl("a i h"), &["A", "A", "I", "I", "H"]); // h.fina, marker reaches (same as default)
}

#[test]
fn test_step2_h_i_marker_blocked() {
    // KEY DIFFERENCE from g: iii2f.A pattern 6 (i+g -> feminine) covers ONLY g, not h. When
    // the marker doesn't reach h, h falls through to its DEFAULT, not to feminine.
    assert_shape(&mgl("a e i h r"), &["A", "A", "A", "I", "I", "G", "R"]); // fem e blocks; h.medi default "G"
    assert_shape(&mgl("a e i h"), &["A", "A", "A", "I", "I", "H"]); // h.fina default "H" (no pattern 6 for h)
    assert_shape(&mgl("i h"), &["A", "I", "H"]); // no masc anywhere; h.fina default "H" (not G like `i g`)
}

#[test]
fn test_step2_h_non_i_prev_no_remote() {
    // h with prev != i — same logic as g but observable on h.medi.
    assert_shape(&mgl("o l h"), &["A", "O", "L", "H"]); // h.fina default "H" (looks like masc, no rule fired)
    assert_shape(&mgl("o l h r"), &["A", "O", "L", "G", "R"]); // h.medi default "G" (genuinely no rule fired)
}

#[test]
fn test_step2_g_non_i_prev_no_remote() {
    // prev letter is NOT i (e.g. consonant) — iii2f.A doesn't fire even if a masc vowel is
    // reachable. This is the strict prev=i gate, narrower than the web docs' broader "remote
    // harmony" prose (mongfontbuilder/web/docs/hudum.mdx:189-220).
    assert_shape(&mgl("o l g"), &["A", "O", "L", "G"]); // prev=l -> default "G" (not H, despite remote masc o)
    assert_shape(&mgl("a l g r"), &["A", "A", "L", "G", "R"]); // prev=l, g.medi with r after -> default "G"
}

#[allow(non_snake_case)] // keep the Python test name verbatim for side-by-side reading
#[test]
fn test_step2_gh_remote_doc_rules_NOT_implemented() {
    // The web docs describe two additional remote-harmony rules NOT implemented in iii.py and
    // NOT exhibited by DraftNew-Regular.otf; documenting the divergence here.
    //
    // Doc rule (5): "g/h remotely follows fem vowel without blocking masc -> feminine". Not
    // observably implementable: g.fina/g.medi/h.medi default is already G (the feminine
    // glyph), and h.fina has no feminine variant (falls back to default H). Our default
    // fallthrough yields the same glyph as the doc-prescribed feminine, so this divergence is
    // invisible in output — recorded here for future reference.
    assert_shape(&mgl("e l g"), &["A", "L", "G"]);
    assert_shape(&mgl("e l g r"), &["A", "L", "G", "R"]);
    //
    // Doc rule (7): "g/h remotely precedes masc vowel without blocking fem ->
    // masculine_devsger". OBSERVABLY divergent from the font: `g r a` should be `Hx` per docs
    // (g.init.masculine_onset/devsger), but DraftNew renders G, because iii2f.B fires first
    // (g.init + consonant -> feminine) and nothing looks ahead remotely for a masc vowel past
    // the consonant. We mirror the implementation, not the docs. Verified against
    // DraftNew-Regular.otf via hb-shape: `g r a` -> u182D.G.init (NOT u182D.Hx.init); `h r a`
    // -> u182C.G.init (NOT u182C.H.init).
    assert_shape(&mgl("g r a"), &["G", "R", "A"]);
    assert_shape(&mgl("h r a"), &["G", "R", "A"]);
    // Adjacency control: g + masc a (no consonant between) -> "Hx".
    assert_shape(&mgl("g a"), &["Hx", "A"]);
}

#[test]
fn test_step2_gh_init_before_consonant_feminine() {
    // g.init / h.init + consonant -> feminine (iii2f.B): the simplest "no adjacent vowel,
    // init position" fallback rule.
    assert_shape(&mgl("g r"), &["G", "R"]);
    assert_shape(&mgl("h r"), &["G", "R"]);
}

#[test]
fn test_step2_t_devsger_before_ee() {
    // ateen (a t ee n): t.medi before ee -> iii2g.t.devsger fires -> "T" (NOT iii2e onset —
    // see iii2e block comment for the carve-out). Verified against DraftNew-Regular.otf:
    // u1832.T.medi (devsger T).
    let t = token_with_alias(&mgl("a t ee n"), Alias::T);
    assert_eq!(t.condition, Some(Condition::Devsger));
    assert_eq!(unit_names(&t.written), ["T"]);
    // Full shape regression: D form would be the buggy output.
    assert_shape(&mgl("a t ee n"), &["A", "A", "T", "W", "A"]);
    // Control: t + non-ee vowel still goes onset -> falls to default "D" (iii2e fires; iii2g
    // doesn't).
    assert_shape(&mgl("a t i n"), &["A", "A", "D", "I", "A"]);
    // Control: t + consonant -> iii2g.t.devsger fires (consonant branch) -> "T".
    assert_shape(&mgl("a t r"), &["A", "A", "T", "R"]);
    // Control: d + ee still goes iii2e onset (carve-out is t-only); d.medi.onset = D (same as
    // default — visually identical).
    assert_shape(&mgl("a d ee n"), &["A", "A", "D", "W", "A"]);
}

#[test]
fn test_step2_sh_dotless() {
    // iii2g.sh.dotless: (1) sh.init + i.medi -> dotless; (2) sh.medi + i.{medi,fina} ->
    // dotless. Verified against DraftNew-Regular.otf (u1831.S.init is the dotless variant of
    // the dotted default u1831.Sh.init).
    assert_shape(&mgl("sh i m"), &["S", "I", "M"]); // clause 1: sh.init + i.medi

    // One input exercises both clauses: 1st sh.init+i.medi (clause 1), 2nd sh.medi+i.fina
    // (clause 2).
    assert_shape(&mgl("sh i sh i"), &["S", "I", "S", "I"]);
    assert_shape(&mgl("a sh i"), &["A", "A", "S", "I"]); // clause 2: sh.medi + i.fina

    // Negative controls — neither clause fires, sh stays default "Sh".
    assert_shape(&mgl("sh i"), &["Sh", "I"]); // sh.init + i.FINA (only 2 letters)
    assert_shape(&mgl("sh a"), &["Sh", "A"]); // sh.init + non-i vowel
    assert_shape(&mgl("sh ee"), &["Sh", "W"]); // ee not in the rule's input set
}

#[test]
fn test_step2_g_dotless() {
    // iii2g.g.dotless has two precise sub-rules — both require s/d before g — and OVERRIDE
    // whatever condition iii2f.h_g.harmony or iii2c.chachlag_onset set earlier (the OpenType
    // class-membership trick).
    // Rule 1: s/d + g.medi + masc vowel -> dotless "H" (overrides masculine_onset "Hx").
    // Raw: the dotless "H" is medial here, and the public shape folds `H:medi` to `A A`.
    assert_shape_raw(&mgl("s g a"), &["S", "H", "A"]);
    assert_shape_raw(&mgl("d g a"), &["T", "H", "A"]);
    assert_shape_raw(&mgl("a s g a"), &["A", "A", "S", "H", "A"]);
    // Rule 2: s/d + g.fina + MVS + chachlag a.isol -> dotless "H" (overrides chachlag_onset
    // "Hx").
    assert_shape(&mgl("s g mvs a"), &["S", "H", "Mvs", "Aa"]);
    assert_shape(&mgl("d g mvs a"), &["T", "H", "Mvs", "Aa"]);
    // Negative: rule 1 needs a masc vowel — fem/neut/consonant don't fire.
    assert_shape(&mgl("s g i"), &["S", "G", "I"]); // neut i
    assert_shape(&mgl("s g e"), &["S", "G", "Aa"]); // fem e (iii2f -> feminine = G)
    assert_shape(&mgl("s g n"), &["S", "G", "A"]); // consonant n

    // Negative: rule 2 needs MVS + chachlag a — bare g.fina doesn't fire.
    assert_shape(&mgl("s g"), &["S", "G"]);
    // Negative: prev letter must be s or d.
    assert_shape_raw(&mgl("a g a"), &["A", "A", "Hx", "A"]); // iii2f masc_onset (`Hx:medi`)
    assert_shape(&mgl("n g mvs a"), &["N", "Hx", "Mvs", "Aa"]); // iii2c chachlag_onset
}

// ── Step 3 · Particle — MVS particle dictionary lookup ──
//
// Per `_PARTICLE_TARGET_ALIASES` in rules.py, only 7 letters can receive the `particle`
// condition (in this order): a, e, i, u, ue, d, y. Tests are organized by which TARGET letter
// they exercise; a test that triggers particle on multiple targets (e.g. d+u or i+y) is listed
// under the target that comes first in that order. (e has no dict entry where it sits at a
// particle index, so it has no dedicated test — the iyen/iyer/degen tests double as negative
// coverage for e.)

#[test]
fn test_step3_particle_acha() {
    // tal + MVS + acha -- a.init at idx 1 -> particle (e, d, y not targeted here)
    assert_shape(
        &mgl("t a l mvs a ch a"),
        &["T", "A", "L", "Mvs", "A", "Ch", "A"],
    );
}

#[test]
fn test_step3_particle_i() {
    // tal + MVS + i -- i.isol at idx 1 -> particle
    assert_shape(&mgl("t a l mvs i"), &["T", "A", "L", "Mvs", "I"]);
}

#[test]
fn test_step3_particle_iyar() {
    // tal + MVS + iyar -- i + y at idx 1,2 -> both particle (masc vowel)
    assert_shape(
        &mgl("t a l mvs i y a r"),
        &["T", "A", "L", "Mvs", "I", "I", "A", "R"],
    );
}

#[test]
fn test_step3_particle_iyer() {
    // fem-vowel pair of iyar; identical shape because e.medi default == a.medi default == "A"
    assert_shape(
        &mgl("t a l mvs i y e r"),
        &["T", "A", "L", "Mvs", "I", "I", "A", "R"],
    );
}

#[test]
fn test_step3_particle_iyen() {
    // i,y particles; e at idx 3 stays default (not in [1,2])
    assert_shape(
        &mgl("t a l mvs i y e n"),
        &["T", "A", "L", "Mvs", "I", "I", "A", "A"],
    );
}

#[test]
fn test_step3_particle_u() {
    // tal + MVS + u -- u.isol at idx 1 -> particle
    assert_shape(&mgl("t a l mvs u"), &["T", "A", "L", "Mvs", "U"]);
}

#[test]
fn test_step3_particle_du() {
    // tal + MVS + du -- d at idx 1 AND u at idx 2 both particles
    assert_shape(&mgl("t a l mvs d u"), &["T", "A", "L", "Mvs", "D", "U"]);
}

#[test]
fn test_step3_particle_ue() {
    // tal + MVS + ue -- ue.isol at idx 1 -> particle "U"
    assert_shape(&mgl("t a l mvs ue"), &["T", "A", "L", "Mvs", "U"]);
}

#[test]
fn test_step3_particle_uen() {
    // tal + MVS + ue+n -- ue.init particle "O", n.fina devsger "A"
    assert_shape(&mgl("t a l mvs ue n"), &["T", "A", "L", "Mvs", "O", "A"]);
}

#[test]
fn test_step3_particle_ued() {
    // tal + MVS + ue+d -- ue.init particle "O", d.fina devsger "Dd" (public shape: `... O O A`)
    assert_shape_raw(&mgl("t a l mvs ue d"), &["T", "A", "L", "Mvs", "O", "Dd"]);
}

#[test]
fn test_step3_particle_duer() {
    // tal + MVS + duer -- d at idx 1 AND ue at idx 2 both particles
    assert_shape(
        &mgl("t a l mvs d ue r"),
        &["T", "A", "L", "Mvs", "D", "O", "R"],
    );
}

#[test]
fn test_step3_particle_dagan() {
    // tal + MVS + dagan -- d at idx 1 -> particle (masc vowel harmony). Raw: `Hx:medi` is
    // `N N` in the public shape.
    assert_shape_raw(
        &mgl("t a l mvs d a g a n"),
        &["T", "A", "L", "Mvs", "D", "A", "Hx", "A", "A"],
    );
}

#[test]
fn test_step3_particle_degen() {
    // fem-vowel pair of dagan. g.medi here gets "G" (iii2f feminine) instead of "Hx"
    // (masc_onset in dagan) because of the surrounding fem vowel e.
    assert_shape(
        &mgl("t a l mvs d e g e n"),
        &["T", "A", "L", "Mvs", "D", "A", "G", "A", "A"],
    );
}

#[test]
fn test_step3_particle_yi() {
    // tal + MVS + yi -- y.init at idx 1 -> particle "I"
    assert_shape(&mgl("t a l mvs y i"), &["T", "A", "L", "Mvs", "I", "I"]);
}

#[test]
fn test_step3_particle_yin() {
    // tal + MVS + yin -- y.init at idx 1 -> particle "I"
    assert_shape(
        &mgl("t a l mvs y i n"),
        &["T", "A", "L", "Mvs", "I", "I", "A"],
    );
}

#[test]
fn test_step3_particle_uu() {
    // "u u" in dict -- u.init at idx 0 -> particle "O" (word-internal, no MVS)
    assert_shape(&mgl("u u"), &["O", "U"]);
}

#[test]
fn test_step3_particle_ueue() {
    // "ue ue" in dict -- ue.init at idx 0 -> particle "O" (word-internal, no MVS)
    assert_shape(&mgl("ue ue"), &["O", "U"]);
}

#[test]
fn test_step3_no_particle_match() {
    // "mvs l e" -- not a dict entry -> l and e stay default
    assert_shape(&mgl("t a l mvs l e"), &["T", "A", "L", "Mvs", "L", "A"]);
    // "mvs r" -- not a dict entry -> r stays default
    assert_shape(&mgl("t a l mvs r"), &["T", "A", "L", "Mvs", "R"]);
}

// ── Step 4 · Devsger — i.medi after vowel -> vowel_devsger (double tooth) ──
//
// Rule (iii.py iii4): i.medi gets `vowel_devsger` IF the immediately preceding vowel's WRITTEN
// form (post-FVS substitution) does NOT end with the unit "I". Verified against
// DraftNew-Regular.otf via hb-shape.

#[test]
fn test_step4_devsger_ail() {
    // a.init = "AA" (ends with A, not I) -> fires -> i.medi.vowel_devsger = "II"
    assert_shape(&mgl("a i l"), &["A", "A", "I", "I", "L"]);
}

#[test]
fn test_step4_no_devsger_tueil() {
    // ue.medi default = "OI" (ends with I) -> no devsger -> i.medi default "I"
    assert_shape(&mgl("t ue i l"), &["T", "O", "I", "I", "L"]);
}

#[test]
fn test_step4_no_devsger_aueil_fvs1() {
    // FVS-resolved form still ends with I: ue.medi.fvs1 = "OI" -> no devsger (same outcome as
    // the plain-default case, arrived at via the FVS path).
    assert_shape(&mgl("a ue fvs1 i l"), &["A", "A", "O", "I", "I", "L"]);
}

#[test]
fn test_step4_no_devsger_naima_fvs3() {
    // i has explicit FVS3 -> devsger rule skipped -> i.medi.fvs3 = "I" (default). Without the
    // FVS3 this would be naima (n a i m a) -> i.medi devsger = "II".
    assert_shape(&mgl("n a i fvs3 m a"), &["N", "A", "I", "M", "A"]);
}

// ── Step 5 · Post-bowed — vowels after bowed consonants ──
//
// Rule (iii.py iii5): vowel.FINA gets `post_bowed` if the preceding consonant renders as a
// "bowed" written unit: bowedB = B, P, F; bowedK = K, K2; bowedG = G, Gx (g/h after feminine
// harmony only — their default Hx/H are NOT bowed). bowedB/bowedK accept all fina vowels (a, e,
// o, u, oe, ue); bowedG accepts only e. Verified against DraftNew-Regular.otf via hb-shape.

#[test]
fn test_step5_post_bowed_after_b() {
    // bowedB (b/p/f) + vowel.fina -> post_bowed
    assert_shape(&mgl("b a"), &["B", "Aa"]);
    assert_shape(&mgl("b e"), &["B", "Aa"]);
    assert_shape(&mgl("b o"), &["B", "O"]); // also iii2a.marked applies; same glyph
    assert_shape(&mgl("p a"), &["P", "Aa"]);
    // i.fina after F: i is NOT in the post_bowed input set -> "I"
    assert_shape(&mgl("f i"), &["F", "I"]);
}

#[test]
fn test_step5_post_bowed_after_k() {
    // bowedK (k/k2) + vowel.fina -> post_bowed
    assert_shape(&mgl("k a"), &["K", "Aa"]);
    assert_shape(&mgl("k2 e"), &["K2", "Aa"]);
}

#[test]
fn test_step5_post_bowed_after_g() {
    // bowedG (g/h with feminine form G/Gx) + e.fina -> post_bowed. G/Gx only appears when g/h
    // gets `feminine` from iii2f — with masc vowel context g becomes Hx instead, which is not
    // bowed.
    assert_shape(&mgl("g e"), &["G", "Aa"]);
    assert_shape(&mgl("oe g e"), &["A", "O", "I", "G", "Aa"]);
    assert_shape(&mgl("b o g e"), &["B", "O", "G", "Aa"]);
}

#[test]
fn test_step5_no_post_bowed_g_init_plus_a() {
    // g.init + masc a: g becomes Hx (masc_onset), NOT G -- so a does NOT get post_bowed.
    assert_shape(&mgl("g a"), &["Hx", "A"]);
}

#[test]
fn test_step5_no_post_bowed_medi() {
    // vowel at MEDI position after bowed -> no post_bowed (the data has no post_bowed variant
    // for medi vowels, so even a rule-assigned condition falls back to the default glyph).
    assert_shape(&mgl("b a l"), &["B", "A", "L"]);
    assert_shape(&mgl("b e r"), &["B", "A", "R"]);
    assert_shape(&mgl("g e r"), &["G", "A", "R"]);
}

// ── Position assignment ──

#[test]
fn test_position_single_isol() {
    assert_eq!(letter_positions(&mgl("n")), [Position::Isol]);
}

#[test]
fn test_position_init_fina() {
    assert_eq!(
        letter_positions(&mgl("a b")),
        [Position::Init, Position::Fina]
    );
}

#[test]
fn test_position_init_medi_fina() {
    assert_eq!(
        letter_positions(&mgl("t a l")),
        [Position::Init, Position::Medi, Position::Fina]
    );
}

#[test]
fn test_position_mvs_breaks_chain() {
    // t a l MVS a -> [init, medi, fina] + [isol]
    assert_eq!(
        letter_positions(&mgl("t a l mvs a")),
        [
            Position::Init,
            Position::Medi,
            Position::Fina,
            Position::Isol
        ]
    );
}

#[test]
fn test_position_double_mvs() {
    // t a l MVS a MVS y i n -> 3 segments
    assert_eq!(
        letter_positions(&mgl("t a l mvs a mvs y i n")),
        [
            Position::Init,
            Position::Medi,
            Position::Fina,
            Position::Isol,
            Position::Init,
            Position::Medi,
            Position::Fina,
        ]
    );
}

// ── NNBSP → MVS normalization ──

#[test]
fn test_nnbsp_produces_same_shape() {
    assert_eq!(
        shape(&mgl("t a l nnbsp a nnbsp y i n")),
        shape(&mgl("t a l mvs a mvs y i n"))
    );
}

// ── UTN-vs-EAC divergences ──
//
// GB/T 25914-2023 (EAC) and the UTN57 model disagree on a small number of edge cases.
// mongfontbuilder follows UTN57 and marks the EAC counter-examples xfail
// (mongfontbuilder/tests/test_font.py:42-69); we follow UTN too and exclude the matching rows
// from `eac_hud.rs` (see `UTN_XFAIL_CASES` there). The tests below pin down the UTN-correct
// shaping that the EAC suite cannot express.

#[test]
fn test_utn_g_fvs2_blocks_chachlag_onset() {
    // A. FVS attached to the pre-MVS letter blocks chachlag_onset (iii.py iii2c,
    // mongfontbuilder): user FVS on h/g/n/j/w immediately before `MVS + a/e.isol` suppresses
    // the chachlag_onset substitution, so the letter keeps its user-chosen FVS form.
    // `b a g fvs2 mvs a` -- UTN: g.fvs2 = G (feminine, user wins). EAC counter-example
    // (XIM11-1012) expects Hx.
    assert_shape(&mgl("b a g fvs2 mvs a"), &["B", "A", "G", "Mvs", "Aa"]);
}

#[test]
fn test_utn_g_fvs3_picks_chachlag_onset() {
    // FVS3 IS the chachlag_onset slot for g.fina (variants: fvs3 -> Hx, conditions =
    // [chachlag_onset]). User and rule agree -> Hx fires the UTN-correct way.
    assert_shape(&mgl("b a g fvs3 mvs a"), &["B", "A", "Hx", "Mvs", "Aa"]);
}

#[test]
fn test_utn_nnbsp_alone_renders_as_mvs() {
    // B. NNBSP is equivalent to MVS (UTN: "old NNBSP function"). iii.py keeps NNBSP in the
    // `mvs` glyph class so chachlag/particle/mvs.narrow/mvs.wide all fire as if it were MVS.
    // EAC wants NNBSP to disable every shaping feature; UTN explicitly rejects that ("the old
    // functionality of NNBSP should be retained").
    // Standalone NNBSP -- UTN renders the MVS slot; EAC wants empty.
    assert_shape(&mgl("nnbsp"), &["Mvs"]);
}

#[test]
fn test_utn_nnbsp_triggers_chachlag() {
    // UTN treats NNBSP as MVS, so the trailing a.isol triggers chachlag (Aa) and g.fina takes
    // chachlag_onset (Hx). EAC XIM11-39 wants `B A H A A` (no chachlag).
    assert_shape(&mgl("b a g nnbsp a"), &["B", "A", "Hx", "Mvs", "Aa"]);
}

#[test]
fn test_utn_nnbsp_triggers_particle() {
    // UTN: particle dict matches `mvs y i n` (NNBSP ≡ MVS) -> y.init = I (particle.fvs1 form).
    // EAC XIM11-40 wants `A A B O Y I A` (no particle).
    assert_shape(
        &mgl("a b u nnbsp y i n"),
        &["A", "A", "B", "O", "Mvs", "I", "I", "A"],
    );
}

#[test]
fn test_utn_nnbsp_renders_mvs_token() {
    // UTN emits the `mvs` separator between the two words; EAC XIM11-41 wants no separator.
    assert_shape(
        &mgl("a b u nnbsp e j i"),
        &["A", "A", "B", "O", "Mvs", "A", "J", "I"],
    );
}

// ── Edge cases ──

#[test]
fn test_single_vowel() {
    assert_shape(&mgl("a"), &["A", "A"]);
}

#[test]
fn test_empty_string() {
    assert!(shaper().shape("").unwrap().is_empty());
}

#[test]
fn test_oron() {
    // oron (o r o n)
    assert_shape(&mgl("o r o n"), &["A", "O", "R", "O", "A"]);
}

#[test]
fn test_mori() {
    // mori (m o r i)
    assert_shape(&mgl("m o r i"), &["M", "O", "R", "I"]);
}

// ── Duplicate encodings folded out of the public shape ──

/// The other side of every `assert_shape_raw` above: what the public `shape` says about the four
/// units that render as the same ink as a sequence of others.
#[test]
fn duplicates_are_folded_out_of_the_public_shape() {
    // `Dd` in both the positions it has, medial `H` and medial `Hx`.
    assert_shape(&mgl("o d"), &["A", "O", "O", "A"]); // Dd:fina   ᠣᠳ
    assert_shape(&mgl("o d b o"), &["A", "O", "O", "A", "B", "O"]); // Dd:medi
    assert_shape(&mgl("b a g sh i"), &["B", "A", "A", "A", "S", "I"]); // H:medi    ᠪᠠᠭᠰᠢ
    assert_shape(&mgl("a r g a l"), &["A", "A", "R", "N", "N", "A", "L"]); // Hx:medi   ᠠᠷᠭᠠᠯ

    // Initial and final `H` / `Hx` are distinct ink and stay.
    assert_shape(&mgl("h o d a"), &["H", "O", "D", "A"]); // H:init
    assert_shape(&mgl("a i g"), &["A", "A", "I", "I", "H"]); // H:fina
    assert_shape(&mgl("a g a"), &["A", "A", "N", "N", "A"]); // Hx:medi collapses …
    assert_shape(&mgl("n g mvs a"), &["N", "Hx", "Mvs", "Aa"]); // … but Hx:fina does not

    // The bug this fixes: ᠠᠷᠠᠳ and ᠠᠷᠠᠤᠠ are one visible word and now shape identically.
    assert_shape(&mgl("a r a d"), &["A", "A", "R", "A", "O", "A"]);
    assert_eq!(shape(&mgl("a r a d")), shape(&mgl("a r a u a")));
    assert!(shaper()
        .same_shape(&mgl("a r a d"), &mgl("a r a u a"))
        .unwrap());
    assert_eq!(
        shaper().normalize(&mgl("a r a d")).unwrap(),
        shaper().normalize(&mgl("a r a u a")).unwrap()
    );

    // `Dd` is a duplicate in every position it has, so it can never reach a public shape at all
    // (the corpus-wide form of this is `no_public_shape_contains_a_duplicate_encoding` in
    // `tests/canonical_golden.rs`).
    for aliases in ["o d", "o d b o", "t a l mvs ue d", "a r a d", "d a d h u"] {
        let units = shape(&mgl(aliases));
        assert!(
            !units.iter().any(|unit| unit == "Dd"),
            "{aliases}: Dd leaked into the public shape {units:?}"
        );
        assert!(shape_raw(&mgl(aliases)).iter().any(|unit| unit == "Dd"));
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// TestSameShape — same_shape() correctly identifies visually identical encodings.
// ════════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_sain_variants_equal() {
    let variants = [
        mgl("s a i n"),
        mgl("s e i n"),
        mgl("s n fvs2 i i n"),
        mgl("s a y fvs1 i n"),
        mgl("s a y fvs1 y fvs1 n"),
    ];
    for variant in &variants[1..] {
        assert!(
            shaper().same_shape(&variants[0], variant).unwrap(),
            "expected same shape: {:?} vs {variant:?}",
            variants[0]
        );
    }
}

#[test]
fn test_different_words_not_equal() {
    // sain (s a i n) vs naima (n a i fvs3 m a) — visually distinct words.
    let sain = mgl("s a i n");
    let naima = mgl("n a i fvs3 m a");
    assert!(!shaper().same_shape(&sain, &naima).unwrap());
}

#[test]
fn test_same_string_reflexive() {
    let sain = mgl("s a i n");
    assert!(shaper().same_shape(&sain, &sain).unwrap());
}

// ════════════════════════════════════════════════════════════════════════════════════════════
// TestNNBSP — tokenization/shape/chachlag subset. (The normalize-related tests, including the
// rest of TestNNBSP, are ported in tests/normalize.rs.)
// ════════════════════════════════════════════════════════════════════════════════════════════

#[test]
fn test_nnbsp_survives_tokenization() {
    // NNBSP between two Mongolian letters must not be dropped; it is normalized to MVS
    // (U+180E) during tokenization.
    let text = mgl("s a i n nnbsp a");
    let cps: Vec<u32> = details(&text).iter().map(|d| d.cp as u32).collect();
    assert!(
        cps.contains(&0x180E),
        "NNBSP must be normalized to MVS during tokenization"
    );
    assert!(
        !cps.contains(&0x202F),
        "NNBSP codepoint must not survive tokenization"
    );
}

#[test]
fn test_nnbsp_token_is_mvs_flag() {
    // NNBSP input must produce exactly one MVS token (cp == U+180E).
    let text = mgl("s nnbsp a");
    assert_eq!(
        details(&text).iter().filter(|d| d.cp == '\u{180E}').count(),
        1
    );
    // Python also asserts `is_mvs` on that token; the flag is not public here, but the behaviour
    // it drives is: the token shapes as the `Mvs` structural unit.
    assert_shape(&text, &["S", "Mvs", "Aa"]);
}

#[test]
fn test_nnbsp_same_shape_as_mvs() {
    // A word with NNBSP should shape identically to the same word with MVS.
    assert_eq!(shape(&mgl("s a i n mvs a")), shape(&mgl("s a i n nnbsp a")));
}

#[test]
fn test_nnbsp_chachlag_trigger() {
    // The suffix 'a' after NNBSP should get the chachlag condition, same as after MVS.
    let text = mgl("s a i n nnbsp a");
    let triggered = details(&text)
        .iter()
        .any(|d| d.alias == Some(Alias::A) && d.condition == Some(Condition::Chachlag));
    assert!(triggered, "NNBSP should trigger chachlag on following a/e");
}
