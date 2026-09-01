//! Declarative shaping rules — one function per mongfontbuilder OpenType Lookup, applied in the
//! declared order (the rule bodies are added in Task 6).
#![allow(dead_code)] // TEMPORARY — removed in Task 6 when the rules land.

use crate::shaper::Shaper;
use crate::tables::Locale;
use crate::token::Token;

/// A single shaping rule = a single mongfontbuilder Lookup.
pub(crate) struct Rule {
    /// The Lookup name, e.g. `III.2f.h_g.harmony` (frozen by the phase-trace golden).
    pub name: &'static str,
    /// Mutates the tokens' `condition` fields in place.
    pub apply: fn(&mut [Token], &Shaper),
}

/// The rule table of a locale (only MNG has rules, exactly as in Python).
pub(crate) fn rules_for(locale: Locale) -> &'static [Rule] {
    match locale {
        Locale::Mng => RULES_MNG,
        Locale::Tod | Locale::Sib | Locale::Mch => &[],
    }
}

/// Execute the rules in declared order.
pub(crate) fn run_rules(rules: &[Rule], tokens: &mut [Token], shaper: &Shaper) {
    for rule in rules {
        (rule.apply)(tokens, shaper);
    }
}

/// Hudum rule table, in mongfontbuilder `iii.py` order.
pub(crate) static RULES_MNG: &[Rule] = &[];
