//! Code-point constants and the character classification the shaper relies on.

// dead_code: TEMPORARY — `check_word_chars` is only exercised by this module's own unit tests
// until Task 5's `Shaper::shape`/`normalize` call it (mirroring Python's `_check_word_chars`);
// remove this allow then.
#![allow(dead_code)]

use crate::tables::Fvs;
use crate::Error;

/// U+180E MONGOLIAN VOWEL SEPARATOR.
pub(crate) const MVS: u32 = 0x180E;
/// U+180A MONGOLIAN NIRUGU (visible stem extender, a joining marker).
pub(crate) const NIRUGU: u32 = 0x180A;
/// U+200D ZERO WIDTH JOINER.
pub(crate) const ZWJ: u32 = 0x200D;
/// U+202F NARROW NO-BREAK SPACE — folded into MVS at tokenization, like Python does.
pub(crate) const NNBSP: u32 = 0x202F;

const fn is_fvs(cp: u32) -> bool {
    Fvs::from_cp(cp).is_some()
}

/// Is `cp` a Mongolian letter (not FVS / MVS / nirugu / punctuation / digit)?
///
/// Mirrors the Python predicate exactly: everything in the Mongolian block `U+1800..=U+18AF`
/// that is not excluded counts — including the unassigned `U+181A..=U+181F`, which then shape
/// to nothing.
pub(crate) const fn is_mongolian_letter_cp(cp: u32) -> bool {
    if is_fvs(cp) || cp == MVS || cp == NIRUGU {
        return false;
    }
    if cp >= 0x1800 && cp < 0x180A {
        return false; // punctuation
    }
    if cp >= 0x1810 && cp < 0x181A {
        return false; // digits
    }
    cp >= 0x1800 && cp < 0x18B0
}

/// Is `c` a Mongolian letter (not FVS / MVS / nirugu / punctuation / digit)?
pub fn is_mongolian_letter(c: char) -> bool {
    is_mongolian_letter_cp(c as u32)
}

/// Does `cp` participate in a Mongolian word run: a letter, an FVS, MVS, NNBSP, nirugu or ZWJ?
pub(crate) const fn is_mongolian_word_char_cp(cp: u32) -> bool {
    is_mongolian_letter_cp(cp) || is_fvs(cp) || matches!(cp, MVS | NNBSP | NIRUGU | ZWJ)
}

/// Does `c` participate in a Mongolian word run: a letter, an FVS, MVS, NNBSP, nirugu or ZWJ?
///
/// Strict word validation ([`Shaper::shape`](crate::Shaper::shape) /
/// [`Shaper::normalize`](crate::Shaper::normalize)) and mixed-text segmentation
/// ([`Shaper::normalize_text`](crate::Shaper::normalize_text)) share this classification.
pub fn is_mongolian_word_char(c: char) -> bool {
    is_mongolian_word_char_cp(c as u32)
}

/// Reject the first character that is not a Mongolian word character.
pub(crate) fn check_word_chars(text: &str) -> Result<(), Error> {
    match text
        .chars()
        .enumerate()
        .find(|(_, c)| !is_mongolian_word_char(*c))
    {
        Some((index, ch)) => Err(Error::NonMongolianChar { ch, index }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_controls() {
        assert!(is_mongolian_letter('\u{1820}'));
        assert!(is_mongolian_letter('\u{1842}'));
        assert!(is_mongolian_letter('\u{18AA}'));
        assert!(is_mongolian_letter('\u{181A}')); // unassigned but inside the block (Python parity)
        for cp in [
            0x180B, 0x180C, 0x180D, 0x180F, 0x180E, 0x180A, 0x1800, 0x1809, 0x1810, 0x1819,
        ] {
            assert!(
                !is_mongolian_letter(char::from_u32(cp).unwrap()),
                "U+{cp:04X}"
            );
        }
        assert!(!is_mongolian_letter('a'));
        assert!(!is_mongolian_letter('\u{200D}'));
        assert!(!is_mongolian_letter('\u{202F}'));
    }

    #[test]
    fn word_chars() {
        for cp in [0x1820, 0x180B, 0x180F, 0x180E, 0x202F, 0x180A, 0x200D] {
            assert!(
                is_mongolian_word_char(char::from_u32(cp).unwrap()),
                "U+{cp:04X}"
            );
        }
        for c in [' ', 'a', '\u{200C}', '\u{1800}', '\u{1810}', ','] {
            assert!(!is_mongolian_word_char(c), "{c:?}");
        }
    }

    #[test]
    fn check_word_chars_reports_the_first_offender() {
        assert_eq!(
            check_word_chars("\u{1820}\u{180B}\u{180E}\u{180A}\u{200D}\u{202F}"),
            Ok(())
        );
        assert_eq!(check_word_chars(""), Ok(()));
        assert_eq!(
            check_word_chars("\u{1820} \u{1820}x"),
            Err(Error::NonMongolianChar { ch: ' ', index: 1 })
        );
        let message = check_word_chars("\u{1820}x").unwrap_err().to_string();
        assert!(
            message.starts_with("non-Mongolian character 'x' (U+0078) at index 1:"),
            "{message}"
        );
    }
}
