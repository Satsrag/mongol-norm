//! The shaping data the generator needs, read from `python/mongol_norm/data/MNG.json` (the same
//! JSON the engine's tables are compiled from).

use std::path::Path;

use mongol_norm::{Position, WrittenUnit};

use crate::json::Json;

/// One `(position, FVS)` variant of a letter.
pub struct Variant {
    pub position: Position,
    /// `0` for the bare letter, `1..=4` for FVS1–FVS4.
    pub fvs: u8,
    pub written: Vec<WrittenUnit>,
    pub default: bool,
    /// Whether a rule-assigned condition can select this variant.
    pub conditional: bool,
    /// The HUD `(unit, position)` records of this variant.
    pub positioned: Vec<(String, String)>,
}

/// One letter of the locale.
pub struct Letter {
    pub cp: u32,
    pub alias: String,
    pub variants: Vec<Variant>,
}

/// Everything read from the rules JSON.
pub struct Rules {
    pub letters: Vec<Letter>,
    /// Particle dictionary keys as alias sequences (`mvs` included where the key has it).
    pub particles: Vec<Vec<String>>,
}

impl Rules {
    pub fn load(path: &Path) -> Rules {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let doc = Json::parse(&text);
        let letters = doc
            .index("letters")
            .as_array()
            .iter()
            .map(|letter| Letter {
                cp: letter.index("cp").as_u64() as u32,
                alias: letter.index("alias").as_str().to_owned(),
                variants: letter
                    .index("variants")
                    .as_array()
                    .iter()
                    .map(|variant| Variant {
                        position: variant
                            .index("position")
                            .as_str()
                            .parse()
                            .expect("known position"),
                        fvs: variant.index("fvs").as_u64() as u8,
                        written: variant
                            .index("written")
                            .strings()
                            .iter()
                            .map(|unit| unit.parse().expect("known written unit"))
                            .collect(),
                        default: matches!(variant.index("default"), Json::Bool(true)),
                        conditional: !variant.index("conditions").as_array().is_empty(),
                        positioned: variant
                            .get("positioned_written")
                            .map(|records| {
                                records
                                    .as_array()
                                    .iter()
                                    .map(|record| {
                                        (
                                            record.index("unit").as_str().to_owned(),
                                            record.index("position").as_str().to_owned(),
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect();
        let particles = match doc.index("particles") {
            Json::Object(fields) => fields
                .iter()
                .map(|(key, _)| key.split(' ').map(str::to_owned).collect())
                .collect(),
            other => panic!("particles is not an object: {other:?}"),
        };
        Rules { letters, particles }
    }

    pub fn alias(&self, cp: u32) -> &str {
        self.letters
            .iter()
            .find(|letter| letter.cp == cp)
            .map_or("?", |letter| letter.alias.as_str())
    }
}
