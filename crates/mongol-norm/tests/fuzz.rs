//! Randomized no-panic test: a deterministic xorshift stream of Mongolian-block strings through
//! every public entry point. Nothing may panic; every successful strict normalization must
//! round-trip through `shape` and be idempotent.

use mongol_norm::{Locale, Shaper};

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

#[test]
fn never_panics_and_strict_normalization_round_trips() {
    let shaper = Shaper::new(Locale::Mng);
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut normalized = 0;
    for _ in 0..20_000 {
        let text = random_text(&mut rng);
        let _ = shaper.shape_detailed(&text);
        let _ = shaper.trace(&text);
        let _ = shaper.normalize_allow_fallback(&text);
        let _ = shaper.normalize_text(&text);
        let _ = shaper.normalize_text_allow_fallback(&text);
        let _ = shaper.parse_written_units(&text);
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
        normalized > 1_000,
        "too few strict normalizations succeeded: {normalized}"
    );
}

#[test]
fn other_locales_never_panic() {
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    for locale in [Locale::Tod, Locale::Sib, Locale::Mch] {
        let shaper = Shaper::new(locale);
        for _ in 0..2_000 {
            let text = random_text(&mut rng);
            let _ = shaper.shape(&text);
            let _ = shaper.shape_detailed(&text);
            let _ = shaper.trace(&text);
            let _ = shaper.normalize(&text);
            let _ = shaper.normalize_text_allow_fallback(&text);
        }
    }
}
