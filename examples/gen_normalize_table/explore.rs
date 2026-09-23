//! Breadth-first exploration of the encoder's abstract states, with the shaping engine as the
//! oracle.
//!
//! For every reachable [`Context`] the generator keeps one concrete committed text that realizes
//! it and asks the engine, by shaping that text followed by probe continuations:
//!
//! * **right-robustness** of each context-dependent candidate: does it render its own written
//!   units — and do all committed letters still render theirs — whatever the encoder may write
//!   next? Answered per next unit and per promise the candidate may make.
//! * **final letters**: which option ends the word correctly here.
//!
//! Successor states come from committing any candidate that is robust for some continuation, and
//! from structural tokens.

use std::collections::{HashMap, HashSet};

use mongol_norm::{Position, Shaper, WrittenUnit};

use crate::candidates::Candidate;
use crate::context::{allows, Context, Trie, DEAD};

pub const MVS: char = '\u{180E}';
pub const NIRUGU: char = '\u{180A}';
pub const ZWJ: char = '\u{200D}';
pub const TOKENS: [WrittenUnit; 3] = [WrittenUnit::Mvs, WrittenUnit::Nirugu, WrittenUnit::Zwj];

fn token_char(token: WrittenUnit) -> char {
    match token {
        WrittenUnit::Mvs => MVS,
        WrittenUnit::Nirugu => NIRUGU,
        WrittenUnit::Zwj => ZWJ,
        other => unreachable!("{other:?}"),
    }
}

/// Index of a unit in `WrittenUnit::ALL` — the bit of a robustness mask.
pub fn unit_bit(unit: WrittenUnit) -> u32 {
    WrittenUnit::ALL
        .iter()
        .position(|u| *u == unit)
        .expect("a WrittenUnit") as u32
}

/// Can the letter after a promise of class `promise` render `unit` wherever that letter may
/// stand (medial if more follows, final otherwise)? Positions `unit` never occurs in do not count.
/// Mirrors `NormalizeTable` in `src/normalize.rs`.
pub fn promise_keepable(cands: &[Candidate], unit: WrittenUnit, promise: u8) -> bool {
    let renders = |position: Position, class: bool| {
        cands.iter().any(|c| {
            c.position == position && c.units == [unit] && (!class || allows(promise, c.cp))
        })
    };
    let occurs = [Position::Medi, Position::Fina]
        .iter()
        .filter(|&&p| renders(p, false))
        .count();
    occurs > 0
        && [Position::Medi, Position::Fina]
            .iter()
            .all(|&p| !renders(p, false) || renders(p, true))
}

/// A reachable state and one committed text that realizes it.
#[derive(Clone)]
pub struct Node {
    pub ctx: Context,
    /// Nothing pending: a structural token may come next directly.
    pub tail_empty: bool,
    pub text: String,
    /// `(token index, written units)` of every committed letter.
    pub letters: Vec<(usize, Vec<WrittenUnit>)>,
    /// Number of tokens (letters and structural characters) in `text`.
    pub tokens: usize,
    /// The shape the committed text stands for.
    pub public: Vec<WrittenUnit>,
    /// Units the pending tail may start with here: the runtime commits the previous letter only
    /// when it is robust for the actual next unit, so answers for other units are never asked.
    pub allowed_next: u128,
}

impl Node {
    fn initial() -> Node {
        Node {
            ctx: Context::initial(),
            tail_empty: true,
            text: String::new(),
            letters: Vec::new(),
            tokens: 0,
            public: Vec::new(),
            allowed_next: u128::MAX,
        }
    }

    fn with_letter(
        &self,
        id: u16,
        cand: &Candidate,
        promise: u8,
        allowed_next: u128,
        trie: &Trie,
    ) -> Node {
        let mut next = self.clone();
        next.allowed_next = allowed_next;
        next.ctx = self.ctx.apply_letter(id, cand, promise, trie);
        next.tail_empty = false;
        next.text.push_str(&cand.text);
        next.letters.push((self.tokens, cand.written.clone()));
        next.tokens += 1;
        next.public.extend_from_slice(&cand.units);
        next
    }

    fn with_token(&self, token: WrittenUnit, trie: &Trie) -> Node {
        let mut next = self.clone();
        next.ctx = self.ctx.apply_token(token, trie);
        next.tail_empty = true;
        next.allowed_next = u128::MAX;
        next.text.push(token_char(token));
        next.tokens += 1;
        next.public.push(token);
        next
    }
}

/// What the tables need from one state.
pub struct StateResult {
    pub ctx: Context,
    /// Union of `allowed_next` over every way the exploration reached this state.
    pub allowed_next: u128,
    /// Per context-dependent candidate usable here: robustness masks per promise over next units.
    pub robust: Vec<(u16, [u128; 6])>,
    /// Per final group index: which of its options (bit = index in the group) end the word
    /// correctly here. The runtime picks among them by preference.
    pub finals: Vec<(usize, u32)>,
    pub successors: Vec<Node>,
    pub probes: usize,
}

/// Static inputs of the exploration.
pub struct Setup<'a> {
    pub shaper: &'a Shaper,
    pub cands: &'a [Candidate],
    pub trie: &'a Trie,
    /// Next-letter probes: distinct candidate texts at medi/fina, grouped by their first unit.
    pub next_by_unit: HashMap<WrittenUnit, Vec<u16>>,
    /// Per promise: the next units before which the runtime may make it (`promise_keepable`).
    pub promise_units: [u128; 6],
    /// Final groups `(position, units)` → candidate ids in preference order.
    pub final_groups: Vec<(Position, Vec<WrittenUnit>, Vec<u16>)>,
    /// Particle completions: node → alias tails (as text) that complete a key from there.
    pub completions: HashMap<u16, Vec<String>>,
    /// What may follow a probed next letter (its own continuation).
    pub after_next: Vec<String>,
    /// What may follow a structural token, two tokens deep.
    pub after_token: Vec<String>,
    pub after_token2: Vec<String>,
}

impl<'a> Setup<'a> {
    pub fn new(
        shaper: &'a Shaper,
        cands: &'a [Candidate],
        trie: &'a Trie,
        particles: &[Vec<String>],
        alias_cp: &dyn Fn(&str) -> u32,
    ) -> Setup<'a> {
        let mut next_by_unit: HashMap<WrittenUnit, Vec<u16>> = HashMap::new();
        for (id, c) in cands.iter().enumerate() {
            if matches!(c.position, Position::Medi | Position::Fina) {
                let ids = next_by_unit.entry(c.units[0]).or_default();
                if !ids
                    .iter()
                    .any(|&other| cands[other as usize].text == c.text)
                {
                    ids.push(id as u16);
                }
            }
        }
        let mut promise_units = [0u128; 6];
        for unit in WrittenUnit::ALL {
            if unit.is_structural() {
                continue;
            }
            for (promise, bits) in promise_units.iter_mut().enumerate() {
                if promise == 0 || promise_keepable(cands, unit, promise as u8) {
                    *bits |= 1u128 << unit_bit(unit);
                }
            }
        }
        // One group per (position, units) at isol/fina: the candidates in table order, so a bit of a
        // validity mask is the index of the option in the runtime's option list.
        let mut final_groups: Vec<(Position, Vec<WrittenUnit>, Vec<u16>)> = Vec::new();
        for (id, c) in cands.iter().enumerate() {
            if !matches!(c.position, Position::Isol | Position::Fina) {
                continue;
            }
            match final_groups
                .iter_mut()
                .find(|(p, u, _)| *p == c.position && *u == c.units)
            {
                Some((_, _, ids)) => ids.push(id as u16),
                None => final_groups.push((c.position, c.units.clone(), vec![id as u16])),
            }
        }
        // Completions: from every node, the alias tails of the keys below it.
        let mut completions: HashMap<u16, Vec<String>> = HashMap::new();
        for (root, after_mvs) in [(0u16, false), (trie.root_after_mvs, true)] {
            for key in particles {
                let syms: Vec<&str> = key.iter().map(String::as_str).collect();
                let syms = match syms.first() {
                    Some(&"mvs") if after_mvs => &syms[1..],
                    Some(&"mvs") => continue,
                    _ => &syms[..],
                };
                let mut node = root;
                for j in 0..syms.len() {
                    let tail: String = syms[j..]
                        .iter()
                        .map(|a| char::from_u32(alias_cp(a)).expect("scalar"))
                        .collect();
                    completions.entry(node).or_default().push(tail);
                    node = trie.child(node, alias_cp(syms[j]));
                }
            }
        }
        for tails in completions.values_mut() {
            tails.sort();
            tails.dedup();
        }
        let l = "\u{182F}".to_string();
        let after_next = vec![
            String::new(),
            l.clone(),
            MVS.to_string(),
            NIRUGU.to_string(),
            ZWJ.to_string(),
        ];
        let mut after_token = vec![
            String::new(),
            MVS.to_string(),
            NIRUGU.to_string(),
            ZWJ.to_string(),
        ];
        for cp in 0x1820..=0x1842u32 {
            after_token.push(char::from_u32(cp).expect("scalar").to_string());
        }
        for s in [
            "\u{1820}\u{180B}",
            "\u{1821}\u{180B}",
            "\u{1822}\u{180B}",
            "\u{1823}\u{180B}",
            "\u{1828}\u{180B}",
            "\u{182D}\u{180C}",
        ] {
            after_token.push(s.to_string());
        }
        let after_token2 = vec![
            String::new(),
            l,
            "\u{1820}".to_string(),
            MVS.to_string(),
            NIRUGU.to_string(),
            "\u{1822}\u{182F}".to_string(),
        ];
        Setup {
            shaper,
            cands,
            trie,
            next_by_unit,
            promise_units,
            final_groups,
            completions,
            after_next,
            after_token,
            after_token2,
        }
    }

    /// Does `cand` at `position`, after the committed text of `node` and before `suffix`, render its
    /// own units — and every committed letter its own?
    fn probe_ok(
        &self,
        node: &Node,
        cand: &Candidate,
        position: Position,
        suffix: &str,
        probes: &mut usize,
    ) -> bool {
        *probes += 1;
        let text = format!("{}{}{}", node.text, cand.text, suffix);
        let Ok(details) = self.shaper.shape_detailed(&text) else {
            return false;
        };
        let Some(own) = details.get(node.tokens) else {
            return false;
        };
        own.written == cand.written
            && own.position == position
            && node
                .letters
                .iter()
                .all(|(index, written)| details[*index].written == *written)
    }

    /// Completions of the particle segment after committing `cand` (empty when no key can match).
    fn tails_after(&self, node: &Node, cand: &Candidate) -> &[String] {
        if node.ctx.particle == DEAD || !cand.bare() {
            return &[];
        }
        let child = self.trie.child(node.ctx.particle, cand.cp);
        if child == DEAD {
            return &[];
        }
        self.completions.get(&child).map_or(&[], Vec::as_slice)
    }

    /// Bare `g`/`h` right after an `i` can fall into the masculine-marker walk of III.2f, which
    /// looks arbitrarily far ahead: never committed.
    fn blocked(&self, node: &Node, cand: &Candidate) -> bool {
        cand.bare()
            && matches!(cand.cp, 0x182C | 0x182D)
            && !node.ctx.mvs_since_prev
            && node
                .ctx
                .prev
                .is_some_and(|p| self.cands[p as usize].cp == 0x1822)
    }

    /// Masks for a context-dependent candidate committed at the state's commit position.
    fn commit_masks(
        &self,
        node: &Node,
        cand: &Candidate,
        masks: &mut [u128; 6],
        probes: &mut usize,
    ) {
        let position = node.ctx.commit_position();
        if self.blocked(node, cand) {
            return;
        }
        for tail in self.tails_after(node, cand) {
            for end in ["", "\u{180E}", "\u{182F}"] {
                if !self.probe_ok(node, cand, position, &format!("{tail}{end}"), probes) {
                    return;
                }
            }
        }
        for (unit, next_ids) in &self.next_by_unit {
            let mut ok_by_next: Vec<(u16, bool)> = Vec::with_capacity(next_ids.len());
            for &next in next_ids {
                let x1 = &self.cands[next as usize];
                let ok = self.after_next.iter().all(|x2| {
                    self.probe_ok(node, cand, position, &format!("{}{x2}", x1.text), probes)
                });
                ok_by_next.push((next, ok));
            }
            let bit = 1u128 << unit_bit(*unit);
            for (promise, mask) in masks.iter_mut().enumerate() {
                if promise > 0 && !cand.bare() {
                    continue;
                }
                let mut any = false;
                let mut all = true;
                for (next, ok) in &ok_by_next {
                    if allows(promise as u8, self.cands[*next as usize].cp) {
                        any = true;
                        all &= *ok;
                    }
                }
                if any && all {
                    *mask |= bit;
                }
            }
        }
    }

    /// Mask bit for a context-dependent candidate that ends its chain before `token`.
    fn close_ok(
        &self,
        node: &Node,
        cand: &Candidate,
        token: WrittenUnit,
        probes: &mut usize,
    ) -> bool {
        let position = node.ctx.close_position(token);
        if self.blocked(node, cand) {
            return false;
        }
        let t = token_char(token);
        if !self.chain_end_ok(node, cand, token, probes) {
            return false;
        }
        if token != WrittenUnit::Mvs {
            for tail in self.tails_after(node, cand) {
                for end in ["", "\u{180E}", "\u{182F}"] {
                    if !self.probe_ok(node, cand, position, &format!("{t}{tail}{end}"), probes) {
                        return false;
                    }
                }
            }
        }
        for x2 in &self.after_token {
            for x3 in &self.after_token2 {
                if !self.probe_ok(node, cand, position, &format!("{t}{x2}{x3}"), probes) {
                    return false;
                }
            }
        }
        true
    }

    /// The chain ends with `cand` before `token`: its public units must survive duplicate
    /// unification (a chain `A Aa` contracts to `A`, so committing it serves no target shape).
    fn chain_end_ok(
        &self,
        node: &Node,
        cand: &Candidate,
        token: WrittenUnit,
        probes: &mut usize,
    ) -> bool {
        *probes += 1;
        let mut want = node.public.clone();
        want.extend_from_slice(&cand.units);
        want.push(token);
        let text = format!("{}{}{}", node.text, cand.text, token_char(token));
        self.shaper.shape(&text).is_ok_and(|shape| shape == want)
    }

    /// Is `cand` a correct last letter of the word here?
    fn final_ok(&self, node: &Node, cand: &Candidate, probes: &mut usize) -> bool {
        let position = node.ctx.final_position();
        if !self.probe_ok(node, cand, position, "", probes) {
            return false;
        }
        *probes += 1;
        let text = format!("{}{}", node.text, cand.text);
        let mut want = node.public.clone();
        want.extend_from_slice(&cand.units);
        self.shaper.shape(&text).is_ok_and(|shape| shape == want)
    }

    pub fn process(&self, node: &Node) -> StateResult {
        let ctx = node.ctx;
        let mut probes = 0usize;
        let mut robust: Vec<(u16, [u128; 6])> = Vec::new();
        let mut successors: Vec<Node> = Vec::new();
        let commit_position = ctx.commit_position();
        // The previous letter was committed because it is robust for the units in
        // `allowed_next`; a next letter starting with any other unit cannot follow it.
        let allowed =
            |units: &[WrittenUnit]| node.allowed_next & (1u128 << unit_bit(units[0])) != 0;
        for (id, cand) in self.cands.iter().enumerate() {
            let id = id as u16;
            if !allows(ctx.promise, cand.cp) || !allowed(&cand.units) {
                continue;
            }
            let at_commit = cand.position == commit_position;
            let closes: Vec<WrittenUnit> = TOKENS
                .iter()
                .copied()
                .filter(|t| ctx.close_position(*t) == cand.position)
                .collect();
            if !at_commit && closes.is_empty() {
                continue;
            }
            let masks = if cand.context_free {
                None
            } else {
                let mut masks = [0u128; 6];
                if at_commit {
                    self.commit_masks(node, cand, &mut masks, &mut probes);
                }
                for &token in &closes {
                    if self.close_ok(node, cand, token, &mut probes) {
                        masks[0] |= 1u128 << unit_bit(token);
                    }
                }
                robust.push((id, masks));
                Some(masks)
            };
            // successors: commits at this position (with every promise it can make) …
            if at_commit {
                match masks {
                    None => successors.push(node.with_letter(id, cand, 0, u128::MAX, self.trie)),
                    Some(masks) => {
                        for (promise, mask) in masks.iter().enumerate() {
                            let usable = mask & self.promise_units[promise];
                            if usable != 0 {
                                successors.push(node.with_letter(
                                    id,
                                    cand,
                                    promise as u8,
                                    usable,
                                    self.trie,
                                ));
                            }
                        }
                    }
                }
            }
            // … and as the last letter before a structural token.
            for &token in &closes {
                let ok = match masks {
                    Some(m) => m[0] & (1u128 << unit_bit(token)) != 0,
                    None => self.chain_end_ok(node, cand, token, &mut probes),
                };
                if ok {
                    successors.push(
                        node.with_letter(id, cand, 0, u128::MAX, self.trie)
                            .with_token(token, self.trie),
                    );
                }
            }
        }
        if node.tail_empty {
            for token in TOKENS {
                successors.push(node.with_token(token, self.trie));
            }
        }
        // final letters
        let final_position = ctx.final_position();
        let mut finals: Vec<(usize, u32)> = Vec::new();
        for (group, (position, units, ids)) in self.final_groups.iter().enumerate() {
            if *position != final_position || !allowed(units) {
                continue;
            }
            let mut valid = 0u32;
            for (bit, &id) in ids.iter().enumerate() {
                let cand = &self.cands[id as usize];
                if self.final_ok(node, cand, &mut probes) {
                    valid |= 1 << bit;
                }
            }
            finals.push((group, valid));
        }
        StateResult {
            ctx,
            allowed_next: node.allowed_next,
            robust,
            finals,
            successors,
            probes,
        }
    }
}

/// Breadth-first exploration from the start state; returns one result per distinct
/// `(context, tail_empty, allowed_next)`, in discovery order.
pub fn explore(setup: &Setup, threads: usize, verbose: bool) -> Vec<StateResult> {
    let start = std::time::Instant::now();
    // A state is identified by its context *and* what may follow it, so that every committed
    // letter of a representative text is robust for what the text puts after it.
    let mut seen: HashSet<(Context, bool, u128)> = HashSet::new();
    let initial = Node::initial();
    seen.insert((initial.ctx, initial.tail_empty, initial.allowed_next));
    let mut frontier = vec![initial];
    let mut out: Vec<StateResult> = Vec::new();
    let mut level = 0;
    while !frontier.is_empty() {
        let results = parallel_map(&frontier, threads, |node| setup.process(node));
        let mut next: Vec<Node> = Vec::new();
        for result in &results {
            for successor in &result.successors {
                if seen.insert((successor.ctx, successor.tail_empty, successor.allowed_next)) {
                    next.push(successor.clone());
                }
            }
        }
        if verbose {
            let probes: usize = results.iter().map(|r| r.probes).sum();
            eprintln!(
                "level {level}: {} states, {probes} probes, {} new, {:.1}s",
                frontier.len(),
                next.len(),
                start.elapsed().as_secs_f64()
            );
        }
        out.extend(results.into_iter().map(|mut r| {
            r.successors.clear();
            r
        }));
        frontier = next;
        level += 1;
    }
    out
}

fn parallel_map<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    if threads <= 1 || items.len() < 2 {
        return items.iter().map(&f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<R>>> =
        items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.min(items.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let r = f(&items[i]);
                *slots[i].lock().expect("unpoisoned") = Some(r);
            });
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.into_inner().expect("unpoisoned").expect("computed"))
        .collect()
}
