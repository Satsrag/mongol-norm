//! Randomized no-panic test: a deterministic xorshift stream of Mongolian-block strings through
//! every public entry point. Nothing may panic; every successful strict normalization must
//! round-trip through `shape` and be idempotent.

use mongol_norm::{Locale, PositionedWrittenUnit, Shaper, UnitPosition, WrittenUnit};

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Every MNG letter, the four FVS, MVS, NNBSP, nirugu, ZWJ, two letters without MNG variants
/// (unassigned U+181A, Todo U+1843), and a few non-word characters.
const ALPHABET: &[u32] = &[
    0x1820, 0x1821, 0x1822, 0x1823, 0x1824, 0x1825, 0x1826, 0x1827, 0x1828, 0x1829, 0x182A, 0x182B,
    0x182C, 0x182D, 0x182E, 0x182F, 0x1830, 0x1831, 0x1832, 0x1833, 0x1834, 0x1835, 0x1836, 0x1837,
    0x1838, 0x1839, 0x183A, 0x183B, 0x183C, 0x183D, 0x183E, 0x183F, 0x1840, 0x1841, 0x1842, 0x180B,
    0x180C, 0x180D, 0x180F, 0x180E, 0x202F, 0x180A, 0x200D, 0x181A, 0x1843, 0x0020, 0x0061, 0x1800,
    0x1810, 0x200C,
];

fn random_text(rng: &mut XorShift) -> String {
    let len = rng.below(13);
    (0..len)
        .map(|_| char::from_u32(ALPHABET[rng.below(ALPHABET.len())]).unwrap())
        .collect()
}

/// A random 0–5 record positioned request. Any unit may be paired with any position, so most of
/// these are rejected by the exact-position audit — the point is that none of them panics.
fn random_records(rng: &mut XorShift) -> Vec<PositionedWrittenUnit> {
    let len = rng.below(6);
    (0..len)
        .map(|_| {
            let unit = WrittenUnit::ALL[rng.below(WrittenUnit::ALL.len())];
            let position = UnitPosition::ALL[rng.below(UnitPosition::ALL.len())];
            PositionedWrittenUnit::new(unit, position)
        })
        .collect()
}

/// Every public entry point, on one random input (`other` is a second random string, for
/// `same_shape`). No call may panic, whatever it returns.
fn exercise(shaper: &Shaper, rng: &mut XorShift, text: &str, other: &str) {
    let _ = shaper.shape(text);
    let _ = shaper.shape_str(text);
    let _ = shaper.shape_detailed(text);
    let _ = shaper.trace(text);
    let _ = shaper.same_shape(text, other);
    let _ = shaper.normalize(text);
    let _ = shaper.normalize_allow_fallback(text);
    let _ = shaper.normalize_text(text);
    let _ = shaper.normalize_text_allow_fallback(text);
    // Whatever parses as a unit stream must also survive being encoded again.
    if let Ok(units) = shaper.parse_written_units(text) {
        let _ = shaper.normalize_written_units(&units);
    }
    let _ = shaper.normalize_positioned_written_units(&random_records(rng));
}

#[test]
fn never_panics_and_strict_normalization_round_trips() {
    let shaper = Shaper::new(Locale::Mng);
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut normalized = 0;
    for _ in 0..20_000 {
        let text = random_text(&mut rng);
        let other = random_text(&mut rng);
        exercise(&shaper, &mut rng, &text, &other);
        let Ok(shape) = shaper.shape(&text) else {
            continue;
        };
        if let Ok(norm) = shaper.normalize(&text) {
            normalized += 1;
            assert_eq!(
                shaper.shape(&norm).unwrap(),
                shape,
                "round trip of {text:?}"
            );
            assert_eq!(
                shaper.normalize(&norm).unwrap(),
                norm,
                "idempotence of {text:?}"
            );
            assert_eq!(
                shaper.normalize_text(&text).unwrap(),
                norm,
                "normalize_text of {text:?}"
            );
        }
    }
    assert!(
        normalized > 10_000,
        "too few strict normalizations succeeded: {normalized}"
    );
}

#[test]
fn other_locales_never_panic() {
    // A fixed seed per locale, so each locale gets its own reproducible stream and adding a
    // locale cannot shift the inputs the others see.
    for (locale, seed) in [
        (Locale::Tod, 0x2545_F491_4F6C_DD1D_u64),
        (Locale::Sib, 0x1D8E_4E27_C47D_124F),
        (Locale::Mch, 0x0F1E_2D3C_4B5A_6978),
    ] {
        let shaper = Shaper::new(locale);
        let mut rng = XorShift(seed);
        for _ in 0..2_000 {
            let text = random_text(&mut rng);
            let other = random_text(&mut rng);
            exercise(&shaper, &mut rng, &text, &other);
        }
    }
}
