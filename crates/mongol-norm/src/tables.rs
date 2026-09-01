//! Hand-written types that the generated data tables in `src/generated/` are expressed in, plus
//! the small public enums shared between the tables and the API: [`Locale`], [`Position`],
//! [`UnitPosition`] and [`Fvs`].

use std::fmt;
use std::str::FromStr;

use crate::generated::enums::{Alias, Condition, WrittenUnit};
use crate::Error;

/// A script locale: which data tables and shaping rules a [`Shaper`](crate::Shaper) uses.
///
/// Only `Mng` (Hudum, Traditional Mongolian) has shaping rules and a normalize table; the other
/// three load their variant data and shape default/FVS forms only — exactly like the Python
/// implementation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    /// Hudum — Traditional Mongolian (`"MNG"`).
    Mng,
    /// Todo (`"TOD"`).
    Tod,
    /// Sibe (`"SIB"`).
    Sib,
    /// Manchu (`"MCH"`).
    Mch,
}

impl Locale {
    /// Every locale.
    pub const ALL: [Locale; 4] = [Locale::Mng, Locale::Tod, Locale::Sib, Locale::Mch];

    /// The contract name used by the Python API, the CLI and the data files.
    pub const fn as_str(self) -> &'static str {
        match self {
            Locale::Mng => "MNG",
            Locale::Tod => "TOD",
            Locale::Sib => "SIB",
            Locale::Mch => "MCH",
        }
    }
}

impl FromStr for Locale {
    type Err = Error;

    fn from_str(name: &str) -> Result<Locale, Error> {
        Locale::ALL
            .iter()
            .copied()
            .find(|locale| locale.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                kind: "locale",
                name: name.to_owned(),
            })
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Joining-topology position of a letter within its chain.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Position {
    /// Isolated (`"isol"`).
    Isol,
    /// Initial (`"init"`).
    Init,
    /// Medial (`"medi"`).
    Medi,
    /// Final (`"fina"`).
    Fina,
}

impl Position {
    /// Every position, in `isol, init, medi, fina` order.
    pub const ALL: [Position; 4] = [
        Position::Isol,
        Position::Init,
        Position::Medi,
        Position::Fina,
    ];

    /// The contract name (`"isol"`, `"init"`, `"medi"`, `"fina"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Position::Isol => "isol",
            Position::Init => "init",
            Position::Medi => "medi",
            Position::Fina => "fina",
        }
    }
}

impl FromStr for Position {
    type Err = Error;

    fn from_str(name: &str) -> Result<Position, Error> {
        Position::ALL
            .iter()
            .copied()
            .find(|position| position.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                kind: "position",
                name: name.to_owned(),
            })
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Position of a written unit in the authoritative HUD inventory, as accepted by
/// [`Shaper::normalize_positioned_written_units`](crate::Shaper::normalize_positioned_written_units).
///
/// This is *not* a Unicode letter's joining topology ([`Position`]): isolated FA borrows
/// `F:init`, and the structural units `Mvs` / `Nirugu` use `Control`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnitPosition {
    /// `"isol"`.
    Isol,
    /// `"init"`.
    Init,
    /// `"medi"`.
    Medi,
    /// `"fina"`.
    Fina,
    /// `"control"` — for `Mvs` and `Nirugu`.
    Control,
}

impl UnitPosition {
    /// Every unit position.
    pub const ALL: [UnitPosition; 5] = [
        UnitPosition::Isol,
        UnitPosition::Init,
        UnitPosition::Medi,
        UnitPosition::Fina,
        UnitPosition::Control,
    ];

    /// The contract name (`"isol"`, `"init"`, `"medi"`, `"fina"`, `"control"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            UnitPosition::Isol => "isol",
            UnitPosition::Init => "init",
            UnitPosition::Medi => "medi",
            UnitPosition::Fina => "fina",
            UnitPosition::Control => "control",
        }
    }

    /// The letter position this names, or `None` for `Control`.
    pub const fn as_position(self) -> Option<Position> {
        match self {
            UnitPosition::Isol => Some(Position::Isol),
            UnitPosition::Init => Some(Position::Init),
            UnitPosition::Medi => Some(Position::Medi),
            UnitPosition::Fina => Some(Position::Fina),
            UnitPosition::Control => None,
        }
    }
}

impl From<Position> for UnitPosition {
    fn from(position: Position) -> UnitPosition {
        match position {
            Position::Isol => UnitPosition::Isol,
            Position::Init => UnitPosition::Init,
            Position::Medi => UnitPosition::Medi,
            Position::Fina => UnitPosition::Fina,
        }
    }
}

impl FromStr for UnitPosition {
    type Err = Error;

    fn from_str(name: &str) -> Result<UnitPosition, Error> {
        UnitPosition::ALL
            .iter()
            .copied()
            .find(|position| position.as_str() == name)
            .ok_or_else(|| Error::UnknownName {
                kind: "unit position",
                name: name.to_owned(),
            })
    }
}

impl fmt::Display for UnitPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A free variation selector (U+180B FVS1, U+180C FVS2, U+180D FVS3, U+180F FVS4).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fvs {
    /// U+180B.
    Fvs1,
    /// U+180C.
    Fvs2,
    /// U+180D.
    Fvs3,
    /// U+180F.
    Fvs4,
}

impl Fvs {
    /// Every selector, FVS1 first.
    pub const ALL: [Fvs; 4] = [Fvs::Fvs1, Fvs::Fvs2, Fvs::Fvs3, Fvs::Fvs4];

    /// The selector's code point.
    pub const fn cp(self) -> u32 {
        match self {
            Fvs::Fvs1 => 0x180B,
            Fvs::Fvs2 => 0x180C,
            Fvs::Fvs3 => 0x180D,
            Fvs::Fvs4 => 0x180F,
        }
    }

    /// The selector as a `char`.
    pub const fn as_char(self) -> char {
        match self {
            Fvs::Fvs1 => '\u{180B}',
            Fvs::Fvs2 => '\u{180C}',
            Fvs::Fvs3 => '\u{180D}',
            Fvs::Fvs4 => '\u{180F}',
        }
    }

    /// `1` for FVS1 … `4` for FVS4 (the `fvs` integer of the data files).
    pub const fn index(self) -> u8 {
        match self {
            Fvs::Fvs1 => 1,
            Fvs::Fvs2 => 2,
            Fvs::Fvs3 => 3,
            Fvs::Fvs4 => 4,
        }
    }

    /// The selector for a code point, if it is one.
    pub const fn from_cp(cp: u32) -> Option<Fvs> {
        match cp {
            0x180B => Some(Fvs::Fvs1),
            0x180C => Some(Fvs::Fvs2),
            0x180D => Some(Fvs::Fvs3),
            0x180F => Some(Fvs::Fvs4),
            _ => None,
        }
    }

    /// The selector for a data-file `fvs` integer (`1..=4`).
    pub const fn from_index(index: u8) -> Option<Fvs> {
        match index {
            1 => Some(Fvs::Fvs1),
            2 => Some(Fvs::Fvs2),
            3 => Some(Fvs::Fvs3),
            4 => Some(Fvs::Fvs4),
            _ => None,
        }
    }
}

// ── Table types (crate-private; instantiated only by the generated statics) ─────────────────

/// One letter of a locale: its code point, alias and every shaping variant (JSON order).
pub(crate) struct Letter {
    pub cp: u32,
    pub alias: Alias,
    pub variants: &'static [Variant],
}

/// One `(position, fvs)` variant of a letter.
pub(crate) struct Variant {
    pub position: Position,
    pub fvs: Option<Fvs>,
    pub written: &'static [WrittenUnit],
    pub default: bool,
    pub conditions: &'static [Condition],
}

/// Phonological categories of a locale (aliases).
pub(crate) struct Categories {
    pub vowel: &'static [Alias],
    pub consonant: &'static [Alias],
    pub vowel_masculine: &'static [Alias],
    pub vowel_feminine: &'static [Alias],
    pub vowel_neuter: &'static [Alias],
}

/// One symbol of a particle-dictionary key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ParticleSym {
    /// The MVS marker (`mvs` in the data).
    Mvs,
    /// A letter alias.
    Alias(Alias),
    /// A letter without an alias in this locale — never appears in generated keys, so any
    /// segment containing one never matches (Python: an empty alias never matches either).
    Unknown,
}

/// One particle-dictionary entry: alias sequence → token indices that take `particle`.
pub(crate) struct Particle {
    pub key: &'static [ParticleSym],
    pub indices: &'static [usize],
}

/// Everything the shaper needs for one locale.
pub(crate) struct LocaleData {
    pub letters: &'static [Letter],
    pub categories: Categories,
    pub particles: &'static [Particle],
}

/// One normalize-table entry: `(position, written units) → (letter code point, FVS)`.
pub(crate) struct UnitEntry {
    pub position: Position,
    pub units: &'static [WrittenUnit],
    pub cp: u32,
    pub fvs: Option<Fvs>,
}

/// The normalize table of a locale (`MNG.normalize.json`).
pub(crate) struct NormalizeData {
    pub canonical_version: &'static str,
    pub unit_enc_max_len: usize,
    pub unit_table: &'static [UnitEntry],
    pub velar_fem: &'static [UnitEntry],
    pub velar_fem_units: &'static [WrittenUnit],
    /// `(masculine cp, feminine cp)` pairs; only the masculine side is consulted at runtime.
    pub masc_to_fem: &'static [(u32, u32)],
    pub positioned_units: &'static [(WrittenUnit, Position)],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_names_round_trip() {
        for locale in Locale::ALL {
            assert_eq!(locale.as_str().parse::<Locale>().unwrap(), locale);
        }
        assert!("MNGx".parse::<Locale>().is_err());
        assert!("mng".parse::<Locale>().is_err());
    }

    #[test]
    fn position_names_round_trip() {
        for position in Position::ALL {
            assert_eq!(position.as_str().parse::<Position>().unwrap(), position);
            assert_eq!(UnitPosition::from(position).as_position(), Some(position));
        }
        assert_eq!(
            "control".parse::<UnitPosition>().unwrap(),
            UnitPosition::Control
        );
        assert_eq!(UnitPosition::Control.as_position(), None);
        assert!("control".parse::<Position>().is_err());
        assert!("middle".parse::<UnitPosition>().is_err());
    }

    #[test]
    fn fvs_code_points() {
        for fvs in Fvs::ALL {
            assert_eq!(Fvs::from_cp(fvs.cp()), Some(fvs));
            assert_eq!(Fvs::from_index(fvs.index()), Some(fvs));
            assert_eq!(fvs.as_char() as u32, fvs.cp());
        }
        assert_eq!(Fvs::from_cp(0x180E), None);
        assert_eq!(Fvs::from_index(0), None);
    }
    #[test]
    fn generated_enums_round_trip() {
        for unit in WrittenUnit::ALL {
            assert_eq!(unit.as_str().parse::<WrittenUnit>().unwrap(), unit);
            assert_eq!(unit.to_string(), unit.as_str());
        }
        for condition in Condition::ALL {
            assert_eq!(condition.as_str().parse::<Condition>().unwrap(), condition);
        }
        for alias in Alias::ALL {
            assert_eq!(alias.as_str().parse::<Alias>().unwrap(), alias);
        }
        assert!(WrittenUnit::Mvs.is_structural());
        assert!(WrittenUnit::Nirugu.is_structural());
        assert!(WrittenUnit::Zwj.is_structural());
        assert!(!WrittenUnit::A.is_structural());
        assert_eq!(Condition::ChachlagOnsetGb.as_str(), "chachlag_onset_gb");
        assert_eq!(Alias::K2.as_str(), "k2");
        assert_eq!(Alias::Oe.as_str(), "oe");
        assert!("mvs".parse::<WrittenUnit>().is_err());
        assert!("MVS".parse::<WrittenUnit>().is_err());
    }

    #[test]
    fn generated_tables_have_the_expected_shape() {
        use crate::generated::{mng, mng_normalize};
        assert_eq!(mng::DATA.letters.len(), 35);
        assert_eq!(mng::DATA.particles.len(), 47);
        assert_eq!(
            mng::DATA.categories.vowel_masculine,
            &[Alias::A, Alias::O, Alias::U]
        );
        let variants: usize = mng::DATA.letters.iter().map(|l| l.variants.len()).sum();
        assert_eq!(variants, 216);
        assert_eq!(mng_normalize::DATA.canonical_version, "mng-canonical/1");
        assert_eq!(mng_normalize::DATA.unit_enc_max_len, 3);
        assert_eq!(mng_normalize::DATA.unit_table.len(), 151);
        assert_eq!(mng_normalize::DATA.velar_fem.len(), 15);
        assert_eq!(mng_normalize::DATA.positioned_units.len(), 95);
        assert_eq!(
            mng_normalize::DATA.masc_to_fem,
            &[(0x1820, 0x1821), (0x1823, 0x1825), (0x1824, 0x1826)]
        );
        // Every (cp, position) has exactly one default, in every locale.
        for data in [
            &crate::generated::mng::DATA,
            &crate::generated::tod::DATA,
            &crate::generated::sib::DATA,
            &crate::generated::mch::DATA,
        ] {
            for letter in data.letters {
                for position in Position::ALL {
                    let defaults = letter
                        .variants
                        .iter()
                        .filter(|v| v.position == position && v.default)
                        .count();
                    let any = letter.variants.iter().any(|v| v.position == position);
                    assert_eq!(
                        defaults,
                        usize::from(any),
                        "U+{:04X} {:?}",
                        letter.cp,
                        position
                    );
                }
            }
        }
    }
}
