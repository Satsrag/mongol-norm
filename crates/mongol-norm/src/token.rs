//! Tokens of the shaping pipeline: one per letter (with its trailing FVS run), MVS, nirugu or
//! ZWJ; plus tokenization and structural position assignment.

// dead_code: TEMPORARY — `has_fvs`/`written_ends_with` are only consumed once the rules (Task 6)
// use them; remove once they do.
#![allow(dead_code)]

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::tables::{Fvs, Position};
use crate::unicode::{is_mongolian_letter_cp, MVS, NIRUGU, NNBSP, ZWJ};

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
}

/// One token. Field semantics follow the Python `Token` class.
#[derive(Clone, Debug)]
pub(crate) struct Token {
    /// The token's kind: letter, MVS, nirugu or ZWJ.
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

    /// Is this a letter token (as opposed to MVS, nirugu or ZWJ)?
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
/// nirugu and ZWJ become joining tokens; everything else is skipped.
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
        } else {
            i += 1;
        }
    }
    tokens
}

/// Assign `isol`/`init`/`medi`/`fina` to letters. Segments are split at MVS; letters *and*
/// joining tokens count toward a segment's length (a nirugu extends the chain) but only letters
/// receive a position.
pub(crate) fn assign_positions(tokens: &mut [Token]) {
    let mut segments: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.is_letter() || token.is_nirugu() {
            current.push(i);
        } else if token.is_mvs() && !current.is_empty() {
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
        let tokens = tokenize("\u{202F}\u{180E}\u{180A}\u{200D}x \u{180B}", no_alias);
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Mvs,
                TokenKind::Mvs,
                TokenKind::Nirugu,
                TokenKind::Zwj
            ]
        );
        assert_eq!(tokens[0].cp, MVS);
        assert!(tokens[2].is_nirugu() && tokens[3].is_nirugu());
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
