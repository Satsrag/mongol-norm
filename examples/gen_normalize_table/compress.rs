//! Turn per-state answers into compact tables: for each function, the smallest set of context
//! components ("projection") that still determines it on every reachable state, and its rows.

use std::collections::{BTreeMap, HashMap};

use mongol_norm::WrittenUnit;

use crate::candidates::Candidate;
use crate::context::{Context, COMPONENTS};
use crate::explore::{unit_bit, StateResult};

/// A function of the context, tabulated over its projection: `default` for every key not in
/// `rows`.
pub struct Table<V> {
    pub projection: u16,
    pub default: V,
    pub rows: Vec<(u64, V)>,
}

/// Robustness of one candidate.
pub enum Robust {
    /// Context-free: always safe to commit at its position.
    Always,
    /// Never committed in any reachable state.
    Never,
    /// Per context: one mask per promise (`0` = none) over the next unit.
    Table(Table<[u128; 6]>),
}

/// Which options of one `(position, units)` group can end the word.
pub enum Final {
    Always(u32),
    Table(Table<u32>),
}

/// Projections in order of preference: fewest components first, then by bit pattern.
fn projections() -> Vec<u16> {
    let mut all: Vec<u16> = (0..(1u16 << COMPONENTS.len())).collect();
    all.sort_by_key(|p| (p.count_ones(), *p));
    all
}

/// The projection under which `samples` is a function with the fewest exception rows (then the
/// fewest components); the most common value becomes the default.
fn tabulate<V: Clone + PartialEq>(
    samples: &[(Context, V)],
    cands: &[Candidate],
    written_ids: &HashMap<Vec<WrittenUnit>, u64>,
) -> Table<V> {
    let mut best: Option<Table<V>> = None;
    'projection: for projection in projections() {
        let mut rows: BTreeMap<u64, V> = BTreeMap::new();
        for (ctx, value) in samples {
            let key = ctx.key(projection, cands, written_ids);
            match rows.get(&key) {
                Some(existing) if existing != value => continue 'projection,
                Some(_) => {}
                None => {
                    rows.insert(key, value.clone());
                }
            }
        }
        // the most common value becomes the default (the first of equally common ones, in key
        // order)
        let mut counts: Vec<(V, usize)> = Vec::new();
        for value in rows.values() {
            match counts.iter_mut().find(|(v, _)| v == value) {
                Some((_, n)) => *n += 1,
                None => counts.push((value.clone(), 1)),
            }
        }
        let mut default = &counts[0];
        for entry in &counts {
            if entry.1 > default.1 {
                default = entry;
            }
        }
        let default = default.0.clone();
        let exceptions: Vec<(u64, V)> = rows.into_iter().filter(|(_, v)| *v != default).collect();
        if best
            .as_ref()
            .is_none_or(|b| exceptions.len() < b.rows.len())
        {
            best = Some(Table {
                projection,
                default,
                rows: exceptions,
            });
            if best.as_ref().is_some_and(|b| b.rows.is_empty()) {
                break;
            }
        }
    }
    best.expect("the full projection distinguishes every context")
}

/// Check that states with the same context got the same answers (the representative text does
/// not matter) and collect the samples per candidate / per final group.
pub fn robust_tables(
    results: &[StateResult],
    cands: &[Candidate],
    written_ids: &HashMap<Vec<WrittenUnit>, u64>,
) -> Result<Vec<Robust>, String> {
    let mut samples: Vec<BTreeMap<Context, [u128; 6]>> = vec![BTreeMap::new(); cands.len()];
    for result in results {
        for (id, masks) in &result.robust {
            let cand = &cands[*id as usize];
            // only asked when the tail starts with this candidate's first unit
            if result.allowed_next & (1u128 << unit_bit(cand.units[0])) == 0 {
                continue;
            }
            let slot = &mut samples[*id as usize];
            match slot.get(&result.ctx) {
                Some(existing) if existing != masks => {
                    return Err(format!(
                        "candidate {id} has two robustness answers in context {:?} (the context misses something the rules see)",
                        result.ctx
                    ))
                }
                Some(_) => {}
                None => {
                    slot.insert(result.ctx, *masks);
                }
            }
        }
    }
    Ok(cands
        .iter()
        .zip(samples)
        .map(|(cand, samples)| {
            if cand.context_free {
                Robust::Always
            } else if samples.values().all(|masks| masks.iter().all(|m| *m == 0)) {
                Robust::Never
            } else {
                let samples: Vec<(Context, [u128; 6])> = samples.into_iter().collect();
                Robust::Table(tabulate(&samples, cands, written_ids))
            }
        })
        .collect())
}

pub fn final_tables(
    results: &[StateResult],
    groups: &[Vec<WrittenUnit>],
    cands: &[Candidate],
    written_ids: &HashMap<Vec<WrittenUnit>, u64>,
) -> Result<Vec<Final>, String> {
    let mut samples: Vec<BTreeMap<Context, u32>> = vec![BTreeMap::new(); groups.len()];
    for result in results {
        for (group, valid) in &result.finals {
            // only asked when the tail is exactly this group's units
            if result.allowed_next & (1u128 << unit_bit(groups[*group][0])) == 0 {
                continue;
            }
            let slot = &mut samples[*group];
            match slot.get(&result.ctx) {
                Some(existing) if existing != valid => {
                    return Err(format!(
                        "final group {group} has two answers in context {:?}",
                        result.ctx
                    ))
                }
                Some(_) => {}
                None => {
                    slot.insert(result.ctx, *valid);
                }
            }
        }
    }
    Ok(samples
        .into_iter()
        .map(|samples| {
            let first = samples.values().next().copied().unwrap_or(0);
            if samples.values().all(|v| *v == first) {
                return Final::Always(first);
            }
            let samples: Vec<(Context, u32)> = samples.into_iter().collect();
            let table = tabulate(&samples, cands, written_ids);
            if table.rows.is_empty() {
                Final::Always(table.default)
            } else {
                Final::Table(table)
            }
        })
        .collect())
}
