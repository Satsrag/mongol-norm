//! Compare the two shapers word by word: `FontShaper` (HarfBuzz rules — the bundled
//! mongfontbuilder Hudum font through harfrust) against `Shaper` (the ported rules).
//!
//! ```text
//! cargo run --release --example compare --features font -- --random 100000
//! cargo run --release --example compare --features font -- words.txt --tsv diff.tsv
//! ```
//!
//! The report is printed to stdout: a summary, the differences grouped by the units the two
//! shapers disagree on (each with an example word), and the first differing words. `--tsv FILE`
//! also writes every differing word to FILE, tab-separated, for a spreadsheet or `grep`;
//! `--markdown FILE` appends the report as Markdown (CI points it at the job summary) and
//! `--github-warning` adds a GitHub Actions annotation. `--fail-on-diff` turns the report into a
//! gate: the exit status is 1 when any word differs. Both shapers are compared on the public
//! `shape`; see `docs/development.md`.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufWriter, IsTerminal, Write};
use std::process::ExitCode;

use mongol_norm::{is_mongolian_word_char, FontShaper, Locale, Shaper, WrittenUnit};

const USAGE: &str = "\
usage: cargo run --release --example compare --features font -- [OPTIONS] [FILE ...]

Compare FontShaper (the bundled mongfontbuilder Hudum font through harfrust) with Shaper (the
ported rules) and report the words they shape differently. The report goes to stdout.

input (one of):
  FILE ...          the Mongolian words of these text files ('-' is stdin)
  (no FILE)         the Mongolian words of stdin
  --random N        N random strings over the Hudum letters, FVS1-4, MVS, NNBSP, nirugu and ZWJ

options:
  --seed S          seed of --random (default 1)
  --max-len L       longest random string (default 8)
  --limit K         differing words to list (default 50; 0 lists none)
  --groups G        difference groups to list (default 20; 0 lists none)
  --tsv FILE        also write every differing word to FILE (tab-separated, with a header)
  --markdown FILE   also append the report to FILE as Markdown (CI: $GITHUB_STEP_SUMMARY)
  --github-warning  also print a GitHub Actions annotation: a warning when any word differs
                    (an error with --fail-on-diff), a notice when none does
  --fail-on-diff    exit with status 1 when any word differs or is shaped by one side only
  -h, --help        show this help

The exit status is 0 whatever the shapers do, unless --fail-on-diff is given and they differ
(1). A usage or I/O error exits with 2.
";

struct Options {
    files: Vec<String>,
    random: Option<usize>,
    seed: u64,
    max_len: usize,
    limit: usize,
    groups: usize,
    tsv: Option<String>,
    markdown: Option<String>,
    github_warning: bool,
    fail_on_diff: bool,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        files: Vec::new(),
        random: None,
        seed: 1,
        max_len: 8,
        limit: 50,
        groups: 20,
        tsv: None,
        markdown: None,
        github_warning: false,
        fail_on_diff: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        let number = |name: &str, text: String| {
            text.parse::<u64>()
                .map_err(|_| format!("{name}: not a number: {text}"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--random" => options.random = Some(number("--random", value("--random")?)? as usize),
            "--seed" => options.seed = number("--seed", value("--seed")?)?,
            "--max-len" => options.max_len = number("--max-len", value("--max-len")?)? as usize,
            "--limit" => options.limit = number("--limit", value("--limit")?)? as usize,
            "--groups" => options.groups = number("--groups", value("--groups")?)? as usize,
            "--tsv" => options.tsv = Some(value("--tsv")?),
            "--markdown" => options.markdown = Some(value("--markdown")?),
            "--github-warning" => options.github_warning = true,
            "--fail-on-diff" => options.fail_on_diff = true,
            _ if arg.starts_with("--") => return Err(format!("unknown option {arg}")),
            _ => options.files.push(arg),
        }
    }
    if options.random.is_some() && !options.files.is_empty() {
        return Err("--random takes no FILE".to_owned());
    }
    if options.max_len == 0 {
        return Err("--max-len must be at least 1".to_owned());
    }
    Ok(options)
}

/// The Mongolian words of some text: the runs of word characters, each kept once, in order.
fn words_of(
    reader: impl BufRead,
    words: &mut Vec<String>,
    seen: &mut HashSet<String>,
) -> io::Result<()> {
    for line in reader.lines() {
        for word in line?.split(|c: char| !is_mongolian_word_char(c)) {
            if !word.is_empty() && seen.insert(word.to_owned()) {
                words.push(word.to_owned());
            }
        }
    }
    Ok(())
}

fn read_words(files: &[String]) -> io::Result<Vec<String>> {
    let mut words = Vec::new();
    let mut seen = HashSet::new();
    if files.is_empty() {
        if io::stdin().is_terminal() {
            eprintln!("reading words from stdin (end with Ctrl-D); --help for the options");
        }
        words_of(io::stdin().lock(), &mut words, &mut seen)?;
    }
    for file in files {
        if file == "-" {
            words_of(io::stdin().lock(), &mut words, &mut seen)?;
        } else {
            let reader = io::BufReader::new(
                File::open(file)
                    .map_err(|error| io::Error::new(error.kind(), format!("{file}: {error}")))?,
            );
            words_of(reader, &mut words, &mut seen)?;
        }
    }
    Ok(words)
}

/// A deterministic xorshift stream, so a seed always gives the same strings.
struct XorShift(u64);

impl XorShift {
    fn below(&mut self, n: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x % n as u64) as usize
    }
}

fn random_words(count: usize, seed: u64, max_len: usize) -> Vec<String> {
    let mut alphabet: Vec<char> = (0x1820..=0x1842).filter_map(char::from_u32).collect();
    alphabet.extend([
        '\u{180B}', '\u{180C}', '\u{180D}', '\u{180F}', '\u{180E}', '\u{202F}', '\u{180A}',
        '\u{200D}',
    ]);
    // Seed 0 would make xorshift emit zeros forever; mix the seed so nearby seeds diverge.
    let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    (0..count)
        .map(|_| {
            let len = 1 + rng.below(max_len);
            (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())])
                .collect()
        })
        .collect()
}

fn joined(units: &[WrittenUnit]) -> String {
    if units.is_empty() {
        return "∅".to_owned();
    }
    units
        .iter()
        .map(|unit| unit.as_str())
        .collect::<Vec<_>>()
        .join("+")
}

fn code_points(word: &str) -> String {
    word.chars()
        .map(|c| format!("{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The part of two unit sequences between their common prefix and common suffix: what the two
/// shapers actually disagree on (`F+T+K2` vs `F+D+K2` → `T` vs `D`).
fn middle<'a>(
    a: &'a [WrittenUnit],
    b: &'a [WrittenUnit],
) -> (&'a [WrittenUnit], &'a [WrittenUnit]) {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix])
}

/// The upstream commit the bundled font was built at, from its pin file.
fn pinned_commit() -> &'static str {
    include_str!("../fonts/mongfontbuilder.json")
        .split("\"commit\": \"")
        .nth(1)
        .and_then(|rest| rest.get(..12))
        .unwrap_or("?")
}

struct Difference {
    word: String,
    font: String,
    engine: String,
    /// The disagreeing parts (font, engine) — the group the word is counted in.
    group: (String, String),
}

/// A word only one side shapes: the side that rejected it, and the error.
struct Rejected {
    word: String,
    side: &'static str,
    error: String,
}

/// Words that differ in the same parts: how many, and the first of them.
struct Group {
    font: String,
    engine: String,
    count: usize,
    example: String,
}

struct Report {
    input: String,
    total: usize,
    differences: Vec<Difference>,
    rejected: Vec<Rejected>,
    /// Most frequent first.
    groups: Vec<Group>,
}

impl Report {
    fn same(&self) -> usize {
        self.total - self.differences.len() - self.rejected.len()
    }

    fn percent(&self, n: usize) -> f64 {
        100.0 * n as f64 / self.total.max(1) as f64
    }
}

fn compare(options: &Options, words: &[String]) -> Report {
    let font = FontShaper::new();
    let engine = Shaper::new(Locale::Mng);
    let mut differences = Vec::new();
    let mut rejected = Vec::new();
    for word in words {
        match (font.shape(word), engine.shape(word)) {
            (Ok(a), Ok(b)) if a == b => {}
            (Ok(a), Ok(b)) => {
                let (x, y) = middle(&a, &b);
                differences.push(Difference {
                    word: word.clone(),
                    font: joined(&a),
                    engine: joined(&b),
                    group: (joined(x), joined(y)),
                });
            }
            (Err(error), Ok(_)) => rejected.push(Rejected {
                word: word.clone(),
                side: "font",
                error: error.to_string(),
            }),
            (Ok(_), Err(error)) => rejected.push(Rejected {
                word: word.clone(),
                side: "engine",
                error: error.to_string(),
            }),
            (Err(_), Err(_)) => {}
        }
    }

    let mut counts: HashMap<&(String, String), (usize, &str)> = HashMap::new();
    for difference in &differences {
        counts
            .entry(&difference.group)
            .or_insert((0, &difference.word))
            .0 += 1;
    }
    let mut groups: Vec<Group> = counts
        .into_iter()
        .map(|((font, engine), (count, example))| Group {
            font: font.clone(),
            engine: engine.clone(),
            count,
            example: example.to_owned(),
        })
        .collect();
    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| (&a.font, &a.engine).cmp(&(&b.font, &b.engine)))
    });

    let input = match options.random {
        Some(count) => format!(
            "{count} random strings (seed {}, length 1-{})",
            options.seed, options.max_len
        ),
        None => format!("{} distinct words", words.len()),
    };
    Report {
        input,
        total: words.len(),
        differences,
        rejected,
        groups,
    }
}

fn write_text(out: &mut impl Write, report: &Report, options: &Options) -> io::Result<()> {
    writeln!(
        out,
        "FontShaper (fonts/hudum.otf, mongfontbuilder {}) vs Shaper (ported rules), public shape",
        pinned_commit()
    )?;
    writeln!(out, "input: {}", report.input)?;
    writeln!(
        out,
        "same: {}   differ: {} ({:.2}%)   shaped by one side only: {}",
        report.same(),
        report.differences.len(),
        report.percent(report.differences.len()),
        report.rejected.len()
    )?;

    if options.groups > 0 && !report.groups.is_empty() {
        writeln!(
            out,
            "\nWhere they differ ({} groups; font vs engine, between the common prefix and suffix):",
            report.groups.len()
        )?;
        let shown = &report.groups[..report.groups.len().min(options.groups)];
        let width = shown
            .iter()
            .map(|group| group.font.chars().count())
            .max()
            .unwrap_or(0);
        for group in shown {
            writeln!(
                out,
                "  {:>6}  {:<width$}  vs  {:<12}  e.g. {}  ({})",
                group.count,
                group.font,
                group.engine,
                group.example,
                code_points(&group.example)
            )?;
        }
        if report.groups.len() > shown.len() {
            writeln!(
                out,
                "  … {} more groups (--groups N)",
                report.groups.len() - shown.len()
            )?;
        }
    }

    if options.limit > 0 && !report.differences.is_empty() {
        let shown = &report.differences[..report.differences.len().min(options.limit)];
        writeln!(
            out,
            "\nDiffering words ({} of {}; --limit N for more, --tsv FILE for all):",
            shown.len(),
            report.differences.len()
        )?;
        let font_width = shown.iter().map(|d| d.font.len()).max().unwrap_or(0).max(4);
        let engine_width = shown
            .iter()
            .map(|d| d.engine.len())
            .max()
            .unwrap_or(0)
            .max(6);
        writeln!(
            out,
            "  {:<font_width$}  {:<engine_width$}  code points / word",
            "font", "engine"
        )?;
        for difference in shown {
            writeln!(
                out,
                "  {:<font_width$}  {:<engine_width$}  {}  {}",
                difference.font,
                difference.engine,
                code_points(&difference.word),
                difference.word
            )?;
        }
    }

    if !report.rejected.is_empty() {
        writeln!(
            out,
            "\nShaped by one side only ({}):",
            report.rejected.len()
        )?;
        for rejected in report.rejected.iter().take(options.limit.max(1)) {
            writeln!(
                out,
                "  {} rejects {}  {}: {}",
                rejected.side,
                code_points(&rejected.word),
                rejected.word,
                rejected.error
            )?;
        }
    }
    Ok(())
}

fn write_markdown(out: &mut impl Write, report: &Report, options: &Options) -> io::Result<()> {
    writeln!(out, "## FontShaper vs Shaper\n")?;
    writeln!(
        out,
        "`fonts/hudum.otf` (mongfontbuilder `{}`) against the ported rules, public `shape`, on {}.\n",
        pinned_commit(),
        report.input
    )?;
    writeln!(out, "| words | same | differ | shaped by one side only |")?;
    writeln!(out, "|---:|---:|---:|---:|")?;
    writeln!(
        out,
        "| {} | {} | {} ({:.2}%) | {} |\n",
        report.total,
        report.same(),
        report.differences.len(),
        report.percent(report.differences.len()),
        report.rejected.len()
    )?;

    if options.groups > 0 && !report.groups.is_empty() {
        let shown = &report.groups[..report.groups.len().min(options.groups)];
        writeln!(
            out,
            "### Where they differ: {} groups{}\n",
            report.groups.len(),
            if shown.len() < report.groups.len() {
                format!(" (the {} largest)", shown.len())
            } else {
                String::new()
            }
        )?;
        writeln!(
            out,
            "The units between the common prefix and suffix of the two shapes; `∅` is nothing.\n"
        )?;
        writeln!(out, "| words | font | engine | example | code points |")?;
        writeln!(out, "|---:|---|---|---|---|")?;
        for group in shown {
            writeln!(
                out,
                "| {} | `{}` | `{}` | {} | `{}` |",
                group.count,
                group.font,
                group.engine,
                group.example,
                code_points(&group.example)
            )?;
        }
        writeln!(out)?;
    }

    if options.limit > 0 && !report.differences.is_empty() {
        let shown = &report.differences[..report.differences.len().min(options.limit)];
        writeln!(
            out,
            "<details><summary>The first {} of {} differing words</summary>\n",
            shown.len(),
            report.differences.len()
        )?;
        writeln!(out, "| font | engine | code points | word |")?;
        writeln!(out, "|---|---|---|---|")?;
        for difference in shown {
            writeln!(
                out,
                "| `{}` | `{}` | `{}` | {} |",
                difference.font,
                difference.engine,
                code_points(&difference.word),
                difference.word
            )?;
        }
        writeln!(out, "\n</details>\n")?;
    }

    if !report.rejected.is_empty() {
        writeln!(
            out,
            "### Shaped by one side only: {}\n",
            report.rejected.len()
        )?;
        writeln!(out, "| rejected by | code points | word | error |")?;
        writeln!(out, "|---|---|---|---|")?;
        for rejected in report.rejected.iter().take(options.limit.max(1)) {
            writeln!(
                out,
                "| {} | `{}` | {} | {} |",
                rejected.side,
                code_points(&rejected.word),
                rejected.word,
                rejected.error
            )?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// The CI artifact a TSV is uploaded as: its file name without the extension.
fn artifact_name(path: &str) -> Option<&str> {
    std::path::Path::new(path).file_stem()?.to_str()
}

/// A GitHub Actions workflow-command value: `%`, CR and LF escaped (and, in a property, `:`
/// and `,` too).
fn escape_command(text: &str, property: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' if property => out.push_str("%3A"),
            ',' if property => out.push_str("%2C"),
            _ => out.push(c),
        }
    }
    out
}

/// One annotation for the whole run: a warning that names the largest groups when any word
/// differs (or only one side shapes it), a notice when the shapers agree on every word.
fn github_annotation(report: &Report, fail_on_diff: bool, artifact: Option<&str>) -> String {
    let title = format!(
        "FontShaper vs Shaper: {} of {} differ ({:.2}%)",
        report.differences.len(),
        report.total,
        report.percent(report.differences.len())
    );
    if report.differences.is_empty() && report.rejected.is_empty() {
        return format!(
            "::notice title={}::The two shapers agree on all {}.",
            escape_command(&title, true),
            report.input
        );
    }
    let mut message = format!(
        "Input: {}. {} groups; the largest (font vs engine):",
        report.input,
        report.groups.len()
    );
    for group in report.groups.iter().take(5) {
        message.push_str(&format!(
            "\n{}× {} vs {}, e.g. {} ({})",
            group.count,
            group.font,
            group.engine,
            group.example,
            code_points(&group.example)
        ));
    }
    if !report.rejected.is_empty() {
        message.push_str(&format!(
            "\n{} words are shaped by one side only.",
            report.rejected.len()
        ));
    }
    match artifact {
        Some(name) => message.push_str(&format!(
            "\nFull report: the job summary; every word: the {name} artifact."
        )),
        None => message.push_str("\nFull report: the job summary."),
    }
    let level = if fail_on_diff { "error" } else { "warning" };
    format!(
        "::{level} title={}::{}",
        escape_command(&title, true),
        escape_command(&message, false)
    )
}

fn write_tsv(path: &str, report: &Report) -> io::Result<()> {
    let mut tsv = BufWriter::new(File::create(path)?);
    writeln!(
        tsv,
        "word\tcode_points\tfont\tengine\tfont_part\tengine_part"
    )?;
    for difference in &report.differences {
        writeln!(
            tsv,
            "{}\t{}\t{}\t{}\t{}\t{}",
            difference.word,
            code_points(&difference.word),
            difference.font,
            difference.engine,
            difference.group.0,
            difference.group.1
        )?;
    }
    for rejected in &report.rejected {
        let error = format!("error: {}", rejected.error);
        let (font, engine) = if rejected.side == "font" {
            (error.as_str(), "")
        } else {
            ("", error.as_str())
        };
        writeln!(
            tsv,
            "{}\t{}\t{font}\t{engine}\t\t",
            rejected.word,
            code_points(&rejected.word)
        )?;
    }
    tsv.flush()
}

/// Writes the report; `Ok(true)` when the shapers agree on every word.
fn run(options: &Options, words: &[String]) -> io::Result<bool> {
    let report = compare(options, words);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    write_text(&mut out, &report, options)?;
    if let Some(path) = &options.tsv {
        write_tsv(path, &report)?;
        writeln!(
            out,
            "\nwrote {} rows to {path}",
            report.differences.len() + report.rejected.len()
        )?;
    }
    if let Some(path) = &options.markdown {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut markdown = BufWriter::new(file);
        write_markdown(&mut markdown, &report, options)?;
        markdown.flush()?;
        writeln!(out, "appended the Markdown report to {path}")?;
    }
    if options.github_warning {
        let artifact = options.tsv.as_deref().and_then(artifact_name);
        writeln!(
            out,
            "{}",
            github_annotation(&report, options.fail_on_diff, artifact)
        )?;
    }
    out.flush()?;
    Ok(report.differences.is_empty() && report.rejected.is_empty())
}

fn main() -> ExitCode {
    let options = match parse_args() {
        Ok(options) => options,
        Err(message) if message.is_empty() => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("compare: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let words = match options.random {
        Some(count) => random_words(count, options.seed, options.max_len),
        None => match read_words(&options.files) {
            Ok(words) => words,
            Err(error) => {
                eprintln!("compare: {error}");
                return ExitCode::from(2);
            }
        },
    };
    match run(&options, &words) {
        Ok(agree) if agree || !options.fail_on_diff => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(error) => {
            eprintln!("compare: {error}");
            ExitCode::from(2)
        }
    }
}
