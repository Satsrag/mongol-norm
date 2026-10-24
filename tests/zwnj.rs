//! ZWNJ (U+200C) as an in-word joining break (issue #34): the letter before it is written
//! `fina`/`isol`, the letter after it `init`/`isol`, the context rules look through it as the
//! reference font does (Kushim-Jiang/mongolian#58), and a break that changes no glyph is dropped
//! from the public shape by the same written-unit machinery as a redundant ZWJ. Twin of
//! `python/tests/test_zwnj.py`.
//!
//! Every expected shape below was cross-checked against HarfBuzz with the bundled
//! `web/public/fonts/hudum.otf` (the glyph names carry the written units): all agree.

mod common;

use common::{hex, mgl, unit_names};
use mongol_norm::{Locale, Position, Shaper, WrittenUnit};

const ZWNJ: char = '\u{200C}';

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

fn shape(text: &str) -> String {
    shaper().shape_str(text).unwrap()
}

/// `(aliases, shape with the ZWNJ(s), shape without them)` — the table of the pull request. Dropped
/// breaks leave no `Zwnj`; kept ones keep exactly where they were.
const TABLE: &[(&str, &str, &str)] = &[
    // word edges: nothing changes position
    ("zwnj b a", "B+Aa", "B+Aa"),
    ("b a zwnj", "B+Aa", "B+Aa"),
    ("zwnj n a", "N+A", "N+A"),
    ("n a zwnj", "N+A", "N+A"),
    ("zwnj a", "A+A", "A+A"),
    ("a zwnj", "A+A", "A+A"),
    // control-only input: a lone ZWNJ is the whole shape (like a lone ZWJ)
    ("zwnj", "Zwnj", ""),
    ("zwnj zwnj", "Zwnj", ""),
    ("zwnj fvs1", "Zwnj", ""),
    ("zwnj mvs", "Mvs", "Mvs"),
    ("mvs zwnj", "Mvs", "Mvs"),
    ("zwj zwnj", "Zwnj", "Zwj"),
    ("zwnj zwj", "Zwnj", "Zwj"),
    // repeated breaks collapse to one
    ("a zwnj zwnj b", "A+A+Zwnj+B", "A+A+B"),
    ("zwnj b zwnj", "B", "B"),
    // between letters: the reviewed edge classes decide
    ("b zwnj b a", "B+B+Aa", "B+B+Aa"), // B:isol≡init, B:init≡medi
    ("a o zwnj b a", "A+A+U+Zwnj+B+Aa", "A+A+O+B+Aa"),
    ("b zwnj a", "B+Zwnj+A+A", "B+Aa"),
    ("n zwnj a", "N+Zwnj+A+A", "N+A"),
    ("a zwnj a", "A+A+Zwnj+A+A", "A+A+A"),
    ("a b zwnj b a", "A+A+B+Zwnj+B+Aa", "A+A+B+B+Aa"),
    ("l i zwnj e fvs1", "L+I+Zwnj+Aa", "L+G"), // no `G` across a kept break
    ("s zwnj g", "S+Zwnj+Hx", "S+G"),
    ("b zwnj s", "B+Zwnj+S", "B+S"),
    ("t a l zwnj a", "T+A+L+Zwnj+A+A", "T+A+L+A"),
    ("a zwnj b zwnj a", "A+A+Zwnj+B+Zwnj+A+A", "A+A+B+Aa"),
    ("a zwnj n zwnj a", "A+A+Zwnj+N+Zwnj+A+A", "A+A+N+A"),
    // beside an MVS / NNBSP: the MVS already breaks joining, and context looks through the ZWNJ
    ("mvs zwnj a", "Mvs+Aa", "Mvs+Aa"), // chachlag through the break
    ("zwnj mvs a", "Mvs+Aa", "Mvs+Aa"),
    ("a zwnj mvs a", "A+A+Mvs+Aa", "A+A+Mvs+Aa"),
    ("a zwnj nnbsp a", "A+A+Mvs+Aa", "A+A+Mvs+Aa"),
    ("mvs zwnj i", "Mvs+I", "Mvs+I"), // particle through the break
    ("mvs zwnj u n", "Mvs+O+A", "Mvs+O+A"),
    ("mvs zwnj y i n", "Mvs+I+I+A", "Mvs+I+I+A"),
    ("n zwnj mvs a", "N+Mvs+Aa", "N+Mvs+Aa"),
    ("a n zwnj mvs a", "A+A+N+Mvs+Aa", "A+A+N+Mvs+Aa"), // chachlag onset
    ("s g zwnj mvs a", "S+H+Mvs+Aa", "S+H+Mvs+Aa"),     // dotless g through the break
    ("g zwnj mvs a", "Hx+Mvs+Aa", "Hx+Mvs+Aa"),
    ("t a l mvs zwnj a", "T+A+L+Mvs+Aa", "T+A+L+Mvs+Aa"),
    // a break inside a particle ends it — the font's patterns are keyed on joining positions
    ("mvs i zwnj y a r", "Mvs+I+Zwnj+Y+A+R", "Mvs+I+I+A+R"),
    ("mvs y zwnj i n", "Mvs+Y+Zwnj+A+I+A", "Mvs+I+I+A"),
    ("mvs u zwnj u", "Mvs+U+Zwnj+A+O", "Mvs+O+U"),
    ("a mvs d u zwnj r", "A+A+Mvs+D+U+Zwnj+R", "A+A+Mvs+D+O+R"),
    // beside a nirugu: the nirugu joins its own side
    ("n zwnj nirugu", "N+Nirugu", "N+Nirugu"), // N:isol ≡ N:init
    ("zwnj nirugu i e fvs1", "Nirugu+G", "Nirugu+G"),
    ("a zwnj nirugu b", "A+A+Zwnj+Nirugu+B", "A+A+Nirugu+B"),
    ("a nirugu zwnj b", "A+A+Nirugu+Zwnj+B", "A+A+Nirugu+B"),
    ("nirugu zwnj b", "Nirugu+Zwnj+B", "Nirugu+B"),
    // beside a ZWJ, both orders
    ("b zwnj zwj a", "B+Aa", "B+Aa"),
    ("n zwnj zwj a", "N+A", "N+A"),
    ("zwj zwnj n a", "N+A", "Zwj+N+A"),
    ("n a zwnj zwj", "N+A", "N+A+Zwj"),
    ("a zwj zwnj b a", "A+A+B+Aa", "A+A+B+Aa"),
    ("a zwj zwnj b", "A+A+Zwj+Zwnj+B", "A+A+B"),
    ("a zwnj zwj b", "A+A+Zwnj+Zwj+B", "A+A+B"),
    ("b zwj zwnj a", "B+Zwnj+A+A", "B+Aa"),
    // an FVS after a ZWNJ selects nothing (it is the ZWNJ's); one before it is the letter's
    ("a zwnj fvs1 a", "A+A+Zwnj+A+A", "A+A"),
    ("a fvs1 zwnj a", "A+Zwnj+A+A", "A+A"),
    ("b a fvs1 zwnj", "B+Aa", "B+Aa"),
    // gender and harmony cross the break (positions do not)
    ("n ue zwnj h nirugu", "N+Ue+Zwnj+G+Nirugu", "N+O+I+G+Nirugu"),
    ("h zwnj oe", "H+Zwnj+A+Ue", "G+Ue"),
    ("g zwnj e", "Hx+Zwnj+A", "G+Aa"),
    ("a zwnj g", "A+A+Zwnj+Hx", "A+A+H"),
    ("a l zwnj g", "A+A+L+Zwnj+Hx", "A+A+L+G"),
    ("a b zwnj g", "A+A+B+Zwnj+Hx", "A+A+B+G"),
    ("i g zwnj b a", "A+I+H+Zwnj+B+Aa", "A+I+A+A+B+Aa"), // backward marker crosses
    ("i g zwnj a", "A+I+G+Zwnj+A+A", "A+I+N+N+A"),
    ("e b zwnj i g", "A+B+Zwnj+A+I+G", "A+B+I+G"),
    ("s g zwnj a", "S+G+Zwnj+A+A", "S+N+N+A"),
    ("m n oe zwnj g e", "M+N+U+Zwnj+G+Aa", "M+N+O+I+G+Aa"),
    // the issue's corpus (ZWJ + ZWNJ): three words lose both controls, the first keeps neither
    // but its `ue` stays marked because the break made the `p` initial — and `P:init` is the
    // same shape as `P:medi`, so the shape is the break-free `L A L A P O I O A`
    (
        "l a l a zwj zwnj p ue d",
        "L+A+L+A+P+O+I+O+A",
        "L+A+L+A+P+O+O+A",
    ),
    (
        "t a ng zwj zwnj p u r u m",
        "T+A+A+G+P+O+R+O+M",
        "T+A+A+G+P+O+R+O+M",
    ),
    (
        "t i l zwj zwnj p u r u g",
        "T+I+L+P+O+R+O+H",
        "T+I+L+P+O+R+O+H",
    ),
    (
        "s e ng zwj zwnj p u r u m",
        "S+A+A+G+P+O+R+O+M",
        "S+A+A+G+P+O+R+O+M",
    ),
    // the same words with the ZWNJ alone keep it: `A:fina`/`G:fina`/`L:fina` are not their
    // `medi` forms
    (
        "l a l a zwnj p ue d",
        "L+A+L+A+Zwnj+P+O+I+O+A",
        "L+A+L+A+P+O+O+A",
    ),
    (
        "t a ng zwnj p u r u m",
        "T+A+A+G+Zwnj+P+O+R+O+M",
        "T+A+A+G+P+O+R+O+M",
    ),
    (
        "t i l zwnj p u r u g",
        "T+I+L+Zwnj+P+O+R+O+H",
        "T+I+L+P+O+R+O+H",
    ),
];

#[test]
fn drop_and_keep_table() {
    let shaper = shaper();
    for (aliases, with, without) in TABLE {
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
    // Words 2–4 are the same shape as their plain spelling.
    for (word, plain) in [
        (word2, "t a ng p u r u m"),
        (word3, "t i l p u r u g"),
        (word4, "s e ng p u r u m"),
    ] {
        let plain = mgl(plain);
        assert!(shaper.same_shape(word, &plain).unwrap(), "{}", hex(word));
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
fn fvs_after_a_break_is_the_breaks() {
    let shaper = shaper();
    // It stays with the ZWNJ, like one after a joiner, and selects no variant.
    let details = shaper.shape_detailed(&mgl("a zwnj fvs1 a")).unwrap();
    assert_eq!(details.len(), 3);
    assert_eq!(details[0].fvs, None);
    assert_eq!(details[1].fvs, Some(mongol_norm::Fvs::Fvs1));
    assert_eq!(details[2].fvs, None);
    assert!(shaper
        .same_shape(&mgl("a zwnj fvs1 a"), &mgl("a zwnj a"))
        .unwrap());
    // An FVS right before the break still belongs to its letter.
    let details = shaper.shape_detailed(&mgl("a fvs1 zwnj a")).unwrap();
    assert_eq!(details[0].fvs, Some(mongol_norm::Fvs::Fvs1));
    // GB.A's lookup skips the ZWNJ, as it skips a joiner, and reads the FVS as one after the
    // vowel before the break: the marked o after an initial t goes back to its default, as in
    // the font.
    assert_eq!(shape(&mgl("t o zwnj")), "T+O");
    assert_eq!(shape(&mgl("t o zwnj fvs3")), "T+U");
    assert_eq!(shape(&mgl("t o zwnj fvs3")), shape(&mgl("t o fvs3")));
    assert_eq!(shape(&mgl("sh ue zwnj fvs3 nnbsp o")), "Sh+U+Mvs+A+O");
}

/// Where a ZWNJ meets an FVS, a joiner, a particle or an MVS, the engine shapes as the reference
/// font does (each row checked with `FontShaper`; `tests/font_vs_engine.rs` keeps them aligned).
const FONT_CASES: &[(&str, &str)] = &[
    // An FVS a ZWNJ carries counts for the FVS-reading lookups, which skip the ZWNJ.
    ("t o zwnj fvs3", "T+U"),
    ("t o zwnj", "T+O"),
    ("d e zwnj fvs3", "T+A"),
    ("d e zwnj", "D+A"),
    ("a j nnbsp a zwnj fvs4", "A+A+J+Mvs+Aa"),
    ("a g mvs zwnj fvs1 a zwnj fvs3", "A+A+Hx+Mvs+Aa"),
    ("a n mvs zwnj fvs1 a zwnj fvs3", "A+A+N+Mvs+Aa"),
    ("a j nnbsp a zwnj", "A+A+I+Mvs+Aa"),
    ("nnbsp ue zwnj fvs3", "Mvs+A+Ue"),
    ("nnbsp ue zwnj", "Mvs+U"),
    ("mvs y i n zwnj fvs1", "Mvs+Y+I+A"),
    ("mvs y i n zwnj", "Mvs+I+I+A"),
    ("mvs i zwnj zwj fvs2", "Mvs+A+I"),
    // A particle follows the joining chain: two ZWJs join across a ZWNJ.
    ("u zwj zwnj zwj u", "O+U"),
    ("u zwnj zwj u", "A+O+U"),
    ("mvs y zwj zwnj zwj i", "Mvs+I+I"),
    ("a r nirugu zwnj ue ue", "A+A+R+Nirugu+O+U"),
    // A ZWNJ between two joiners changes no position; the font ligates across it.
    ("b zwj zwnj zwj ee zwj zwnj", "B+W+Zwj"),
    ("t k zwj zwnj zwj u j k2 e", "T+K+O+J+K2+Aa"),
    // post_bowed reads a final or isolated consonant before its FVS applies.
    ("g fvs2 zwnj zwj e", "G+A"),
    ("a b fvs1 zwnj zwj e", "A+A+B2+Zwnj+Aa"),
    ("a g fvs1 zwnj zwj e", "A+A+H+Zwnj+Zwj+A"),
    ("a g fvs2 zwnj zwj o", "A+A+G+Zwnj+Zwj+O"),
    ("t g fvs2 zwnj nirugu o", "T+G+Zwnj+Nirugu+O"),
    // The lookups that need an MVS beside a letter look through a run that holds a ZWNJ.
    ("a n mvs zwj zwnj a", "A+A+N+Mvs+Aa"),
    ("a n mvs zwnj zwj a", "A+A+A+Mvs+Zwj+A"),
    ("e g mvs zwj zwnj e", "A+H+Mvs+Aa"),
    ("a y a g mvs zwj zwnj a", "A+A+Y+A+Hx+Mvs+Aa"),
    (
        "i g sh i n fvs2 mvs zwnj fvs2 zwnj zwnj a",
        "A+I+A+A+S+I+A+Mvs+Aa",
    ),
];

#[test]
fn follows_the_font_around_the_break() {
    for (aliases, expected) in FONT_CASES {
        assert_eq!(shape(&mgl(aliases)), *expected, "{aliases}");
    }
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
        &[
            "a b a",
            "a zwj zwnj b a",
            "a zwj zwj b a",
            "a zwj zwnj zwj b a",
            "a zwj zwnj zwnj zwj b a",
        ],
        // A break between two joiners breaks nothing: the shape is the joined word.
        &["b o", "b zwj zwnj zwj o"],
        &["b nirugu nirugu o", "b nirugu zwnj nirugu o"],
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
        for aliases in &group[1..] {
            let text = mgl(aliases);
            assert_eq!(
                shaper.shape(&text).unwrap(),
                shape0,
                "{aliases} vs {}",
                group[0]
            );
        }
    }
}

#[test]
fn error_message_names_zwnj() {
    let message = shaper().shape("\u{1820}x").unwrap_err().to_string();
    assert!(
        message.contains("FVS/MVS/NNBSP/Nirugu/ZWJ/ZWNJ"),
        "{message}"
    );
}

/// Deterministic xorshift, ZWNJ-dense alphabet: no panics, no adjacent `Zwnj`s in the public
/// shape, and `shape_raw` keeps every ZWNJ.
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
    }
    assert!(kept > 1_000, "kept {kept}");
}
