//! The online encoder (`mng-canonical/3`) on inputs beyond the corpus: every input of up to two
//! symbols, random longer words heavy in MVS / nirugu / ZWJ, and every prefix of every corpus
//! word. For each word: `normalize` succeeds (strict), round-trips, and the encoding of every
//! encodable shape-prefix is — apart from its last letter — a prefix of the word's encoding.

mod common;

use std::collections::BTreeSet;

use common::{all_corpus_words, mgl, split_letters, unit_names};
use mongol_norm::{Locale, Shaper, WrittenUnit};

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

/// Letters (bare and with each FVS) and the three structural characters.
fn symbols() -> Vec<String> {
    let mut out = Vec::new();
    for cp in 0x1820..=0x1842u32 {
        let letter = char::from_u32(cp).unwrap();
        out.push(letter.to_string());
        for fvs in ['\u{180B}', '\u{180C}', '\u{180D}', '\u{180F}'] {
            out.push(format!("{letter}{fvs}"));
        }
    }
    for structural in ["\u{180E}", "\u{180A}", "\u{200D}"] {
        out.push(structural.to_owned());
    }
    out
}

/// Normalize `word` and check the round trip and the committed-prefix property against every
/// encodable prefix of its shape. Returns a failure description.
fn check(shaper: &Shaper, word: &str) -> Result<(), String> {
    let shape = shaper.shape(word).map_err(|e| format!("{word:?}: {e}"))?;
    if shape.is_empty() {
        return Ok(());
    }
    let encoded = shaper
        .normalize(word)
        .map_err(|e| format!("{word:?}: {e}"))?;
    if shaper.shape(&encoded).unwrap() != shape {
        return Err(format!("{word:?} -> {encoded:?} does not round-trip"));
    }
    let letters = split_letters(&encoded);
    for end in 1..shape.len() {
        let Ok(prefix) = shaper.normalize_written_units(&shape[..end]) else {
            continue; // not a shape of anything
        };
        let prefix_letters = split_letters(&prefix);
        let committed = &prefix_letters[..prefix_letters.len() - 1];
        if letters.len() < committed.len() || letters[..committed.len()] != *committed {
            return Err(format!(
                "{:?}: prefix {:?} encodes as {prefix_letters:?}, the word as {letters:?}",
                unit_names(&shape),
                unit_names(&shape[..end])
            ));
        }
    }
    Ok(())
}

fn report(failures: &[String], total: usize) {
    assert!(
        failures.is_empty(),
        "{} of {total} words fail:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

#[test]
fn every_input_of_up_to_two_symbols() {
    let shaper = shaper();
    let symbols = symbols();
    let mut seen: BTreeSet<Vec<WrittenUnit>> = BTreeSet::new();
    let mut failures = Vec::new();
    let mut words = 0;
    for first in &symbols {
        for second in std::iter::once("").chain(symbols.iter().map(String::as_str)) {
            let word = format!("{first}{second}");
            let shape = shaper.shape(&word).unwrap();
            if !seen.insert(shape) {
                continue;
            }
            words += 1;
            if let Err(failure) = check(&shaper, &word) {
                failures.push(failure);
            }
        }
    }
    assert!(words > 1500, "only {words} distinct shapes");
    report(&failures, words);
}

/// A small deterministic generator (xorshift), so the random words are the same on every run.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

#[test]
fn random_words_with_structural_characters() {
    let shaper = shaper();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut failures = Vec::new();
    let total = 2000;
    for _ in 0..total {
        let length = 4 + rng.below(9);
        let mut word = String::new();
        for index in 0..length {
            match rng.below(100) {
                r if r < 10 && index > 0 => word.push('\u{180E}'),
                r if r < 16 => word.push('\u{180A}'),
                r if r < 20 => word.push('\u{200D}'),
                _ => {
                    word.push(char::from_u32(0x1820 + rng.below(35) as u32).unwrap());
                    if rng.below(100) < 12 {
                        word.push(['\u{180B}', '\u{180C}', '\u{180D}', '\u{180F}'][rng.below(4)]);
                    }
                }
            }
        }
        if let Err(failure) = check(&shaper, &word) {
            failures.push(failure);
        }
    }
    report(&failures, total);
}

#[test]
fn every_prefix_of_every_corpus_word() {
    let shaper = shaper();
    let mut seen: BTreeSet<Vec<WrittenUnit>> = BTreeSet::new();
    let mut failures = Vec::new();
    for word in all_corpus_words() {
        if !seen.insert(shaper.shape(&word).unwrap()) {
            continue;
        }
        if let Err(failure) = check(&shaper, &word) {
            failures.push(failure);
        }
    }
    report(&failures, seen.len());
}

/// Worked examples (aliases in, aliases out). Everything but the last letter is committed as the
/// word is read; a letter carries an FVS only where a later continuation could otherwise change
/// its form (`s a i+fvs3 …`: `S A I I` is itself a shape, so its first `I` must be a single
/// medial `I` after a vowel); equally short spellings follow the preference order.
#[test]
fn worked_examples() {
    let shaper = shaper();
    for (input, expected) in [
        ("a", "a"),
        ("e", "e"),
        ("i", "i"),
        ("s a i n", "s a i fvs3 i n"),
        ("s e i n", "s a i fvs3 i n"),
        ("m o ng g o l", "m o ng n fvs1 n o l"),
        ("t ng r i", "t ng r i"),
        ("m o r i n", "m o r i n"),
        ("g e r", "g e r"),
        ("e n e", "e n e"),
        ("o r o n", "o r o n"),
        ("n o m", "n o m"),
        ("t a l mvs a", "t a l mvs a"),
        ("t a l mvs a mvs y i n", "t a l mvs a mvs j i n"),
        ("g e r mvs e ch e", "g e r mvs e ch e"),
        ("b a", "b a"),
        ("a b u", "a b o"),
        ("h o t a", "h o t a"),
        ("u l a g a n", "o l a n fvs1 n a n"),
    ] {
        assert_eq!(
            shaper.normalize(&mgl(input)).unwrap(),
            mgl(expected),
            "{input}"
        );
    }
}
