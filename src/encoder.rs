//! The online normalize encoder (`mng-canonical/3`).
//!
//! The shape is read left to right. Every letter but the last one is *committed* — appended to
//! the output and never revised — so the encoding of a word's prefix is always a prefix of the
//! encoding of the word, apart from that last letter: prefix-stability holds by construction.
//!
//! A letter may be committed only if it renders its own written units whatever the encoder writes
//! next (and leaves the committed letters intact). Whether it does depends on what came before it,
//! summarised by a small [`Context`], and on the next unit; the answers were computed offline by
//! shaping probe texts (`examples/gen_normalize_table`) and are looked up in the generated tables.
//! A bare letter may also constrain the class of the letter after it (a *promise*: bare `n` is
//! `N` before a vowel), which the encoder then keeps.
//!
//! Commitment is lazy: a pending tail waits as long as one final letter can still end the word
//! with it, which keeps multi-unit letters (`a` = `A A`, `o` = `A O`, `ng` = `A G`, …) available.
//! When a new unit makes that impossible, a small search over the tail (usually one or two units)
//! commits the cheapest safe letters. A structural token commits the whole tail. See
//! `docs/internals.md`, "Normalization strategy".

use crate::generated::enums::WrittenUnit;
use crate::normalize::{structural_char, NormalizeTable, OptionGroup};
use crate::tables::{ContextTable, FinalValidity, Position, Robustness};

/// No previous letter / no parent.
pub(crate) const NONE: u16 = u16::MAX;
/// A particle-trie state that can no longer match any key.
pub(crate) const DEAD: u16 = 1023;

const TOKEN_NONE: u8 = 0;
const TOKEN_LETTER: u8 = 1;
const TOKEN_MVS: u8 = 2;
const TOKEN_NIRUGU: u8 = 3;
const TOKEN_ZWJ: u8 = 4;

const CLUSTER_BROKEN: u8 = 0;
const CLUSTER_INITIAL: u8 = 1;
const CLUSTER_INITIAL_MEDIAL: u8 = 2;

const HARMONY_NONE: u8 = 0;
const HARMONY_MASCULINE: u8 = 1;
pub(crate) const HARMONY_FEMININE: u8 = 2;

/// Bit widths of the context components, in key-packing order: previous letter, its FVS, its
/// position, its written units, the previous token, MVS since the previous letter, consonant
/// cluster, masculine marker, particle node, promise. Mirrors `COMPONENTS` in
/// `examples/gen_normalize_table/context.rs` and `context_components` in the JSON. (`harmony` is
/// not a component: it only orders equally good letters, which the tables do not encode.)
const COMPONENT_BITS: [u32; 10] = [6, 3, 3, 8, 3, 1, 1, 1, 10, 3];

/// The promise classes tried for a bare letter, in order (`0` = no promise).
const PROMISES: [u8; 6] = [0, 1, 2, 3, 4, 5];

fn is_vowel(cp: u32) -> bool {
    (0x1820..=0x1827).contains(&cp)
}

fn is_feminine(cp: u32) -> bool {
    matches!(cp, 0x1821 | 0x1825 | 0x1826 | 0x1827)
}

fn is_masculine(cp: u32) -> bool {
    matches!(cp, 0x1820 | 0x1823 | 0x1824)
}

fn position_code(position: Position) -> u64 {
    match position {
        Position::Isol => 1,
        Position::Init => 2,
        Position::Medi => 3,
        Position::Fina => 4,
    }
}

/// What the rules can see of the committed text when they shape the next letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Context {
    /// The last committed letter (candidate index), structural tokens skipped; [`NONE`] at the start.
    prev: u16,
    /// The last committed token: none, a letter, MVS, nirugu or ZWJ.
    prev_token: u8,
    /// An MVS came after `prev`.
    mvs_since_prev: bool,
    /// The consonants back to an initial consonant (III.2a cluster `marked`).
    cluster: u8,
    /// The nearest vowel back, within the MVS segment, is masculine and initial/medial (III.2f).
    masculine: bool,
    /// Particle-key prefix of the current MVS segment (III.3), or [`DEAD`].
    particle: u16,
    /// The class the next letter was promised to be (`0` = none).
    promise: u8,
    /// The last masculine or feminine vowel committed (suffixes follow the stem, so an MVS does
    /// not reset it): equally short spellings follow its harmony.
    harmony: u8,
}

impl Context {
    fn initial() -> Context {
        Context {
            prev: NONE,
            prev_token: TOKEN_NONE,
            mvs_since_prev: false,
            cluster: CLUSTER_BROKEN,
            masculine: false,
            particle: 0,
            promise: 0,
            harmony: HARMONY_NONE,
        }
    }

    /// Is the next letter the first of its chain, with no joiner before it?
    fn chain_first(&self) -> bool {
        matches!(self.prev_token, TOKEN_NONE | TOKEN_MVS)
    }

    /// Position of a letter committed with more letters of its chain to follow.
    fn commit_position(&self) -> Position {
        if self.chain_first() {
            Position::Init
        } else {
            Position::Medi
        }
    }

    /// Position of the last letter before the structural token `token`.
    fn close_position(&self, token: WrittenUnit) -> Position {
        let joined_right = matches!(token, WrittenUnit::Nirugu | WrittenUnit::Zwj);
        match (self.chain_first(), joined_right) {
            (true, false) => Position::Isol,
            (true, true) => Position::Init,
            (false, false) => Position::Fina,
            (false, true) => Position::Medi,
        }
    }

    /// Position of the word's last letter.
    fn final_position(&self) -> Position {
        if self.chain_first() {
            Position::Isol
        } else {
            Position::Fina
        }
    }

    fn apply_letter(&self, table: &NormalizeTable, id: u16, promise: u8) -> Context {
        let cand = &table.data.candidates[id as usize];
        let cp = cand.cp;
        let cluster = if is_vowel(cp) {
            CLUSTER_BROKEN
        } else {
            match cand.position {
                Position::Init => CLUSTER_INITIAL,
                Position::Medi if self.cluster != CLUSTER_BROKEN => CLUSTER_INITIAL_MEDIAL,
                _ => CLUSTER_BROKEN,
            }
        };
        let masculine = if is_feminine(cp) {
            false
        } else if is_masculine(cp) && matches!(cand.position, Position::Init | Position::Medi) {
            true
        } else {
            self.masculine
        };
        let particle = if cand.fvs.is_none() {
            table.particle_child(self.particle, cp)
        } else {
            DEAD
        };
        let harmony = if is_masculine(cp) {
            HARMONY_MASCULINE
        } else if is_feminine(cp) {
            HARMONY_FEMININE
        } else {
            self.harmony
        };
        Context {
            prev: id,
            prev_token: TOKEN_LETTER,
            mvs_since_prev: false,
            cluster,
            masculine,
            particle,
            promise,
            harmony,
        }
    }

    fn apply_token(&self, token: WrittenUnit) -> Context {
        let mut next = *self;
        next.promise = 0;
        match token {
            WrittenUnit::Mvs => {
                next.prev_token = TOKEN_MVS;
                next.mvs_since_prev = true;
                next.cluster = CLUSTER_BROKEN;
                next.masculine = false;
                next.particle = 1;
            }
            WrittenUnit::Nirugu => next.prev_token = TOKEN_NIRUGU,
            _ => next.prev_token = TOKEN_ZWJ,
        }
        next
    }

    fn component(&self, table: &NormalizeTable, index: usize) -> u64 {
        let prev = (self.prev != NONE).then(|| &table.data.candidates[self.prev as usize]);
        match index {
            0 => prev.map_or(0, |c| u64::from(c.cp - 0x1820 + 1)),
            1 => prev.map_or(0, |c| c.fvs.map_or(0, |fvs| u64::from(fvs.index()))),
            2 => prev.map_or(0, |c| position_code(c.position)),
            3 => prev.map_or(0, |c| u64::from(c.written_id)),
            4 => u64::from(self.prev_token),
            5 => u64::from(self.mvs_since_prev),
            6 => u64::from(self.cluster == CLUSTER_INITIAL_MEDIAL),
            7 => u64::from(self.masculine),
            8 => u64::from(self.particle),
            _ => u64::from(self.promise),
        }
    }

    /// The row key of this context under `projection` (a bit set over the components).
    fn key(&self, table: &NormalizeTable, projection: u16) -> u64 {
        let mut key = 0u64;
        for (index, bits) in COMPONENT_BITS.iter().enumerate() {
            if projection & (1 << index) != 0 {
                key = (key << bits) | self.component(table, index);
            }
        }
        key
    }

    fn lookup<V: Copy>(&self, table: &NormalizeTable, context_table: &ContextTable<V>) -> V {
        let key = self.key(table, context_table.projection);
        context_table
            .rows
            .binary_search_by_key(&key, |(k, _)| *k)
            .map_or(context_table.default, |index| context_table.rows[index].1)
    }
}

/// One committed letter of a plan: candidate, units covered, promise made.
#[derive(Clone, Copy)]
struct Step {
    id: u16,
    units: usize,
    promise: u8,
}

fn cost(table: &NormalizeTable, id: u16) -> usize {
    if table.data.candidates[id as usize].fvs.is_some() {
        2
    } else {
        1
    }
}

/// The cheapest way to commit letters from `pending` so that the rest can be one final letter
/// (or, with `token`, so that all of it is committed before the token). Pushes the committed
/// letters onto `steps` and returns the total cost (the final letter included), or `None`.
fn plan(
    table: &NormalizeTable,
    ctx: Context,
    pending: &[WrittenUnit],
    token: Option<WrittenUnit>,
    steps: &mut Vec<Step>,
) -> Option<usize> {
    if pending.is_empty() {
        return token.map(|_| 0);
    }
    let base = steps.len();
    let mut best: Option<usize> = None;
    if token.is_none() {
        if let Some(id) = table.final_choice(&ctx, pending) {
            best = Some(cost(table, id));
        }
    }
    for span in (1..=pending.len().min(3)).rev() {
        let is_last = span == pending.len();
        let closing = if is_last {
            match token {
                Some(token) => Some(token),
                None => continue, // the word's last letter stays pending
            }
        } else {
            None
        };
        let position = match closing {
            Some(token) => ctx.close_position(token),
            None => ctx.commit_position(),
        };
        let rest = &pending[span..];
        let next = closing.unwrap_or_else(|| rest[0]);
        let Some(group) = table.options(position, &pending[..span]) else {
            continue;
        };
        // A chain-final letter follows the final-letter preference (`n` after a vowel); nothing is
        // planned after it, so the order cannot change the length.
        let order = table.order(&ctx, group, closing.is_some() && table.after_vowel(&ctx));
        'options: for &index in order {
            let id = group.ids[index as usize];
            let cand = &table.data.candidates[id as usize];
            if !table.allows(ctx.promise, cand.cp) {
                continue;
            }
            let own = cost(table, id);
            if best.is_some_and(|b| own >= b) {
                break;
            }
            let promises: &[u8] = if cand.fvs.is_none() && closing.is_none() {
                &PROMISES
            } else {
                &PROMISES[..1]
            };
            for &promise in promises {
                if promise > 0 && !table.promise_feasible(next, promise) {
                    continue;
                }
                if !table.robust(&ctx, id, next, promise) {
                    continue;
                }
                let after = ctx.apply_letter(table, id, promise);
                let mark = steps.len();
                steps.push(Step {
                    id,
                    units: span,
                    promise,
                });
                if let Some(sub) = plan(table, after, rest, token, steps) {
                    let total = own + sub;
                    if best.is_none_or(|b| total < b) {
                        best = Some(total);
                        steps.drain(base..mark); // keep only this plan
                    } else {
                        steps.truncate(mark);
                    }
                    break 'options; // the cheapest safe option of this span
                }
                steps.truncate(mark);
            }
        }
    }
    best
}

fn push_letter(out: &mut String, table: &NormalizeTable, id: u16) {
    let cand = &table.data.candidates[id as usize];
    out.push(char::from_u32(cand.cp).expect("table code points are scalar values"));
    if let Some(fvs) = cand.fvs {
        out.push(fvs.as_char());
    }
}

/// Encode a shape (duplicate encodings already unified), or `None` when the tables have no
/// encoding for it.
pub(crate) fn encode(table: &NormalizeTable, shape: &[WrittenUnit]) -> Option<String> {
    let mut out = String::with_capacity(shape.len() * 6);
    let mut ctx = Context::initial();
    let mut covered = 0;
    let mut steps: Vec<Step> = Vec::new();
    for end in 1..=shape.len() {
        let unit = shape[end - 1];
        if unit.is_structural() {
            steps.clear();
            plan(table, ctx, &shape[covered..end - 1], Some(unit), &mut steps)?;
            for step in &steps {
                push_letter(&mut out, table, step.id);
                ctx = ctx.apply_letter(table, step.id, step.promise);
            }
            out.push(structural_char(unit).expect("structural token"));
            ctx = ctx.apply_token(unit);
            covered = end;
            continue;
        }
        let pending = &shape[covered..end];
        if table.final_choice(&ctx, pending).is_some() {
            continue; // one final letter can still end the word: wait
        }
        steps.clear();
        if plan(table, ctx, pending, None, &mut steps).is_some() {
            for step in &steps {
                push_letter(&mut out, table, step.id);
                ctx = ctx.apply_letter(table, step.id, step.promise);
                covered += step.units;
            }
        }
        // else: this prefix is no shape of anything; wait for more units
    }
    if covered < shape.len() {
        let id = table.final_choice(&ctx, &shape[covered..])?;
        push_letter(&mut out, table, id);
    }
    Some(out)
}

impl NormalizeTable {
    /// The preferred final letter for `pending` after `ctx`, if one ends the word correctly.
    fn final_choice(&self, ctx: &Context, pending: &[WrittenUnit]) -> Option<u16> {
        let position = ctx.final_position();
        let valid = match self.final_validity(position, pending)? {
            FinalValidity::Always(mask) => *mask,
            FinalValidity::Table(table) => ctx.lookup(self, table),
        };
        let group = self.options(position, pending)?;
        self.order(ctx, group, self.after_vowel(ctx))
            .iter()
            .find_map(|&index| {
                let id = group.ids[index as usize];
                (valid & (1 << index) != 0
                    && self.allows(ctx.promise, self.data.candidates[id as usize].cp))
                .then_some(id)
            })
    }

    /// The preference order of `group` after `ctx` (indices into `group.ids`), see
    /// [`crate::normalize::preference`].
    fn order<'g>(&self, ctx: &Context, group: &'g OptionGroup, after_vowel: bool) -> &'g [u8] {
        let feminine = usize::from(ctx.harmony == HARMONY_FEMININE);
        &group.orders[feminine + 2 * usize::from(after_vowel)]
    }

    /// Is the previous letter a vowel of the same chain?
    fn after_vowel(&self, ctx: &Context) -> bool {
        ctx.prev_token == TOKEN_LETTER && is_vowel(self.data.candidates[ctx.prev as usize].cp)
    }

    /// May candidate `id` be committed after `ctx` before `next`, making `promise`?
    fn robust(&self, ctx: &Context, id: u16, next: WrittenUnit, promise: u8) -> bool {
        match &self.data.candidates[id as usize].robust {
            Robustness::Always => promise == 0,
            Robustness::Never => false,
            Robustness::Table(table) => {
                let set = ctx.lookup(self, table);
                self.data.mask_sets[set as usize][promise as usize] & (1u128 << (next as u32)) != 0
            }
        }
    }
}
