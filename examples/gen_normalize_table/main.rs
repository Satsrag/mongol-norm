//! Generate `python/mongol_norm/data/MNG.normalize.json` — the tables of the online normalize
//! encoder (`src/encoder.rs`, `docs/internals.md`).
//!
//! The encoder commits letters left to right and never revises them; a committed letter must
//! render its own written units whatever the encoder writes next. Whether it does depends on the
//! letters before it, summarised by the encoder's context (`context.rs`, mirrored in
//! `src/encoder.rs`). This tool explores every context the encoder can reach, asks the shaping
//! engine — by shaping probe texts — which letters are safe to commit and which letter can end
//! the word there, and writes the answers as compact tables. It uses only the crate's public API
//! and the shaping-rules JSON.
//!
//! ```text
//! cargo run --release --example gen_normalize_table              # regenerate
//! cargo run --release --example gen_normalize_table -- --check   # exit 1 if stale
//! ```
//!
//! Then compile the JSON into the engine: `python3 python/scripts/gen_rust_tables.py`.

mod candidates;
mod compress;
mod context;
mod data;
mod explore;
#[allow(dead_code)]
#[path = "../../tests/common/json.rs"]
mod json;

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use mongol_norm::{Locale, Position, Shaper, WrittenUnit};

use candidates::{unit_names, Candidate};
use compress::{Final, Robust, Table};
use context::{allows, Trie, COMPONENTS, PROMISES};
use data::Rules;

const CANONICAL_VERSION: &str = "mng-canonical/3";
const SCHEMA: &str = "mongol-normalize-table/2";

struct Args {
    check: bool,
    out: Option<PathBuf>,
    threads: usize,
    verbose: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        check: false,
        out: None,
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        verbose: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--check" => args.check = true,
            "--verbose" | "-v" => args.verbose = true,
            "--out" => args.out = Some(it.next().ok_or("--out needs a path")?.into()),
            "--threads" => {
                args.threads = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .ok_or("--threads needs a number")?;
            }
            "-h" | "--help" => {
                return Err(
                    "usage: gen_normalize_table [--check] [--out PATH] [--threads N] [--verbose]"
                        .into(),
                )
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = root.join("python/mongol_norm/data");
    let target = args
        .out
        .clone()
        .unwrap_or_else(|| data_dir.join("MNG.normalize.json"));
    let start = std::time::Instant::now();

    let rules = Rules::load(&data_dir.join("MNG.json"));
    let alias_cp = |alias: &str| -> u32 {
        rules
            .letters
            .iter()
            .find(|l| l.alias == alias)
            .unwrap_or_else(|| panic!("unknown alias {alias}"))
            .cp
    };
    let cands = candidates::build(&rules);
    let trie = Trie::new(&rules.particles, alias_cp);
    let shaper = Shaper::new(Locale::Mng);
    let setup = explore::Setup::new(&shaper, &cands, &trie, &rules.particles, &alias_cp);
    let results = explore::explore(&setup, args.threads, args.verbose);

    let written: BTreeSet<String> = cands.iter().map(|c| unit_names(&c.written)).collect();
    let written_list: Vec<String> = written.into_iter().collect();
    let written_ids: HashMap<Vec<WrittenUnit>, u64> = cands
        .iter()
        .map(|c| {
            let id = written_list
                .iter()
                .position(|w| *w == unit_names(&c.written))
                .expect("listed") as u64
                + 1;
            (c.written.clone(), id)
        })
        .collect();
    let robust = match compress::robust_tables(&results, &cands, &written_ids) {
        Ok(tables) => tables,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    let group_units: Vec<Vec<WrittenUnit>> = setup
        .final_groups
        .iter()
        .map(|(_, units, _)| units.clone())
        .collect();
    let finals = match compress::final_tables(&results, &group_units, &cands, &written_ids) {
        Ok(tables) => tables,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    let text = render(
        &rules,
        &cands,
        &trie,
        &written_list,
        &robust,
        &setup.final_groups,
        &finals,
    );

    let probes: usize = results.iter().map(|r| r.probes).sum();
    let contexts: BTreeSet<_> = results.iter().map(|r| r.ctx).collect();
    let rows: usize = robust
        .iter()
        .map(|r| match r {
            Robust::Table(t) => t.rows.len(),
            _ => 0,
        })
        .sum();
    let final_rows: usize = finals
        .iter()
        .map(|f| match f {
            Final::Table(t) => t.rows.len(),
            Final::Always(_) => 0,
        })
        .sum();
    eprintln!(
        "{} candidates ({} context-dependent), {} contexts, {probes} probes, {rows} robustness rows, {final_rows} final rows, {:.1}s",
        cands.len(),
        cands.iter().filter(|c| !c.context_free).count(),
        contexts.len(),
        start.elapsed().as_secs_f64()
    );

    if args.check {
        let current = std::fs::read_to_string(&target).unwrap_or_default();
        if current == text {
            println!("fresh: {}", display(&root, &target));
            ExitCode::SUCCESS
        } else {
            eprintln!("stale: {}", display(&root, &target));
            eprintln!("regenerate with: cargo run --release --example gen_normalize_table (then python3 python/scripts/gen_rust_tables.py)");
            ExitCode::FAILURE
        }
    } else {
        std::fs::write(&target, text)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", target.display()));
        println!("wrote {}", display(&root, &target));
        ExitCode::SUCCESS
    }
}

fn display(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

// ── JSON output ────────────────────────────────────────────────────────────────────────────────

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    format!("[{}]", items.iter().map(f).collect::<Vec<_>>().join(", "))
}

fn projection_names(projection: u16) -> String {
    let names: Vec<&str> = COMPONENTS
        .iter()
        .enumerate()
        .filter(|(i, _)| projection & (1 << i) != 0)
        .map(|(_, (name, _))| *name)
        .collect();
    list(&names, |n| quote(n))
}

/// Index of `masks` in the shared pool (added on first use).
fn mask_set(pool: &mut Vec<[u128; 6]>, masks: &[u128; 6]) -> usize {
    match pool.iter().position(|m| m == masks) {
        Some(index) => index,
        None => {
            pool.push(*masks);
            pool.len() - 1
        }
    }
}

fn robust_json(table: &Table<[u128; 6]>, pool: &mut Vec<[u128; 6]>) -> String {
    let default = mask_set(pool, &table.default);
    let rows: Vec<String> = table
        .rows
        .iter()
        .map(|(key, masks)| format!("[\"{key:x}\", {}]", mask_set(pool, masks)))
        .collect();
    format!(
        "{{\"projection\": {}, \"default\": {default}, \"rows\": [{}]}}",
        projection_names(table.projection),
        rows.join(", ")
    )
}

fn valid_json(mask: &u32) -> String {
    format!("\"{mask:x}\"")
}

fn position_name(position: Position) -> &'static str {
    position.as_str()
}

#[allow(clippy::too_many_arguments)]
fn render(
    rules: &Rules,
    cands: &[Candidate],
    trie: &Trie,
    written_list: &[String],
    robust: &[Robust],
    final_groups: &[(Position, Vec<WrittenUnit>, Vec<u16>)],
    finals: &[Final],
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    let _ = writeln!(out, "  \"schema\": {},", quote(SCHEMA));
    let _ = writeln!(
        out,
        "  \"canonical_version\": {},",
        quote(CANONICAL_VERSION)
    );
    out.push_str("  \"locale\": \"MNG\",\n");
    let _ = writeln!(
        out,
        "  \"description\": {},",
        quote(
            "Tables of the online normalize encoder: encoding options per (position, written units) \
             in preference order, when each context-dependent option may be committed (projection \
             of the encoder context -> per-promise masks over the next unit), and the final-letter \
             choice. Generated by examples/gen_normalize_table; see docs/data-format.md."
        )
    );
    out.push_str("  \"constants\": {\"MVS\": \"180E\", \"NIRUGU\": \"180A\", \"ZWJ\": \"200D\", \"FVS1\": \"180B\", \"FVS2\": \"180C\", \"FVS3\": \"180D\", \"FVS4\": \"180F\"},\n");
    let _ = writeln!(
        out,
        "  \"context_components\": {},",
        list(&COMPONENTS, |(name, bits)| format!(
            "[{}, {bits}]",
            quote(name)
        ))
    );
    let promises: Vec<String> = PROMISES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let letters: Vec<String> = rules
                .letters
                .iter()
                .filter(|l| allows(i as u8 + 1, l.cp))
                .map(|l| quote(&l.alias))
                .collect();
            format!("[{}, [{}]]", quote(name), letters.join(", "))
        })
        .collect();
    let _ = writeln!(out, "  \"promises\": [{}],", promises.join(", "));
    let _ = writeln!(
        out,
        "  \"particle_nodes\": {},",
        list(&trie.nodes, |n| quote(n))
    );
    let _ = writeln!(
        out,
        "  \"written_sequences\": {},",
        list(written_list, |w| quote(w))
    );
    let _ = writeln!(
        out,
        "  \"mask_units\": {},",
        list(&WrittenUnit::ALL, |u| quote(u.as_str()))
    );
    let known: BTreeSet<&str> = rules
        .letters
        .iter()
        .flat_map(|l| {
            l.variants
                .iter()
                .flat_map(|v| v.written.iter().map(|u| u.as_str()))
        })
        .collect();
    let known: Vec<&str> = known.into_iter().collect();
    let _ = writeln!(out, "  \"known_units\": {},", list(&known, |u| quote(u)));
    let mut pool: Vec<[u128; 6]> = Vec::new();
    let mut candidate_lines: Vec<String> = Vec::new();
    for (cand, robust) in cands.iter().zip(robust) {
        let robust = match robust {
            Robust::Always => "\"always\"".to_string(),
            Robust::Never => "\"never\"".to_string(),
            Robust::Table(table) => robust_json(table, &mut pool),
        };
        let fvs = if cand.fvs == 0 {
            "null".to_string()
        } else {
            format!("\"{:04X}\"", candidates::fvs_char(cand.fvs) as u32)
        };
        candidate_lines.push(format!(
            "    {{\"letter\": {}, \"cp\": \"{:04X}\", \"fvs\": {fvs}, \"position\": \"{}\", \"units\": {}, \"written\": {}, \"robust\": {robust}}}",
            quote(rules.alias(cand.cp)),
            cand.cp,
            position_name(cand.position),
            quote(&unit_names(&cand.units)),
            quote(&unit_names(&cand.written)),
        ));
    }
    out.push_str("  \"mask_sets\": [\n");
    let sets: Vec<String> = pool
        .iter()
        .map(|set| format!("    {}", list(set, |m| format!("\"{m:x}\""))))
        .collect();
    out.push_str(&sets.join(",\n"));
    out.push_str("\n  ],\n");
    out.push_str("  \"candidates\": [\n");
    out.push_str(&candidate_lines.join(",\n"));
    out.push_str("\n  ],\n");
    out.push_str("  \"finals\": [\n");
    let entries: Vec<String> = final_groups
        .iter()
        .zip(finals)
        .filter_map(|((position, units, _), choice)| {
            let valid = match choice {
                Final::Always(0) => return None,
                Final::Always(mask) => valid_json(mask),
                Final::Table(table) => format!(
                    "{{\"projection\": {}, \"default\": {}, \"rows\": {}}}",
                    projection_names(table.projection),
                    valid_json(&table.default),
                    list(&table.rows, |(key, mask)| format!(
                        "[\"{key:x}\", {}]",
                        valid_json(mask)
                    ))
                ),
            };
            Some(format!(
                "    {{\"position\": \"{}\", \"units\": {}, \"valid\": {valid}}}",
                position_name(*position),
                quote(&unit_names(units))
            ))
        })
        .collect();
    out.push_str(&entries.join(",\n"));
    out.push_str("\n  ],\n");
    let positioned: BTreeSet<(String, String)> = rules
        .letters
        .iter()
        .flat_map(|l| l.variants.iter().flat_map(|v| v.positioned.iter().cloned()))
        .collect();
    let positioned: Vec<(String, String)> = positioned.into_iter().collect();
    let _ = writeln!(
        out,
        "  \"positioned_units\": {}",
        list(&positioned, |(unit, position)| format!(
            "{{\"unit\": {}, \"position\": {}}}",
            quote(unit),
            quote(position)
        ))
    );
    out.push_str("}\n");
    out
}
