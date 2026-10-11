//! Tokens of the shaping pipeline: one per letter (with its trailing FVS run), MVS, nirugu, ZWJ
//! or ZWNJ; plus tokenization and structural position assignment.

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::tables::{Fvs, Position};
use crate::unicode::{is_mongolian_letter_cp, MVS, NIRUGU, NNBSP, ZWJ, ZWNJ};

/// Does a ZWNJ (U+200C) stop the phase-III context rules, the way an MVS does?
///
/// This is the one place that decides how the rules see a ZWNJ. It breaks cursive joining
/// regardless: the letter before it takes its `fina` (or `isol`) form and the letter after it
/// its `init` (or `isol`) form, exactly as [`assign_positions`] splits segments at it. What the
/// switch governs is whether the vowel-harmony, marker, chachlag, dotless and particle rules
/// look *through* the break (`false`, the shipped behaviour) or treat it as a wall (`true`).
///
/// `false` follows the reference font (mongfontbuilder `hudum.otf`, Kushim-Jiang/mongolian#58):
/// phase III substitutes ZWNJ to an ignored glyph before any rule runs, and every rule lookup
/// skips ignored glyphs, so context crosses a ZWNJ the same way it crosses a nirugu — only the
/// joining positions changed. MVS stays a real glyph in the font and keeps blocking.
pub(crate) const fn zwnj_blocks_context() -> bool {
    false
}

/// What a token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// A Mongolian letter.
    Letter,
    /// MVS (U+180E) — NNBSP is folded into this at tokenization.
    Mvs,
    /// Nirugu (U+180A).
    Nirugu,
    /// ZWJ (U+200D). Grouped with nirugu for every rule (Python `is_nirugu`), but kept apart so
    /// the shape emits `Zwj` rather than `Nirugu`.
    Zwj,
    /// ZWNJ (U+200C). Splits the joining chain like an MVS, but is not a glyph: the context
    /// rules look through it (see [`zwnj_blocks_context`]).
    Zwnj,
}

/// One token. Field semantics follow the Python `Token` class.
#[derive(Clone, Debug)]
pub(crate) struct Token {
    /// The token's kind: letter, MVS, nirugu, ZWJ or ZWNJ.
    pub kind: TokenKind,
    /// Code point (MVS for NNBSP input).
    pub cp: u32,
    /// The letter's alias in the shaper's locale; `None` for structural tokens and for letters
    /// this locale does not know (they shape to nothing).
    pub alias: Option<Alias>,
    /// Every FVS directly following the letter, in stream order.
    pub fvs: Vec<Fvs>,
    /// Structural position; stays `Isol` for non-letters.
    pub position: Position,
    /// Condition assigned by the rule pipeline.
    pub condition: Option<Condition>,
    /// Lazily resolved written units (memoised exactly like Python's `tok.written`): a slice
    /// borrowed from the generated tables, or the empty slice for structural tokens and letters
    /// this locale has no variant for.
    pub written: Option<&'static [WrittenUnit]>,
}

impl Token {
    fn new(kind: TokenKind, cp: u32, alias: Option<Alias>) -> Token {
        Token {
            kind,
            cp,
            alias,
            fvs: Vec::new(),
            position: Position::Isol,
            condition: None,
            written: None,
        }
    }

    /// Is this a letter token (as opposed to MVS, nirugu, ZWJ or ZWNJ)?
    pub fn is_letter(&self) -> bool {
        self.kind == TokenKind::Letter
    }

    /// Is this an MVS token?
    pub fn is_mvs(&self) -> bool {
        self.kind == TokenKind::Mvs
    }

    /// Nirugu *or* ZWJ — the Python `is_nirugu` flag covers both.
    pub fn is_nirugu(&self) -> bool {
        matches!(self.kind, TokenKind::Nirugu | TokenKind::Zwj)
    }

    /// Is this a ZWNJ token?
    pub fn is_zwnj(&self) -> bool {
        self.kind == TokenKind::Zwnj
    }

    /// Do the context rules look through this token as if it were not there? Nirugu and ZWJ
    /// always; a ZWNJ unless [`zwnj_blocks_context`] says it is a wall. A letter or an MVS
    /// never is.
    pub fn is_transparent(&self) -> bool {
        self.is_nirugu() || (self.is_zwnj() && !zwnj_blocks_context())
    }

    /// Does this token end the joining chain — an MVS or a ZWNJ?
    pub fn breaks_joining(&self) -> bool {
        matches!(self.kind, TokenKind::Mvs | TokenKind::Zwnj)
    }

    /// Does this token carry at least one FVS?
    pub fn has_fvs(&self) -> bool {
        !self.fvs.is_empty()
    }

    /// The first FVS (Python `tok.fvs_cp`).
    pub fn first_fvs(&self) -> Option<Fvs> {
        self.fvs.first().copied()
    }

    /// Python `_written_ends_with`: resolved, non-empty, and ending in `unit`.
    pub fn written_ends_with(&self, unit: WrittenUnit) -> bool {
        self.written.and_then(<[WrittenUnit]>::last) == Some(&unit)
    }
}

/// Split text into tokens. Letters absorb every directly following FVS; MVS/NNBSP become `Mvs`;
/// nirugu and ZWJ become joining tokens; ZWNJ becomes a joining break; everything else —
/// including an FVS that does not directly follow a letter, e.g. one after a ZWNJ — is skipped.
pub(crate) fn tokenize(text: &str, alias_of: impl Fn(u32) -> Option<Alias>) -> Vec<Token> {
    let cps: Vec<u32> = text.chars().map(|c| c as u32).collect();
    let mut tokens = Vec::with_capacity(cps.len());
    let mut i = 0;
    while i < cps.len() {
        let cp = cps[i];
        if is_mongolian_letter_cp(cp) {
            let mut token = Token::new(TokenKind::Letter, cp, alias_of(cp));
            let mut j = i + 1;
            while let Some(fvs) = cps.get(j).copied().and_then(Fvs::from_cp) {
                token.fvs.push(fvs);
                j += 1;
            }
            tokens.push(token);
            i = j;
        } else if cp == MVS || cp == NNBSP {
            tokens.push(Token::new(TokenKind::Mvs, MVS, None));
            i += 1;
        } else if cp == NIRUGU {
            tokens.push(Token::new(TokenKind::Nirugu, NIRUGU, None));
            i += 1;
        } else if cp == ZWJ {
            tokens.push(Token::new(TokenKind::Zwj, ZWJ, None));
            i += 1;
        } else if cp == ZWNJ {
            tokens.push(Token::new(TokenKind::Zwnj, ZWNJ, None));
            i += 1;
        } else {
            i += 1;
        }
    }
    tokens
}

/// Assign `isol`/`init`/`medi`/`fina` to letters. Segments are split at MVS and at ZWNJ;
/// letters *and* joining tokens count toward a segment's length (a nirugu extends the chain)
/// but only letters receive a position.
pub(crate) fn assign_positions(tokens: &mut [Token]) {
    let mut segments: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.is_letter() || token.is_nirugu() {
            current.push(i);
        } else if token.breaks_joining() && !current.is_empty() {
            segments.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    for segment in segments {
        let n = segment.len();
        for (k, &i) in segment.iter().enumerate() {
            if !tokens[i].is_letter() {
                continue;
            }
            tokens[i].position = if n == 1 {
                Position::Isol
            } else if k == 0 {
                Position::Init
            } else if k == n - 1 {
                Position::Fina
            } else {
                Position::Medi
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_alias(_: u32) -> Option<Alias> {
        None
    }

    fn positions(text: &str) -> Vec<(TokenKind, Position)> {
        let mut tokens = tokenize(text, no_alias);
        assign_positions(&mut tokens);
        tokens.iter().map(|t| (t.kind, t.position)).collect()
    }

    #[test]
    fn letters_absorb_their_fvs_run() {
        let tokens = tokenize("\u{1820}\u{180B}\u{180C}\u{1828}", no_alias);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].fvs, vec![Fvs::Fvs1, Fvs::Fvs2]);
        assert_eq!(tokens[0].first_fvs(), Some(Fvs::Fvs1));
        assert!(tokens[0].has_fvs());
        assert!(!tokens[1].has_fvs());
    }

    #[test]
    fn nnbsp_becomes_mvs_and_joiners_keep_their_kind() {
        let tokens = tokenize(
            "\u{202F}\u{180E}\u{180A}\u{200D}\u{200C}x \u{180B}",
            no_alias,
        );
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Mvs,
                TokenKind::Mvs,
                TokenKind::Nirugu,
                TokenKind::Zwj,
                TokenKind::Zwnj
            ]
        );
        assert_eq!(tokens[0].cp, MVS);
        assert!(tokens[2].is_nirugu() && tokens[3].is_nirugu());
        assert!(!tokens[4].is_nirugu() && tokens[4].is_zwnj());
        assert_eq!(tokens[4].cp, ZWNJ);
        // Nirugu and ZWJ are always transparent to the rules; MVS never; ZWNJ follows the switch.
        assert!(tokens[2].is_transparent() && tokens[3].is_transparent());
        assert!(!tokens[0].is_transparent());
        assert_eq!(tokens[4].is_transparent(), !zwnj_blocks_context());
        assert!(tokens[0].breaks_joining() && tokens[4].breaks_joining());
        assert!(!tokens[2].breaks_joining() && !tokens[3].breaks_joining());
    }

    #[test]
    fn zwnj_splits_the_joining_chain_like_an_mvs() {
        use Position::*;
        use TokenKind::*;
        // `t a l` + ZWNJ + `a`: the l is final, the a isolated — exactly the MVS picture.
        assert_eq!(
            positions("\u{1832}\u{1820}\u{182F}\u{200C}\u{1820}"),
            vec![
                (Letter, Init),
                (Letter, Medi),
                (Letter, Fina),
                (Zwnj, Isol),
                (Letter, Isol)
            ]
        );
        // Leading, trailing and doubled ZWNJs leave the letters as they would be without them.
        assert_eq!(
            positions("\u{200C}\u{1820}\u{182A}\u{200C}"),
            vec![(Zwnj, Isol), (Letter, Init), (Letter, Fina), (Zwnj, Isol)]
        );
        assert_eq!(
            positions("\u{1820}\u{200C}\u{200C}\u{1820}"),
            vec![(Letter, Isol), (Zwnj, Isol), (Zwnj, Isol), (Letter, Isol)]
        );
        // A joiner on the far side of the ZWNJ still joins its own side: ZWJ + ZWNJ + `b` is a
        // lone `b`, ZWNJ + ZWJ + `b` is a final `b`.
        assert_eq!(
            positions("\u{1820}\u{200D}\u{200C}\u{182A}"),
            vec![(Letter, Init), (Zwj, Isol), (Zwnj, Isol), (Letter, Isol)]
        );
        assert_eq!(
            positions("\u{1820}\u{200C}\u{200D}\u{182A}"),
            vec![(Letter, Isol), (Zwnj, Isol), (Zwj, Isol), (Letter, Fina)]
        );
        // An FVS attaches only to the letter directly before it; across a ZWNJ it is dropped.
        let tokens = tokenize("\u{1820}\u{200C}\u{180B}\u{1820}", no_alias);
        assert_eq!(tokens.len(), 3);
        assert!(tokens.iter().all(|token| !token.has_fvs()));
        let tokens = tokenize("\u{1820}\u{180B}\u{200C}\u{1820}", no_alias);
        assert_eq!(tokens[0].fvs, vec![Fvs::Fvs1]);
        assert!(tokens[1].is_zwnj());
    }

    #[test]
    fn structural_positions() {
        use Position::*;
        use TokenKind::*;
        assert_eq!(positions("\u{1828}"), vec![(Letter, Isol)]);
        assert_eq!(
            positions("\u{1820}\u{182A}"),
            vec![(Letter, Init), (Letter, Fina)]
        );
        assert_eq!(
            positions("\u{1832}\u{1820}\u{182F}"),
            vec![(Letter, Init), (Letter, Medi), (Letter, Fina)]
        );
        // MVS breaks the chain; the MVS token itself keeps Isol.
        assert_eq!(
            positions("\u{1832}\u{1820}\u{182F}\u{180E}\u{1820}"),
            vec![
                (Letter, Init),
                (Letter, Medi),
                (Letter, Fina),
                (Mvs, Isol),
                (Letter, Isol)
            ]
        );
        // Nirugu extends the chain without taking a position.
        assert_eq!(
            positions("\u{180A}\u{1820}"),
            vec![(Nirugu, Isol), (Letter, Fina)]
        );
        assert_eq!(
            positions("\u{1820}\u{180A}"),
            vec![(Letter, Init), (Nirugu, Isol)]
        );
        assert_eq!(
            positions("\u{180A}\u{1820}\u{180A}"),
            vec![(Nirugu, Isol), (Letter, Medi), (Nirugu, Isol)]
        );
        assert_eq!(
            positions("\u{200D}\u{1833}"),
            vec![(Zwj, Isol), (Letter, Fina)]
        );
    }

    #[test]
    fn written_ends_with_requires_resolution() {
        let mut token = tokenize("\u{1820}", no_alias).remove(0);
        assert!(!token.written_ends_with(WrittenUnit::A));
        token.written = Some(&[WrittenUnit::A, WrittenUnit::A]);
        assert!(token.written_ends_with(WrittenUnit::A));
        assert!(!token.written_ends_with(WrittenUnit::I));
    }

    #[test]
    fn alias_of_is_threaded_onto_letters_only() {
        let tokens = tokenize("\u{1820}\u{180B}\u{180E}", |cp| {
            (cp == 0x1820).then_some(Alias::A)
        });
        assert_eq!(tokens[0].alias, Some(Alias::A)); // from the letter, not its FVS
        assert_eq!(tokens[1].alias, None); // structural token
    }

    #[test]
    fn empty_and_mvs_edge_cases() {
        use Position::*;
        use TokenKind::*;

        assert!(tokenize("", no_alias).is_empty());
        let mut empty: Vec<Token> = Vec::new();
        assign_positions(&mut empty);
        assert!(empty.is_empty());

        // Leading MVS.
        assert_eq!(
            positions("\u{180E}\u{1820}"),
            vec![(Mvs, Isol), (Letter, Isol)]
        );

        // Doubled MVS: no empty-segment problems, both letters stay Isol.
        assert_eq!(
            positions("\u{1820}\u{180E}\u{180E}\u{1820}"),
            vec![(Letter, Isol), (Mvs, Isol), (Mvs, Isol), (Letter, Isol)]
        );

        // A leading FVS (nothing precedes it) is dropped, not attached to anything.
        let tokens = tokenize("\u{180B}\u{1820}", no_alias);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, Letter);
        assert!(!tokens[0].has_fvs());
    }
}
