//! The font backend (Cargo feature `font`): shaping with the bundled mongfontbuilder Hudum font
//! through harfrust, the Rust port of HarfBuzz, instead of the ported rules.
//!
//! See [`FontShaper`] for the contract. The glyph → written-unit table this reads is generated
//! from the font's glyph names (`src/generated/font_hudum.rs`, by
//! `python/scripts/gen_font_shaper.py`); the font itself is `fonts/hudum.otf`, pinned to an
//! upstream commit by `fonts/mongfontbuilder.json`.

use std::sync::OnceLock;

use harfrust::{
    shape, Buffer, ClusterLevel, Direction, Font, Language, Script, ShapeOptions, ShapePlan,
    ShaperFont,
};

use crate::generated::font_hudum::GLYPHS;
use crate::shaper::canonical_units;
use crate::unicode::check_word_chars;
use crate::{Error, Position, WrittenUnit};

/// The bundled shaping font: mongfontbuilder's Hudum font without its outlines (GSUB, GDEF,
/// cmap and the sfnt skeleton). `fonts/mongfontbuilder.json` names the commit it was built at.
static FONT_DATA: &[u8] = include_bytes!("../fonts/hudum.otf");

/// The parsed font, shared by every `FontShaper`: read-fonts caches the layout tables on it.
static FONT: OnceLock<Font> = OnceLock::new();

fn font() -> &'static Font {
    FONT.get_or_init(|| Font::new(FONT_DATA, 0).expect("fonts/hudum.otf is a valid font"))
}

/// Characters HarfBuzz skips when it forms a ligature (marks to the font): the four FVS, ZWJ
/// and ZWNJ. They never count as a letter of a ligature.
fn is_skipped(c: char) -> bool {
    matches!(
        c as u32,
        0x180B | 0x180C | 0x180D | 0x180F | 0x200D | 0x200C
    )
}

/// The BCP 47 language harfrust maps to the font's one language system, `MNG `.
fn language() -> Language {
    Language::new("mn").expect("a non-empty language tag")
}

/// What a glyph of the bundled font contributes to a shaped run (the generated table's entry type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A letter variant or a ligature: `units` is what it shows.
    Letters,
    /// An MVS or NNBSP, in any of its drawings (the space of a separator that separates
    /// nothing included — its second, silent glyph is `Silent`).
    Mvs,
    /// A nirugu.
    Nirugu,
    /// A ZWJ.
    Zwj,
    /// A ZWNJ.
    Zwnj,
    /// Ink or no ink, nothing the crate reports: an FVS (valid or not — the font draws an
    /// invalid one as a placeholder), the silent half of an idle MVS, a composer marker.
    Silent,
    /// A glyph no shaped word may contain: `.notdef` (a character the font has no glyph for),
    /// punctuation, the composer's building blocks.
    Unsupported,
}

/// One entry of the generated glyph table.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Glyph {
    pub(crate) kind: Kind,
    pub(crate) units: &'static [WrittenUnit],
    /// The letters (code points) the glyph draws: 2+ for a ligature.
    pub(crate) letters: u8,
    pub(crate) form: Option<Position>,
    pub(crate) joining: Option<Position>,
}

impl Glyph {
    const fn control(kind: Kind, units: &'static [WrittenUnit]) -> Glyph {
        Glyph {
            kind,
            units,
            letters: 0,
            form: None,
            joining: None,
        }
    }

    pub(crate) const fn letters(
        units: &'static [WrittenUnit],
        letters: u8,
        form: Position,
        joining: Position,
    ) -> Glyph {
        Glyph {
            kind: Kind::Letters,
            units,
            letters,
            form: Some(form),
            joining: Some(joining),
        }
    }

    pub(crate) const fn mvs() -> Glyph {
        Glyph::control(Kind::Mvs, &[WrittenUnit::Mvs])
    }

    pub(crate) const fn nirugu() -> Glyph {
        Glyph::control(Kind::Nirugu, &[WrittenUnit::Nirugu])
    }

    pub(crate) const fn zwj() -> Glyph {
        Glyph::control(Kind::Zwj, &[WrittenUnit::Zwj])
    }

    pub(crate) const fn zwnj() -> Glyph {
        Glyph::control(Kind::Zwnj, &[WrittenUnit::Zwnj])
    }

    pub(crate) const fn silent() -> Glyph {
        Glyph::control(Kind::Silent, &[])
    }

    pub(crate) const fn unsupported() -> Glyph {
        Glyph::control(Kind::Unsupported, &[])
    }
}

/// One glyph of a shaped run, as [`FontShaper::shape_glyphs`] reports it.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapedGlyph {
    /// The glyph id in `fonts/hudum.otf`; `fonts/hudum.glyphs.txt` names it.
    pub glyph: u16,
    /// The `char` index of the input character the glyph comes from — for a ligature, of its
    /// first letter.
    pub index: usize,
    /// The written units the glyph shows; empty for a silent glyph (an FVS, the silent half of
    /// an idle MVS, …).
    pub units: &'static [WrittenUnit],
    /// The position of the written form the glyph draws; `None` for a control.
    pub form: Option<Position>,
    /// The joining position the glyph serves. Differs from `form` where a position borrows
    /// another's form — an isolated `N` is drawn with `N:init`, so `form` is `Init` and
    /// `joining` is `Isol`.
    pub joining: Option<Position>,
    /// A ZWJ or ZWNJ the font's ligature formation skipped: it sits between two letters of the
    /// preceding ligature, HarfBuzz moved it after the ligature, and [`FontShaper::shape`]
    /// drops it, as the redundant-ZWJ pass drops any joiner between two joined letter units. A
    /// ZWNJ is inside a ligature only between two ZWJs (`k zwj zwnj zwj u` is the `KO`
    /// ligature): they join its two letters again, and it breaks nothing.
    pub skipped: bool,
}

/// Shapes Hudum with the bundled mongfontbuilder font through HarfBuzz rules — the font backend
/// of the crate (Cargo feature `font`), an alternative to the rule engine
/// [`Shaper`](crate::Shaper).
///
/// harfrust (the Rust port of HarfBuzz) runs the Hudum font that mongfontbuilder — the
/// reference implementation of UTN #57 — composes, with the OpenType rules the composer
/// generates. The crate reads the written units off the glyphs the font chose. The font is
/// bundled without its outlines (`fonts/hudum.otf`, 30 KB: GSUB, GDEF, cmap and the sfnt
/// skeleton), and the glyph → written-unit table is generated from its glyph names.
///
/// Compared with the rule engine:
/// * the rules are the font's, at the mongfontbuilder commit `fonts/mongfontbuilder.json`
///   pins — not a port. Where the two disagree, this is what the font draws;
/// * there is no `trace` or `shape_detailed`: the font's lookups are not the crate's rules.
///   The glyph-level view is [`FontShaper::shape_glyphs`];
/// * there is no `normalize`: the normalizer is built on the rule engine;
/// * it costs harfrust and its crates, Rust 1.85, and a few microseconds per word.
///
/// # Output contract
///
/// [`FontShaper::shape`] is the same kind of sequence as [`Shaper::shape`](crate::Shaper::shape):
/// `Mvs` and `Nirugu` verbatim, a `Zwj` or `Zwnj` only where it changes the ink, the duplicate
/// encodings unified — the post-passes are shared with the rule engine — so the two backends
/// can be compared unit for unit. How the glyphs are read:
/// * an FVS, valid or not, is never a unit. The font draws an invalid one as a visible
///   placeholder; the crate follows the rule engine and the GB/T 25914-2023 unit streams and
///   reports nothing;
/// * an MVS that separates nothing is drawn as a space plus a silent glyph: one `Mvs`. An
///   NNBSP is an `Mvs`, as in the rule engine;
/// * a ZWJ between two letters the font ligates comes out of HarfBuzz after the ligature, and
///   so does a ZWNJ between two such ZWJs. [`FontShaper::shape_raw`] keeps it there; `shape`
///   drops it (see [`ShapedGlyph::skipped`]).
///
/// # Errors
///
/// [`Error::NonMongolianChar`] on anything but Mongolian letters, FVS, MVS, NNBSP, nirugu, ZWJ
/// and ZWNJ — the rule engine's rule; [`Error::UnsupportedChar`] on a character of the block the
/// font has no glyph for: a letter of another writing system (Todo, Sibe, Manchu, Ali Gali),
/// an Ali Gali sign, an unassigned code point. The rule engine shapes those to nothing; the
/// font would draw its `.notdef` box, so this backend refuses them instead.
///
/// # Example
///
/// ```
/// use mongol_norm::{FontShaper, Locale, Shaper};
///
/// let font = FontShaper::new();
/// let rules = Shaper::new(Locale::Mng);
/// assert_eq!(font.shape_str("ᠰᠠᠢᠨ")?, "S+A+I+I+A");
/// assert_eq!(font.shape("ᠰᠠᠢᠨ")?, rules.shape("ᠰᠠᠢᠨ")?);
/// # Ok::<(), mongol_norm::Error>(())
/// ```
pub struct FontShaper {
    language: Language,
    plan: ShapePlan,
}

impl Default for FontShaper {
    fn default() -> FontShaper {
        FontShaper::new()
    }
}

impl FontShaper {
    /// A shaper over the bundled font.
    ///
    /// The font is parsed once per process and shared; a shaper holds its shape plan, so it is
    /// cheap to keep and shapes without further setup.
    pub fn new() -> FontShaper {
        let language = language();
        let plan = ShapePlan::new(
            font(),
            Direction::LeftToRight,
            Some(Script::MONGOLIAN),
            Some(&language),
            &[],
        );
        FontShaper { language, plan }
    }

    /// Shape `text` into its written-unit sequence, on the same contract as
    /// [`Shaper::shape`](crate::Shaper::shape): `Mvs` and `Nirugu` verbatim, a `Zwj` or `Zwnj`
    /// only where it changes the ink, the duplicate encodings unified.
    pub fn shape(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        let glyphs = self.run(text)?;
        let raw: Vec<WrittenUnit> = glyphs
            .iter()
            .filter(|glyph| !glyph.skipped)
            .flat_map(|glyph| glyph.units.iter().copied())
            .collect();
        Ok(canonical_units(&raw))
    }

    /// The written units of the glyphs the font chose, in glyph order, before the shared
    /// post-passes: every ZWJ and ZWNJ of the input (a ZWJ the font ligated across comes after
    /// the ligature) and every duplicate encoding is still there. The counterpart of
    /// [`Shaper::shape_raw`](crate::Shaper::shape_raw), and like it not part of the public
    /// contract — it is what the conformance suites compare against.
    #[doc(hidden)]
    pub fn shape_raw(&self, text: &str) -> Result<Vec<WrittenUnit>, Error> {
        let glyphs = self.run(text)?;
        Ok(glyphs
            .iter()
            .flat_map(|glyph| glyph.units.iter().copied())
            .collect())
    }

    /// [`FontShaper::shape`] joined with `+` (`S+A+I+I+A`), the CLI's output format.
    pub fn shape_str(&self, text: &str) -> Result<String, Error> {
        let units = self.shape(text)?;
        let mut out = String::new();
        for unit in units {
            if !out.is_empty() {
                out.push('+');
            }
            out.push_str(unit.as_str());
        }
        Ok(out)
    }

    /// Do `a` and `b` render the same glyph sequence?
    pub fn same_shape(&self, a: &str, b: &str) -> Result<bool, Error> {
        Ok(self.shape(a)? == self.shape(b)?)
    }

    /// The glyphs the font chose for `text`, in the order HarfBuzz returns them, each with its
    /// written units and positions — the debugging view of this backend.
    pub fn shape_glyphs(&self, text: &str) -> Result<Vec<ShapedGlyph>, Error> {
        self.run(text)
    }

    fn run(&self, text: &str) -> Result<Vec<ShapedGlyph>, Error> {
        check_word_chars(text)?;
        let chars: Vec<char> = text.chars().collect();

        let mut buffer = Buffer::new();
        // One cluster per character, never merged: the cluster of a glyph is the index of the
        // character it comes from, which is what tells a skipped ZWJ or ZWNJ from one after a
        // ligature.
        buffer.set_cluster_level(ClusterLevel::Characters);
        for (index, &ch) in chars.iter().enumerate() {
            buffer.push(ch as u32, index as u32);
        }
        buffer.set_direction(Direction::LeftToRight);
        buffer.set_script(Some(Script::MONGOLIAN));
        buffer.set_language(Some(self.language.clone()));
        // `ShaperFont` borrows the shared font and reuses its cached layout data; it holds a
        // non-`Sync` callback slot, so it is built per call rather than kept in the shaper.
        let shaper_font = ShaperFont::new(font());
        shape(
            &shaper_font,
            &mut buffer,
            ShapeOptions::new().plan(Some(&self.plan)),
        )
        .expect("the plan was built for this direction and script");

        let mut out = Vec::with_capacity(buffer.len());
        // The letters of the last letter glyph: a ZWJ or ZWNJ whose character lies strictly
        // inside them was skipped by the ligature that drew them.
        let mut span: Option<(usize, usize)> = None;
        for info in buffer.glyph_infos() {
            let id = info.glyph_id as usize;
            let index = info.cluster as usize;
            let glyph = GLYPHS.get(id).copied().unwrap_or(Glyph::unsupported());
            match glyph.kind {
                Kind::Unsupported => {
                    return Err(Error::UnsupportedChar {
                        ch: chars[index],
                        index,
                    })
                }
                Kind::Letters => span = Some((index, span_end(&chars, index, glyph.letters))),
                _ => {}
            }
            let skipped = matches!(glyph.kind, Kind::Zwj | Kind::Zwnj)
                && span.is_some_and(|(start, end)| start < index && index < end);
            out.push(ShapedGlyph {
                glyph: id as u16,
                index,
                units: glyph.units,
                form: glyph.form,
                joining: glyph.joining,
                skipped,
            });
        }
        Ok(out)
    }
}

/// The `char` index of the last of the `letters` characters a glyph starting at `start`
/// draws. FVS and joiners do not count: the font skips them when it forms a ligature.
fn span_end(chars: &[char], start: usize, letters: u8) -> usize {
    let mut remaining = letters.max(1);
    let mut end = start;
    for (offset, &ch) in chars[start..].iter().enumerate() {
        if is_skipped(ch) {
            continue;
        }
        remaining -= 1;
        end = start + offset;
        if remaining == 0 {
            break;
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::font_hudum::GLYPH_COUNT;

    #[test]
    fn the_table_covers_the_font() {
        assert_eq!(font().num_glyphs() as usize, GLYPH_COUNT);
        assert_eq!(GLYPHS.len(), GLYPH_COUNT);
    }

    #[test]
    fn every_letter_glyph_shows_at_least_one_unit() {
        for (id, glyph) in GLYPHS.iter().enumerate() {
            match glyph.kind {
                Kind::Letters => {
                    assert!(!glyph.units.is_empty(), "glyph {id} shows nothing");
                    assert!(glyph.letters >= 1, "glyph {id} draws no letter");
                }
                Kind::Mvs | Kind::Nirugu | Kind::Zwj | Kind::Zwnj => {
                    assert_eq!(glyph.units.len(), 1)
                }
                Kind::Silent | Kind::Unsupported => assert!(glyph.units.is_empty()),
            }
        }
    }

    #[test]
    fn shapers_are_shareable_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FontShaper>();
    }

    #[test]
    fn span_end_skips_fvs_and_joiners() {
        let chars: Vec<char> = "ᠪ\u{200D}\u{180B}ᠣᠯ".chars().collect();
        assert_eq!(span_end(&chars, 0, 1), 0);
        assert_eq!(span_end(&chars, 0, 2), 3);
        assert_eq!(span_end(&chars, 0, 3), 4);
        assert_eq!(span_end(&chars, 3, 1), 3);
        // Asking for more letters than there are stops at the last one.
        assert_eq!(span_end(&chars, 3, 5), 4);
    }
}
