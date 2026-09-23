//! Encoding options: every `(letter, FVS)` with the written units it renders at a position.

use mongol_norm::{Position, WrittenUnit};

use crate::data::Rules;

/// One encoding option.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub cp: u32,
    /// `0` bare, `1..=4` FVS1–FVS4.
    pub fvs: u8,
    pub position: Position,
    /// The public (duplicate-unified) units it contributes to a shape.
    pub units: Vec<WrittenUnit>,
    /// The engine's own units for the token (`shape_detailed`).
    pub written: Vec<WrittenUnit>,
    /// Code points: 1 bare, 2 with an FVS.
    pub cost: u8,
    /// Renders `written` in every context (a named FVS variant, or a letter whose variants at this
    /// position carry no condition).
    pub context_free: bool,
    /// The Unicode text of the option.
    pub text: String,
}

impl Candidate {
    pub fn bare(&self) -> bool {
        self.fvs == 0
    }
}

pub fn fvs_char(fvs: u8) -> char {
    match fvs {
        1 => '\u{180B}',
        2 => '\u{180C}',
        3 => '\u{180D}',
        4 => '\u{180F}',
        _ => unreachable!("FVS index {fvs}"),
    }
}

/// Spelling rule kept from `mng-canonical/2`: an isolated `I` is written `i`, never `j` (both
/// render the lone double tooth).
fn is_excluded(cp: u32, fvs: u8, written: &[WrittenUnit], position: Position) -> bool {
    cp == 0x1835 && fvs == 0 && position == Position::Isol && written == [WrittenUnit::I]
}

/// Renderings that `shape` unifies with a unit pair (`src/duplicates.rs`): never emitted, so
/// every candidate's public units equal its own units.
fn is_duplicate(written: &[WrittenUnit], position: Position) -> bool {
    written.len() == 1
        && matches!(
            (written[0], position),
            (WrittenUnit::Dd, _)
                | (WrittenUnit::H, Position::Medi)
                | (WrittenUnit::Hx, Position::Medi)
                | (WrittenUnit::Cr, Position::Init)
        )
}

/// Equal-cost tie-break: code point order, except `g` before `h` (ᠭᠡᠷ, not ᠬᠡᠷ).
pub fn rank(cp: u32) -> u32 {
    if cp == 0x182D {
        0x182C * 2 - 1
    } else {
        cp * 2
    }
}

fn position_order(position: Position) -> u8 {
    match position {
        Position::Isol => 0,
        Position::Init => 1,
        Position::Medi => 2,
        Position::Fina => 3,
    }
}

/// Every encoding option, grouped by `(position, units)` and in preference order within a group
/// (`cost`, then `rank`, then the FVS).
pub fn build(rules: &Rules) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut push = |cp: u32, fvs: u8, position: Position, written: &[WrittenUnit], cond: bool| {
        if written.is_empty()
            || is_duplicate(written, position)
            || is_excluded(cp, fvs, written, position)
        {
            return;
        }
        if out
            .iter()
            .any(|c| c.cp == cp && c.fvs == fvs && c.position == position && c.written == written)
        {
            return;
        }
        let ch = char::from_u32(cp).expect("scalar");
        let text = if fvs == 0 {
            ch.to_string()
        } else {
            format!("{ch}{}", fvs_char(fvs))
        };
        out.push(Candidate {
            cp,
            fvs,
            position,
            units: written.to_vec(),
            written: written.to_vec(),
            cost: if fvs == 0 { 1 } else { 2 },
            context_free: !cond,
            text,
        });
    };
    for letter in &rules.letters {
        for position in Position::ALL {
            let at: Vec<_> = letter
                .variants
                .iter()
                .filter(|v| v.position == position)
                .collect();
            if at.is_empty() {
                continue;
            }
            // A bare letter can render the default or any condition variant; whether it depends on
            // the context is decided by the conditions at this position.
            let conditional = at.iter().any(|v| v.conditional);
            for v in &at {
                if v.fvs > 0 {
                    // FVS resolution comes first in the engine: always this variant.
                    push(letter.cp, v.fvs, position, &v.written, false);
                }
                if v.default || v.conditional {
                    push(letter.cp, 0, position, &v.written, conditional);
                }
            }
            // An FVS that names no variant still blocks the FVS-sensitive rules; the letter
            // renders its default unless an FVS-insensitive rule picks a condition variant. Which
            // unnamed FVS does not matter (the only rules that read the value, III.2a GB.B and
            // III.5 GB, concern g/h forms that are all named, or a bowed final g, whose only
            // unnamed FVS is FVS4), so only the lowest is offered.
            if let Some(default) = at.iter().find(|v| v.default) {
                if let Some(fvs) = (1..=4u8).find(|fvs| !at.iter().any(|v| v.fvs == *fvs)) {
                    push(letter.cp, fvs, position, &default.written, conditional);
                }
            }
        }
    }
    out.sort_by(|a, b| {
        (
            position_order(a.position),
            unit_names(&a.units),
            a.cost,
            rank(a.cp),
            a.fvs,
        )
            .cmp(&(
                position_order(b.position),
                unit_names(&b.units),
                b.cost,
                rank(b.cp),
                b.fvs,
            ))
    });
    out
}

pub fn unit_names(units: &[WrittenUnit]) -> String {
    units
        .iter()
        .map(|u| u.as_str())
        .collect::<Vec<_>>()
        .join("+")
}
