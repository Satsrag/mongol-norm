//! The encoder's abstract left context — the generator's copy of `src/encoder.rs::Context`. The
//! two must agree exactly: the tables are keyed by what this module computes.

use std::collections::HashMap;

use mongol_norm::{Position, WrittenUnit};

use crate::candidates::Candidate;

pub const TOKEN_NONE: u8 = 0;
pub const TOKEN_LETTER: u8 = 1;
pub const TOKEN_MVS: u8 = 2;
pub const TOKEN_NIRUGU: u8 = 3;
pub const TOKEN_ZWJ: u8 = 4;

pub const CLUSTER_BROKEN: u8 = 0;
pub const CLUSTER_INITIAL: u8 = 1;
pub const CLUSTER_INITIAL_MEDIAL: u8 = 2;

pub const DEAD: u16 = 1023;

/// `(name, bits)` of every key component, in packing order.
pub const COMPONENTS: [(&str, u32); 10] = [
    ("prev_letter", 6),
    ("prev_fvs", 3),
    ("prev_position", 3),
    ("prev_written", 8),
    ("prev_token", 3),
    ("mvs_since_prev", 1),
    ("cluster", 1),
    ("masculine", 1),
    ("particle", 10),
    ("promise", 3),
];

/// Promise names, index 1..=5 (0 = no promise).
pub const PROMISES: [&str; 5] = [
    "vowel",
    "vowel_not_ee",
    "masculine",
    "feminine_or_neutral",
    "consonant_or_ee",
];

pub fn is_vowel(cp: u32) -> bool {
    (0x1820..=0x1827).contains(&cp)
}

pub fn is_feminine(cp: u32) -> bool {
    matches!(cp, 0x1821 | 0x1825 | 0x1826 | 0x1827)
}

pub fn is_masculine(cp: u32) -> bool {
    matches!(cp, 0x1820 | 0x1823 | 0x1824)
}

/// Does the letter `cp` satisfy promise `promise` (0 = no promise)?
pub fn allows(promise: u8, cp: u32) -> bool {
    match promise {
        0 => true,
        1 => is_vowel(cp),
        2 => is_vowel(cp) && cp != 0x1827,
        3 => is_masculine(cp),
        4 => matches!(cp, 0x1821 | 0x1822 | 0x1825 | 0x1826 | 0x1827),
        5 => !is_vowel(cp) || cp == 0x1827,
        _ => false,
    }
}

/// The particle-key trie: node 0 is the word start, node `root_after_mvs` the segment after an MVS.
pub struct Trie {
    pub nodes: Vec<String>,
    pub root_after_mvs: u16,
    children: HashMap<(u16, u32), u16>,
}

impl Trie {
    /// Nodes are every prefix of every key a segment can match: at the word start only keys
    /// without `mvs`; after an MVS the keys with `mvs` (stripped) and the ones without.
    pub fn new(particles: &[Vec<String>], alias_cp: impl Fn(&str) -> u32) -> Trie {
        let mut nodes: Vec<String> = vec![String::new(), String::from("M:")];
        let mut children = HashMap::new();
        for (root, after_mvs) in [(0u16, false), (1u16, true)] {
            for key in particles {
                let syms: Vec<&str> = key.iter().map(String::as_str).collect();
                let syms = match syms.first() {
                    Some(&"mvs") if after_mvs => &syms[1..],
                    Some(&"mvs") => continue,
                    _ => &syms[..],
                };
                let mut parent = root;
                for j in 0..syms.len() {
                    let name = format!("{}{}", &nodes[root as usize], syms[..=j].join(" "));
                    let id = match nodes.iter().position(|n| *n == name) {
                        Some(id) => id as u16,
                        None => {
                            nodes.push(name);
                            (nodes.len() - 1) as u16
                        }
                    };
                    children.insert((parent, alias_cp(syms[j])), id);
                    parent = id;
                }
            }
        }
        assert!(nodes.len() < DEAD as usize, "particle trie too large");
        Trie {
            nodes,
            root_after_mvs: 1,
            children,
        }
    }

    pub fn child(&self, node: u16, cp: u32) -> u16 {
        if node == DEAD {
            return DEAD;
        }
        self.children.get(&(node, cp)).copied().unwrap_or(DEAD)
    }
}

/// The abstract left context of the next letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Context {
    pub prev: Option<u16>,
    pub prev_token: u8,
    pub mvs_since_prev: bool,
    pub cluster: u8,
    pub masculine: bool,
    pub particle: u16,
    pub promise: u8,
}

impl Context {
    pub fn initial() -> Context {
        Context {
            prev: None,
            prev_token: TOKEN_NONE,
            mvs_since_prev: false,
            cluster: CLUSTER_BROKEN,
            masculine: false,
            particle: 0,
            promise: 0,
        }
    }

    /// Is the next letter the first of its chain with no joiner before it?
    pub fn chain_first(&self) -> bool {
        matches!(self.prev_token, TOKEN_NONE | TOKEN_MVS)
    }

    /// Position of a letter committed with more letters of its chain to follow.
    pub fn commit_position(&self) -> Position {
        if self.chain_first() {
            Position::Init
        } else {
            Position::Medi
        }
    }

    /// Position of the last letter before the structural token `token`.
    pub fn close_position(&self, token: WrittenUnit) -> Position {
        let joined_right = matches!(token, WrittenUnit::Nirugu | WrittenUnit::Zwj);
        match (self.chain_first(), joined_right) {
            (true, false) => Position::Isol,
            (true, true) => Position::Init,
            (false, false) => Position::Fina,
            (false, true) => Position::Medi,
        }
    }

    /// Position of the word's last letter.
    pub fn final_position(&self) -> Position {
        if self.chain_first() {
            Position::Isol
        } else {
            Position::Fina
        }
    }

    pub fn apply_letter(&self, id: u16, cand: &Candidate, promise: u8, trie: &Trie) -> Context {
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
        let particle = if cand.bare() {
            trie.child(self.particle, cp)
        } else {
            DEAD
        };
        Context {
            prev: Some(id),
            prev_token: TOKEN_LETTER,
            mvs_since_prev: false,
            cluster,
            masculine,
            particle,
            promise,
        }
    }

    pub fn apply_token(&self, token: WrittenUnit, trie: &Trie) -> Context {
        let mut next = *self;
        next.promise = 0;
        match token {
            WrittenUnit::Mvs => {
                next.prev_token = TOKEN_MVS;
                next.mvs_since_prev = true;
                next.cluster = CLUSTER_BROKEN;
                next.masculine = false;
                next.particle = trie.root_after_mvs;
            }
            WrittenUnit::Nirugu => next.prev_token = TOKEN_NIRUGU,
            WrittenUnit::Zwj => next.prev_token = TOKEN_ZWJ,
            other => unreachable!("not a structural token: {other:?}"),
        }
        next
    }

    /// Component `index` of the key (see `COMPONENTS`).
    pub fn component(
        &self,
        index: usize,
        cands: &[Candidate],
        written_ids: &HashMap<Vec<WrittenUnit>, u64>,
    ) -> u64 {
        let prev = self.prev.map(|id| &cands[id as usize]);
        match index {
            0 => prev.map_or(0, |c| u64::from(c.cp - 0x1820 + 1)),
            1 => prev.map_or(0, |c| u64::from(c.fvs)),
            2 => prev.map_or(0, |c| match c.position {
                Position::Isol => 1,
                Position::Init => 2,
                Position::Medi => 3,
                Position::Fina => 4,
            }),
            3 => prev.map_or(0, |c| written_ids[&c.written]),
            4 => u64::from(self.prev_token),
            5 => u64::from(self.mvs_since_prev),
            6 => u64::from(self.cluster == CLUSTER_INITIAL_MEDIAL),
            7 => u64::from(self.masculine),
            8 => u64::from(self.particle),
            9 => u64::from(self.promise),
            _ => unreachable!(),
        }
    }

    /// The key of this context under `projection` (a bit set over `COMPONENTS`).
    pub fn key(
        &self,
        projection: u16,
        cands: &[Candidate],
        written_ids: &HashMap<Vec<WrittenUnit>, u64>,
    ) -> u64 {
        let mut key = 0u64;
        for (index, (_, bits)) in COMPONENTS.iter().enumerate() {
            if projection & (1 << index) != 0 {
                key = (key << bits) | self.component(index, cands, written_ids);
            }
        }
        key
    }
}
