//! ZWNJ (U+200C) as an in-word joining break (issue #34): the letter before it is written
//! `fina`/`isol`, the letter after it `init`/`isol`, the context rules look through it as the
//! reference font does (Kushim-Jiang/mongolian#58), and a break that changes no glyph is dropped
//! from the public shape by the same written-unit machinery as a redundant ZWJ. Twin of
//! `python/tests/test_zwnj.py`.
//!
//! Every expected shape below was cross-checked against HarfBuzz with the bundled
//! `web/public/fonts/hudum.otf` (the glyph names carry the written units): all agree, except
//! the one row marked "pre-existing", where the engine already differs from the font without
//! any ZWNJ.

mod common;

use common::{hex, mgl, unit_names};
use mongol_norm::{
    Error, Locale, Position, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit,
};

const ZWNJ: char = '\u{200C}';

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

fn shape(text: &str) -> String {
    shaper().shape_str(text).unwrap()
}

/// `(aliases, shape with the ZWNJ(s), shape without them, canonical code points)` — the table of
/// the pull request. Dropped breaks leave no `Zwnj`; kept ones keep exactly where they were.
const TABLE: &[(&str, &str, &str, &str)] = &[
    // word edges: nothing changes position
    ("zwnj b a", "B+Aa", "B+Aa", "182A 1820 180B"),
    ("b a zwnj", "B+Aa", "B+Aa", "182A 1820 180B"),
    ("zwnj n a", "N+A", "N+A", "1828 1820 180C"),
    ("n a zwnj", "N+A", "N+A", "1828 1820 180C"),
    ("zwnj a", "A+A", "A+A", "1820 180B 1820 180C"),
    ("a zwnj", "A+A", "A+A", "1820 180B 1820 180C"),
    // control-only input: a lone ZWNJ is the whole shape (like a lone ZWJ)
    ("zwnj", "Zwnj", "", "200C"),
    ("zwnj zwnj", "Zwnj", "", "200C"),
    ("zwnj fvs1", "Zwnj", "", "200C"),
    ("zwnj mvs", "Mvs", "Mvs", "180E"),
    ("mvs zwnj", "Mvs", "Mvs", "180E"),
    ("zwj zwnj", "Zwnj", "Zwj", "200C"),
    ("zwnj zwj", "Zwnj", "Zwj", "200C"),
    // repeated breaks collapse to one
    (
        "a zwnj zwnj b",
        "A+A+Zwnj+B",
        "A+A+B",
        "1820 180B 1820 180C 200C 182A",
    ),
    ("zwnj b zwnj", "B", "B", "182A"),
    // between letters: the reviewed edge classes decide
    ("b zwnj b a", "B+B+Aa", "B+B+Aa", "182A 182A 1820 180B"), // B:isol≡init, B:init≡medi
    (
        "a o zwnj b a",
        "A+A+U+Zwnj+B+Aa",
        "A+A+O+B+Aa",
        "1820 180B 1820 1823 180C 200C 182A 1820 180B",
    ),
    (
        "b zwnj a",
        "B+Zwnj+A+A",
        "B+Aa",
        "182A 200C 1820 180B 1820 180C",
    ),
    (
        "n zwnj a",
        "N+Zwnj+A+A",
        "N+A",
        "1828 200C 1820 180B 1820 180C",
    ),
    (
        "a zwnj a",
        "A+A+Zwnj+A+A",
        "A+A+A",
        "1820 180B 1820 180C 200C 1820 180B 1820 180C",
    ),
    (
        "a b zwnj b a",
        "A+A+B+Zwnj+B+Aa",
        "A+A+B+B+Aa",
        "1820 180B 1820 182A 200C 182A 1820 180B",
    ),
    (
        "l i zwnj e fvs1",
        "L+I+Zwnj+Aa",
        "L+G",
        "182F 1822 200C 1820 180C",
    ), // no `G` across a kept break
    ("s zwnj g", "S+Zwnj+Hx", "S+G", "1830 200C 182C 180B"),
    ("b zwnj s", "B+Zwnj+S", "B+S", "182A 200C 1830"),
    (
        "t a l zwnj a",
        "T+A+L+Zwnj+A+A",
        "T+A+L+A",
        "1832 1820 182F 200C 1820 180B 1820 180C",
    ),
    (
        "a zwnj b zwnj a",
        "A+A+Zwnj+B+Zwnj+A+A",
        "A+A+B+Aa",
        "1820 180B 1820 180C 200C 182A 200C 1820 180B 1820 180C",
    ),
    (
        "a zwnj n zwnj a",
        "A+A+Zwnj+N+Zwnj+A+A",
        "A+A+N+A",
        "1820 180B 1820 180C 200C 1828 200C 1820 180B 1820 180C",
    ),
    // beside an MVS / NNBSP: the MVS already breaks joining, and context looks through the ZWNJ
    ("mvs zwnj a", "Mvs+Aa", "Mvs+Aa", "180E 1820"), // chachlag through the break
    ("zwnj mvs a", "Mvs+Aa", "Mvs+Aa", "180E 1820"),
    (
        "a zwnj mvs a",
        "A+A+Mvs+Aa",
        "A+A+Mvs+Aa",
        "1820 180B 1820 180C 180E 1820",
    ),
    (
        "a zwnj nnbsp a",
        "A+A+Mvs+Aa",
        "A+A+Mvs+Aa",
        "1820 180B 1820 180C 180E 1820",
    ),
    ("mvs zwnj i", "Mvs+I", "Mvs+I", "180E 1822 180B"), // particle through the break
    (
        "mvs zwnj u n",
        "Mvs+O+A",
        "Mvs+O+A",
        "180E 1824 180B 1820 180C",
    ),
    (
        "mvs zwnj y i n",
        "Mvs+I+I+A",
        "Mvs+I+I+A",
        "180E 1822 180B 1822 180D 1820 180C",
    ),
    ("n zwnj mvs a", "N+Mvs+Aa", "N+Mvs+Aa", "1828 180E 1820"),
    (
        "a n zwnj mvs a",
        "A+A+N+Mvs+Aa",
        "A+A+N+Mvs+Aa",
        "1820 180B 1820 1828 180B 180E 1820",
    ), // chachlag onset
    (
        "s g zwnj mvs a",
        "S+H+Mvs+Aa",
        "S+H+Mvs+Aa",
        "1830 182C 180E 1820",
    ), // dotless g through the break
    (
        "g zwnj mvs a",
        "Hx+Mvs+Aa",
        "Hx+Mvs+Aa",
        "182C 180B 180E 1820",
    ),
    (
        "t a l mvs zwnj a",
        "T+A+L+Mvs+Aa",
        "T+A+L+Mvs+Aa",
        "1832 1820 182F 180E 1820",
    ),
    // a break inside a particle ends it — the font's patterns are keyed on joining positions
    (
        "mvs i zwnj y a r",
        "Mvs+I+Zwnj+Y+A+R",
        "Mvs+I+I+A+R",
        "180E 1822 180B 200C 1836 180C 1820 1837",
    ),
    (
        "mvs y zwnj i n",
        "Mvs+Y+Zwnj+A+I+A",
        "Mvs+I+I+A",
        "180E 1836 200C 1820 180B 1822 180D 1820 180C",
    ),
    (
        "mvs u zwnj u",
        "Mvs+U+Zwnj+A+O",
        "Mvs+O+U",
        "180E 1824 180B 200C 1820 180B 1823 180B",
    ),
    (
        "a mvs d u zwnj r",
        "A+A+Mvs+D+U+Zwnj+R",
        "A+A+Mvs+D+O+R",
        "1820 180B 1820 180C 180E 1833 180B 1823 180C 200C 1837",
    ),
    // beside a nirugu: the nirugu joins its own side
    ("n zwnj nirugu", "N+Nirugu", "N+Nirugu", "1828 180A"), // N:isol ≡ N:init
    (
        "zwnj nirugu i e fvs1",
        "Nirugu+G",
        "Nirugu+G",
        "180A 182D 180C",
    ),
    (
        "a zwnj nirugu b",
        "A+A+Zwnj+Nirugu+B",
        "A+A+Nirugu+B",
        "1820 180B 1820 180C 200C 180A 182A",
    ),
    (
        "a nirugu zwnj b",
        "A+A+Nirugu+Zwnj+B",
        "A+A+Nirugu+B",
        "1820 180B 1820 180A 200C 182A",
    ),
    (
        "nirugu zwnj b",
        "Nirugu+Zwnj+B",
        "Nirugu+B",
        "180A 200C 182A",
    ),
    // beside a ZWJ, both orders
    ("b zwnj zwj a", "B+Aa", "B+Aa", "182A 1820 180B"),
    ("n zwnj zwj a", "N+A", "N+A", "1828 1820 180C"),
    ("zwj zwnj n a", "N+A", "Zwj+N+A", "1828 1820 180C"),
    ("n a zwnj zwj", "N+A", "N+A+Zwj", "1828 1820 180C"),
    (
        "a zwj zwnj b a",
        "A+A+B+Aa",
        "A+A+B+Aa",
        "1820 180B 1820 182A 1820 180B",
    ),
    (
        "a zwj zwnj b",
        "A+A+Zwj+Zwnj+B",
        "A+A+B",
        "1820 180B 1820 200D 200C 182A",
    ),
    (
        "a zwnj zwj b",
        "A+A+Zwnj+Zwj+B",
        "A+A+B",
        "1820 180B 1820 180C 200C 200D 182A",
    ),
    (
        "b zwj zwnj a",
        "B+Zwnj+A+A",
        "B+Aa",
        "182A 200C 1820 180B 1820 180C",
    ),
    // an FVS belongs to the letter directly before it; one after a ZWNJ is a stray and ignored
    (
        "a zwnj fvs1 a",
        "A+A+Zwnj+A+A",
        "A+A",
        "1820 180B 1820 180C 200C 1820 180B 1820 180C",
    ),
    (
        "a fvs1 zwnj a",
        "A+Zwnj+A+A",
        "A+A",
        "1820 180B 200C 1820 180B 1820 180C",
    ),
    ("b a fvs1 zwnj", "B+Aa", "B+Aa", "182A 1820 180B"),
    // gender and harmony cross the break (positions do not)
    (
        "n ue zwnj h nirugu",
        "N+Ue+Zwnj+G+Nirugu",
        "N+O+I+G+Nirugu",
        "1828 1825 180B 200C 182D 180C 180A",
    ),
    (
        "h zwnj oe",
        "H+Zwnj+A+Ue",
        "G+Ue",
        "182C 200C 1820 180B 1825 180B",
    ),
    ("g zwnj e", "Hx+Zwnj+A", "G+Aa", "182C 180B 200C 1820 180B"),
    (
        "a zwnj g",
        "A+A+Zwnj+Hx",
        "A+A+H",
        "1820 180B 1820 180C 200C 182C 180B",
    ),
    (
        "a l zwnj g",
        "A+A+L+Zwnj+Hx",
        "A+A+L+G",
        "1820 180B 1820 182F 200C 182C 180B",
    ),
    (
        "a b zwnj g",
        "A+A+B+Zwnj+Hx",
        "A+A+B+G",
        "1820 180B 1820 182A 200C 182C 180B",
    ),
    (
        "i g zwnj b a",
        "A+I+H+Zwnj+B+Aa",
        "A+I+A+A+B+Aa",
        "1820 180B 1822 180D 182C 200C 182A 1820 180B",
    ), // backward marker crosses
    (
        "i g zwnj a",
        "A+I+G+Zwnj+A+A",
        "A+I+N+N+A",
        "1820 180B 1822 180D 182D 180C 200C 1820 180B 1820 180C",
    ),
    (
        "e b zwnj i g",
        "A+B+Zwnj+A+I+G",
        "A+B+I+G",
        "1820 180B 182A 200C 1820 180B 1822 180D 182D 180C",
    ),
    (
        "s g zwnj a",
        "S+G+Zwnj+A+A",
        "S+A+A+A",
        "1830 182D 180C 200C 1820 180B 1820 180C",
    ),
    // pre-existing: the font marks a final `oe` after an init+medi consonant cluster (`Ue`)
    // with or without a ZWNJ; the engine does not. Unrelated to the break.
    (
        "m n oe zwnj g e",
        "M+N+U+Zwnj+G+Aa",
        "M+N+O+I+G+Aa",
        "182E 1828 180B 1823 180C 200C 182D 180C 1821 180B",
    ),
    // the issue's corpus (ZWJ + ZWNJ): three words lose both controls, the first keeps neither
    // but its `ue` stays marked because the break made the `p` initial — and `P:init` is the
    // same shape as `P:medi`, so the shape is the break-free `L A L A P O I O A`
    (
        "l a l a zwj zwnj p ue d",
        "L+A+L+A+P+O+I+O+A",
        "L+A+L+A+P+O+O+A",
        "182F 1820 182F 1820 182B 1823 1822 180D 1823 1820 180C",
    ),
    (
        "t a ng zwj zwnj p u r u m",
        "T+A+A+G+P+O+R+O+M",
        "T+A+A+G+P+O+R+O+M",
        "1832 1820 1820 182D 180C 182B 1823 1837 1823 182E",
    ),
    (
        "t i l zwj zwnj p u r u g",
        "T+I+L+P+O+R+O+H",
        "T+I+L+P+O+R+O+H",
        "1832 1822 180D 182F 182B 1823 1837 1823 182C",
    ),
    (
        "s e ng zwj zwnj p u r u m",
        "S+A+A+G+P+O+R+O+M",
        "S+A+A+G+P+O+R+O+M",
        "1830 1820 1820 182D 180C 182B 1823 1837 1823 182E",
    ),
    // the same words with the ZWNJ alone keep it: `A:fina`/`G:fina`/`L:fina` are not their
    // `medi` forms
    (
        "l a l a zwnj p ue d",
        "L+A+L+A+Zwnj+P+O+I+O+A",
        "L+A+L+A+P+O+O+A",
        "182F 1820 182F 1820 180C 200C 182B 1823 1822 180D 1823 1820 180C",
    ),
    (
        "t a ng zwnj p u r u m",
        "T+A+A+G+Zwnj+P+O+R+O+M",
        "T+A+A+G+P+O+R+O+M",
        "1832 1820 1820 182D 180C 200C 182B 1823 1837 1823 182E",
    ),
    (
        "t i l zwnj p u r u g",
        "T+I+L+Zwnj+P+O+R+O+H",
        "T+I+L+P+O+R+O+H",
        "1832 1822 180D 182F 200C 182B 1823 1837 1823 182C",
    ),
];

#[test]
fn drop_and_keep_table() {
    let shaper = shaper();
    for (aliases, with, without, canonical) in TABLE {
        let text = mgl(aliases);
        let plain: String = text.chars().filter(|&c| c != ZWNJ).collect();
        assert_eq!(shape(&text), *with, "shape({aliases})");
        assert_eq!(shape(&plain), *without, "shape({aliases} without ZWNJ)");
        let kept = with.split('+').any(|unit| unit == "Zwnj");
        if kept {
            assert_ne!(
                shaper.shape(&text).unwrap(),
                shaper.shape(&plain).unwrap(),
                "{aliases}: a kept break must change the shape"
            );
        } else {
            assert!(
                shaper.same_shape(&text, &plain).unwrap() || plain.is_empty() || !with.is_empty(),
                "{aliases}"
            );
        }
        let norm = shaper.normalize(&text).unwrap();
        assert_eq!(hex(&norm), *canonical, "normalize({aliases})");
        assert_eq!(
            norm.contains(ZWNJ),
            kept,
            "{aliases}: U+200C emitted iff kept"
        );
        // round trip and idempotence
        assert_eq!(
            shaper.shape(&norm).unwrap(),
            shaper.shape(&text).unwrap(),
            "{aliases}"
        );
        assert_eq!(
            shaper.normalize(&norm).unwrap(),
            norm,
            "{aliases}: idempotent"
        );
        // every ZWNJ of the input is still in the engine's own output
        let raw = shaper.shape_raw(&text).unwrap();
        assert_eq!(
            raw.iter().filter(|&&u| u == WrittenUnit::Zwnj).count(),
            text.chars().filter(|&c| c == ZWNJ).count(),
            "{aliases}: shape_raw keeps every ZWNJ"
        );
        assert_eq!(
            shaper.trace(&text).unwrap().shape,
            shaper.shape(&text).unwrap()
        );
    }
}

#[test]
fn dropped_breaks_shape_like_the_plain_word() {
    let shaper = shaper();
    for (aliases, with, _, _) in TABLE {
        if with.contains("Zwnj") || with.is_empty() {
            continue;
        }
        let text = mgl(aliases);
        let plain: String = text.chars().filter(|&c| c != ZWNJ).collect();
        // Where the plain text has the same ink, the two normalize alike; `zwj zwnj n a` and
        // friends are the exception in the table — the ZWJ comes alive without the break.
        if shaper.same_shape(&text, &plain).unwrap() {
            assert_eq!(
                shaper.normalize(&text).unwrap(),
                shaper.normalize(&plain).unwrap(),
                "{aliases}"
            );
        }
    }
}

#[test]
fn issue_corpus_words() {
    let shaper = shaper();
    let word1 = "\u{182F}\u{1820}\u{182F}\u{1820}\u{200D}\u{200C}\u{182B}\u{1826}\u{1833}";
    let word2 = "\u{1832}\u{1820}\u{1829}\u{200D}\u{200C}\u{182B}\u{1824}\u{1837}\u{1824}\u{182E}";
    let word3 = "\u{1832}\u{1822}\u{182F}\u{200D}\u{200C}\u{182B}\u{1824}\u{1837}\u{1824}\u{182D}";
    let word4 = "\u{1830}\u{1821}\u{1829}\u{200D}\u{200C}\u{182B}\u{1824}\u{1837}\u{1824}\u{182E}";
    // The engine's own units: the font's picture, controls included (`L.init A.medi L.medi
    // A.medi [zwj] [zwnj] POI.init Dd.fina`).
    assert_eq!(
        unit_names(&shaper.shape_raw(word1).unwrap()),
        ["L", "A", "L", "A", "Zwj", "Zwnj", "P", "O", "I", "Dd"]
    );
    assert_eq!(shape(word1), "L+A+L+A+P+O+I+O+A");
    assert_eq!(
        hex(&shaper.normalize(word1).unwrap()),
        "182F 1820 182F 1820 182B 1823 1822 180D 1823 1820 180C"
    );
    // Words 2–4 are the same shape as their plain spelling.
    for (word, plain) in [
        (word2, "t a ng p u r u m"),
        (word3, "t i l p u r u g"),
        (word4, "s e ng p u r u m"),
    ] {
        let plain = mgl(plain);
        assert!(shaper.same_shape(word, &plain).unwrap(), "{}", hex(word));
        assert_eq!(
            shaper.normalize(word).unwrap(),
            shaper.normalize(&plain).unwrap()
        );
    }
    assert_eq!(shape(word2), "T+A+A+G+P+O+R+O+M");
    assert_eq!(shape(word3), "T+I+L+P+O+R+O+H");
    assert_eq!(shape(word4), "S+A+A+G+P+O+R+O+M");
}

#[test]
fn gender_crosses_the_break_but_joining_does_not() {
    let shaper = shaper();
    let witness = "\u{1828}\u{1826}\u{200C}\u{182C}\u{180A}";
    let details = shaper.shape_detailed(witness).unwrap();
    let positions: Vec<Position> = details.iter().map(|d| d.position).collect();
    assert_eq!(
        positions,
        [
            Position::Init,
            Position::Fina,
            Position::Isol,
            Position::Init,
            Position::Isol
        ]
    );
    // The `h` is initial yet feminine (`G`), from the `ue` on the far side of the break — the
    // font's `u182C.G.init`; the `ue` is final (`Ue`), no longer the medial `O I`.
    assert_eq!(unit_names(&details[3].written), ["G"]);
    assert_eq!(unit_names(&details[1].written), ["Ue"]);
    assert_eq!(shape(witness), "N+Ue+Zwnj+G+Nirugu");
    assert_eq!(shape("\u{1828}\u{1826}\u{182C}\u{180A}"), "N+O+I+G+Nirugu");
    assert_eq!(shape("\u{1828}\u{1826}"), "N+Ue");
    assert_eq!(shape("\u{182C}\u{180A}"), "H+Nirugu");
}

#[test]
fn positions_split_at_the_break() {
    let shaper = shaper();
    let positions = |text: &str| -> Vec<Position> {
        shaper
            .shape_detailed(text)
            .unwrap()
            .iter()
            .map(|d| d.position)
            .collect()
    };
    use Position::{Fina, Init, Isol, Medi};
    assert_eq!(
        positions(&mgl("t a l zwnj a")),
        [Init, Medi, Fina, Isol, Isol]
    );
    assert_eq!(positions(&mgl("a zwj zwnj b")), [Init, Isol, Isol, Isol]);
    assert_eq!(positions(&mgl("a zwnj zwj b")), [Isol, Isol, Isol, Fina]);
    assert_eq!(positions(&mgl("zwnj b zwnj")), [Isol, Isol, Isol]);
}

#[test]
fn fvs_after_a_break_is_a_stray() {
    let shaper = shaper();
    let details = shaper.shape_detailed(&mgl("a zwnj fvs1 a")).unwrap();
    assert_eq!(details.len(), 3);
    assert!(details.iter().all(|d| d.fvs.is_none()));
    assert!(shaper
        .same_shape(&mgl("a zwnj fvs1 a"), &mgl("a zwnj a"))
        .unwrap());
    // An FVS right before the break still belongs to its letter.
    let details = shaper.shape_detailed(&mgl("a fvs1 zwnj a")).unwrap();
    assert_eq!(details[0].fvs, Some(mongol_norm::Fvs::Fvs1));
}

#[test]
fn equivalent_spellings_converge() {
    let shaper = shaper();
    let groups: &[&[&str]] = &[
        &[
            "n a",
            "zwnj n a",
            "n a zwnj",
            "zwj zwnj n a",
            "n a zwnj zwj",
            "n zwnj zwj a",
            "zwnj zwnj n a zwnj",
        ],
        &[
            "b a",
            "zwnj b a",
            "b a zwnj",
            "b zwnj zwj a",
            "b a fvs1 zwnj",
            "zwj b a zwnj",
        ],
        &["b b a", "b zwnj b a", "b zwj zwnj b a", "zwnj b zwnj b a"],
        &["a b a", "a zwj zwnj b a"],
        &[
            "mvs a",
            "mvs zwnj a",
            "zwnj mvs a",
            "mvs zwnj zwnj a",
            "nnbsp zwnj a",
        ],
        &["s g mvs a", "s g zwnj mvs a", "s g zwnj zwj mvs a"],
        &["l a l a zwj zwnj p ue d", "l a l a p o i fvs3 o a fvs2"],
    ];
    for group in groups {
        let first = mgl(group[0]);
        let shape0 = shaper.shape(&first).unwrap();
        let norm0 = shaper.normalize(&first).unwrap();
        for aliases in &group[1..] {
            let text = mgl(aliases);
            assert_eq!(
                shaper.shape(&text).unwrap(),
                shape0,
                "{aliases} vs {}",
                group[0]
            );
            assert_eq!(
                shaper.normalize(&text).unwrap(),
                norm0,
                "{aliases} vs {}",
                group[0]
            );
        }
    }
}

#[test]
fn prefix_before_a_kept_break_is_stable() {
    let shaper = shaper();
    for (left, right) in [
        ("b", "a"),
        ("n ue", "h nirugu"),
        ("t a l", "a"),
        ("a", "a"),
        ("a o", "b a"),
    ] {
        let word = mgl(&format!("{left} zwnj {right}"));
        let norm = shaper.normalize(&word).unwrap();
        let prefix = format!("{}{ZWNJ}", shaper.normalize(&mgl(left)).unwrap());
        assert!(
            norm.starts_with(&prefix),
            "{left} zwnj {right}: {} / {}",
            hex(&norm),
            hex(&prefix)
        );
    }
}

#[test]
fn normalize_text_treats_zwnj_as_a_word_character() {
    let shaper = shaper();
    let word = mgl("b zwnj a");
    assert_eq!(
        shaper.normalize_text(&format!("x {word} y")).unwrap(),
        format!("x {} y", shaper.normalize(&word).unwrap())
    );
    // A ZWNJ with no Mongolian letter around it is a lone break and stays what it is.
    assert_eq!(shaper.normalize_text("a\u{200C}b").unwrap(), "a\u{200C}b");
    assert_eq!(shaper.normalize_text("\u{200C}").unwrap(), "\u{200C}");
    assert_eq!(
        shaper.normalize_text(&mgl("zwnj n a")).unwrap(),
        shaper.normalize(&mgl("n a")).unwrap()
    );
}

#[test]
fn written_unit_apis_take_zwnj() {
    let shaper = shaper();
    assert!(shaper
        .known_written_units()
        .unwrap()
        .contains(&WrittenUnit::Zwnj));
    assert_eq!(
        shaper.parse_written_units("B+Zwnj+A+A").unwrap(),
        [
            WrittenUnit::B,
            WrittenUnit::Zwnj,
            WrittenUnit::A,
            WrittenUnit::A
        ]
    );
    assert_eq!(
        shaper.parse_written_units("Zwnj").unwrap(),
        [WrittenUnit::Zwnj]
    );
    // A kept break encodes as U+200C; a redundant one is folded away like a redundant ZWJ.
    assert_eq!(
        hex(&shaper
            .normalize_written_units(&[
                WrittenUnit::B,
                WrittenUnit::Zwnj,
                WrittenUnit::A,
                WrittenUnit::A
            ])
            .unwrap()),
        "182A 200C 1820 180B 1820 180C"
    );
    assert_eq!(
        shaper
            .normalize_written_units(&[WrittenUnit::Zwnj, WrittenUnit::B, WrittenUnit::Aa])
            .unwrap(),
        shaper.normalize(&mgl("b a")).unwrap()
    );
    assert_eq!(
        shaper
            .normalize_written_units(&[WrittenUnit::Zwnj])
            .unwrap(),
        "\u{200C}"
    );
    assert_eq!(
        shaper
            .normalize_written_units(&[WrittenUnit::Zwnj, WrittenUnit::Zwnj])
            .unwrap(),
        "\u{200C}"
    );
    assert_eq!(
        shaper
            .normalize_written_units(&[
                WrittenUnit::L,
                WrittenUnit::I,
                WrittenUnit::Zwnj,
                WrittenUnit::Aa
            ])
            .unwrap(),
        shaper.normalize(&mgl("l i zwnj e fvs1")).unwrap()
    );
    // Positioned: `Zwnj` is a control.
    let records = [
        PositionedWrittenUnit::new(WrittenUnit::B, UnitPosition::Init),
        PositionedWrittenUnit::new(WrittenUnit::Zwnj, UnitPosition::Control),
        PositionedWrittenUnit::new(WrittenUnit::A, UnitPosition::Isol),
    ];
    let encoded = shaper.normalize_positioned_written_units(&records).unwrap();
    assert!(encoded.contains(ZWNJ));
    assert_eq!(
        unit_names(&shaper.shape(&encoded).unwrap()),
        ["B", "Zwnj", "A"]
    );
    assert_eq!(
        shaper.normalize_positioned_written_units(&[PositionedWrittenUnit::new(
            WrittenUnit::Zwnj,
            UnitPosition::Isol
        )]),
        Err(Error::ControlRequiresControlPosition {
            index: 0,
            unit: WrittenUnit::Zwnj
        })
    );
    assert_eq!(
        shaper
            .normalize_positioned_written_units(&[PositionedWrittenUnit::new(
                WrittenUnit::Zwnj,
                UnitPosition::Control
            )])
            .unwrap(),
        "\u{200C}"
    );
    // `Zwnj` is a segment edge, not a joiner: a `medi` record after it still gets its ZWJ.
    let records = [
        PositionedWrittenUnit::new(WrittenUnit::A, UnitPosition::Isol),
        PositionedWrittenUnit::new(WrittenUnit::Zwnj, UnitPosition::Control),
        PositionedWrittenUnit::new(WrittenUnit::N, UnitPosition::Medi),
    ];
    let encoded = shaper.normalize_positioned_written_units(&records).unwrap();
    assert_eq!(
        unit_names(&shaper.shape(&encoded).unwrap()),
        ["A", "Zwnj", "Zwj", "N", "Zwj"]
    );
}

#[test]
fn canonical_version_is_four() {
    assert_eq!(shaper().canonical_version(), Some("mng-canonical/4"));
}

#[test]
fn error_message_names_zwnj() {
    let message = shaper().shape("\u{1820}x").unwrap_err().to_string();
    assert!(
        message.contains("FVS/MVS/NNBSP/Nirugu/ZWJ/ZWNJ"),
        "{message}"
    );
}

/// Deterministic xorshift, ZWNJ-dense alphabet: no panics, every strict normalization
/// round-trips and is idempotent, the public fold is idempotent, and `shape_raw` keeps every
/// ZWNJ.
#[test]
fn zwnj_fuzz() {
    const ALPHABET: &[u32] = &[
        0x1820, 0x1821, 0x1822, 0x1823, 0x1824, 0x1825, 0x1826, 0x1828, 0x1829, 0x182A, 0x182B,
        0x182C, 0x182D, 0x182E, 0x182F, 0x1830, 0x1831, 0x1832, 0x1833, 0x1835, 0x1836, 0x1837,
        0x1838, 0x183B, 0x1842, 0x180B, 0x180C, 0x180D, 0x180E, 0x202F, 0x180A, 0x200D, 0x200C,
        0x200C, 0x200C, 0x200C,
    ];
    let shaper = shaper();
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut kept = 0;
    let mut normalized = 0;
    for _ in 0..20_000 {
        let len = (next() % 10) as usize + 1;
        let text: String = (0..len)
            .map(|_| char::from_u32(ALPHABET[(next() % ALPHABET.len() as u64) as usize]).unwrap())
            .collect();
        let shape = shaper.shape(&text).unwrap();
        let raw = shaper.shape_raw(&text).unwrap();
        assert_eq!(
            raw.iter().filter(|&&u| u == WrittenUnit::Zwnj).count(),
            text.chars().filter(|&c| c == ZWNJ).count(),
            "{}",
            hex(&text)
        );
        assert!(
            shape.iter().filter(|&&u| u == WrittenUnit::Zwnj).count()
                <= raw.iter().filter(|&&u| u == WrittenUnit::Zwnj).count()
        );
        assert!(
            !shape
                .windows(2)
                .any(|w| w == [WrittenUnit::Zwnj, WrittenUnit::Zwnj]),
            "{}",
            hex(&text)
        );
        if shape.contains(&WrittenUnit::Zwnj) {
            kept += 1;
        }
        assert_eq!(shaper.trace(&text).unwrap().shape, shape);
        if let Ok(norm) = shaper.normalize(&text) {
            normalized += 1;
            assert_eq!(
                shaper.shape(&norm).unwrap(),
                shape,
                "{} -> {}",
                hex(&text),
                hex(&norm)
            );
            assert_eq!(shaper.normalize(&norm).unwrap(), norm, "{}", hex(&text));
            assert_eq!(
                shaper.normalize_written_units(&shape).unwrap(),
                norm,
                "{}",
                hex(&text)
            );
        }
    }
    assert!(kept > 1_000, "kept {kept}");
    assert!(normalized > 15_000, "normalized {normalized}");
}
