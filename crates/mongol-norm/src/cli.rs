//! The `mongol-norm` command line — a port of `mongol_norm/shaper.py::main` and its helpers
//! (`_read_input`, `_write_output`, `_process_batch`, argument handling). Hidden from the docs:
//! it exists so the binary is a shim and the crate's own tests can drive every path
//! in-process, including normalization fallback (which no real input triggers).
//!
//! Intentional differences from the Python CLI (all deliberate, pinned by `tests/cli.rs` and
//! this module's unit tests):
//!
//! * `-V` / `--version` exists, so the usage line reads `[-V]`.
//! * `<cmd> --help` prints the global help instead of the sub-command's (an *unknown* command
//!   still loses to argparse's `invalid choice`, which resolves CMD before `-h`).
//! * `invalid choice` quotes the choices argparse-≤3.13 style; argparse 3.14 dropped the quotes.
//! * `--locale XX` is a usage error (usage line, exit 2) where Python tracebacks on the missing
//!   `data/XX.json`.
//! * The remaining places Python lets an exception escape are `error: …` with exit 2 here: a
//!   missing or unreadable `-i` file and a non-UTF-8 input file (Python: `FileNotFoundError` /
//!   `UnicodeDecodeError`), an unwritable `-o` path (`OSError`), and EPIPE on stdout — Python
//!   prints `Exception ignored while flushing sys.stdout: BrokenPipeError` at shutdown (exit
//!   120) or a `BrokenPipeError` traceback when a mid-write flush fails (exit 1), while this
//!   CLI prints `error: Broken pipe (os error 32)` and exits 2.
//! * A hyphen-initial `same` argument is rejected with the global usage line and
//!   `unrecognized arguments: …`; Python's `same` sub-parser instead reports the missing
//!   `text1, text2`.
//! * `same` on invalid input (e.g. a non-Mongolian character) reports `error: …` with exit 2;
//!   Python lets the `ValueError` escape (traceback, exit 1 — which collided with the
//!   "shapes differ" exit code).
//! * Non-UTF-8 argv is rejected instead of being smuggled through as surrogate escapes.

use std::io::{self, Read, Write};

use crate::{Error, Locale, Shaper};

const COMMANDS: [&str; 5] = [
    "shape",
    "normalize",
    "normalize-written-units",
    "normalize-text",
    "same",
];

/// The one place the usage line is spelled; [`USAGE`] and [`HELP`] are both built from it so the
/// two can never drift apart.
macro_rules! usage_line {
    () => {
        "usage: mongol-norm [-h] [-V] [--locale LOCALE] CMD ..."
    };
}

const USAGE: &str = usage_line!();

const HELP: &str = concat!(
    usage_line!(),
    "

Mongolian shaping / normalization tool (UTN #57 v4 + GB/T 25914-2023).
蒙古文字形 / 规范化工具 (UTN #57 v4 + GB/T 25914-2023)。

options:
  -h, --help            show this help and exit
  -V, --version         print the version and exit
  --locale LOCALE       MNG (default), TOD, SIB or MCH (also --locale=LOCALE)

commands:
  shape                   Return the '+'-joined written-unit sequence
  normalize               Normalize a single Mongolian WORD to canonical Unicode
  normalize-written-units Encode compact or '+'-joined PascalCase units as canonical Unicode
  normalize-text          Normalize full text (multi-word, mixed script)
  same TEXT1 TEXT2        Exit 0 if both inputs shape identically, 1 otherwise

I/O modes (shape / normalize / normalize-text / normalize-written-units):
  inline      :  mongol-norm <cmd> 'TEXT'
  stdin       :  echo 'TEXT' | mongol-norm <cmd> -
  file input  :  mongol-norm <cmd> -i input.txt
                 (also --input FILE and --input=FILE)
  file output :  mongol-norm <cmd> -i input.txt -o output.txt
                 (also --output FILE and --output=FILE)
  batch       :  mongol-norm <cmd> --batch -i words.txt -o out.txt
                 (one word/text per line in, one result per line out)
  --allow-fallback (normalize, normalize-text): keep an uncovered word instead of failing
  --                    end of options: every later argument is text, even a leading '-'

Examples / 示例:
  mongol-norm normalize 'ᠰᠡᠢᠨ'
  mongol-norm shape 'ᠰᠠᠢᠨ'                          # → S+A+I+I+A
  mongol-norm normalize-written-units 'B+Aa'           # → ᠪᠠ᠋
  mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ world'
  mongol-norm normalize-text -- '-ᠰᠡᠢᠨ'                # text starting with '-'
  echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -
"
);

/// Run the CLI on the process arguments and standard streams; returns the exit code.
pub fn main() -> i32 {
    let mut args: Vec<String> = Vec::new();
    for argument in std::env::args_os().skip(1) {
        match argument.into_string() {
            Ok(text) => args.push(text),
            Err(raw) => {
                let lossy = raw.to_string_lossy().into_owned();
                let _ = writeln!(
                    io::stderr(),
                    "{USAGE}\nmongol-norm: error: argument is not valid UTF-8: {lossy}"
                );
                return 2;
            }
        }
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    run(
        &args,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut stderr.lock(),
    )
}

/// Run the CLI with explicit streams (`args` excludes the program name); returns the exit code.
pub(crate) fn run(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    run_with(args, stdin, stdout, stderr, Shaper::new)
}

/// [`run`] with a custom shaper factory (tests inject a table-less shaper).
pub(crate) fn run_with(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    make_shaper: impl Fn(Locale) -> Shaper,
) -> i32 {
    match execute(args, stdin, stdout, &make_shaper) {
        Ok(code) => code,
        Err(Failure::Usage(message)) => {
            let _ = writeln!(stderr, "{USAGE}\nmongol-norm: error: {message}");
            2
        }
        Err(Failure::Operation(message)) => {
            let _ = writeln!(stderr, "error: {message}");
            2
        }
    }
}

enum Failure {
    /// Bad arguments: usage line + message, exit 2 (argparse style).
    Usage(String),
    /// A runtime error: `error: …`, exit 2 (Python catches `ValueError`).
    Operation(String),
}

impl From<Error> for Failure {
    fn from(error: Error) -> Failure {
        Failure::Operation(error.to_string())
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Failure {
        Failure::Operation(error.to_string())
    }
}

struct IoArgs {
    text: Option<String>,
    input: Option<String>,
    output: Option<String>,
    batch: bool,
    allow_fallback: bool,
}

enum Command {
    Shape(IoArgs),
    Normalize(IoArgs),
    NormalizeWrittenUnits(IoArgs),
    NormalizeText(IoArgs),
    Same(String, String),
    Help,
    Version,
}

fn parse_locale(value: &str) -> Result<Locale, Failure> {
    value
        .parse::<Locale>()
        .map_err(|error| Failure::Usage(format!("argument --locale: {error}")))
}

/// The value of a `--name=VALUE` argument, if `arg` is that option in its `=` form.
fn split_long<'a>(arg: &'a str, name: &str) -> Option<&'a str> {
    arg.strip_prefix(name)?.strip_prefix('=')
}

/// True for an argument argparse would read as an option (a bare `-` is stdin, not an option).
fn looks_like_option(arg: &str) -> bool {
    arg.starts_with('-') && arg != "-"
}

/// argparse's `argument CMD: invalid choice`, with the choices quoted the way argparse ≤ 3.13
/// quotes them (3.14 prints them bare).
fn invalid_choice(name: &str) -> Failure {
    let choices: Vec<String> = COMMANDS.iter().map(|c| format!("'{c}'")).collect();
    Failure::Usage(format!(
        "argument CMD: invalid choice: '{name}' (choose from {})",
        choices.join(", ")
    ))
}

fn parse(args: &[String]) -> Result<(Locale, Command), Failure> {
    let mut locale = Locale::Mng;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            // End of options: the next argument is CMD even if it starts with '-'.
            "--" => {
                index += 1;
                break;
            }
            "-h" | "--help" => return Ok((locale, Command::Help)),
            "-V" | "--version" => return Ok((locale, Command::Version)),
            "--locale" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    Failure::Usage("argument --locale: expected one argument".to_owned())
                })?;
                locale = parse_locale(value)?;
                index += 2;
            }
            _ => {
                if let Some(value) = split_long(arg, "--locale") {
                    locale = parse_locale(value)?;
                    index += 1;
                } else if looks_like_option(arg) {
                    return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
                } else {
                    break;
                }
            }
        }
    }
    let Some(name) = args.get(index) else {
        return Err(Failure::Usage(
            "the following arguments are required: CMD".to_owned(),
        ));
    };
    let rest = &args[index + 1..];
    // argparse resolves CMD against its sub-parsers before any of them sees `-h`, so
    // `mongol-norm bogus --help` is an invalid choice, not a help request.
    if !COMMANDS.contains(&name.as_str()) {
        return Err(invalid_choice(name));
    }
    // A `-h` after `--` is text, not a help request.
    if rest
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "-h" || arg == "--help")
    {
        return Ok((locale, Command::Help));
    }
    let command = match name.as_str() {
        "same" => match parse_same(rest)?.as_slice() {
            [a, b] => Command::Same((*a).to_owned(), (*b).to_owned()),
            _ => {
                return Err(Failure::Usage(
                    "same: expected exactly two arguments: TEXT1 TEXT2".to_owned(),
                ))
            }
        },
        "shape" => Command::Shape(parse_io("shape", rest)?),
        "normalize" => Command::Normalize(parse_io("normalize", rest)?),
        "normalize-written-units" => {
            Command::NormalizeWrittenUnits(parse_io("normalize-written-units", rest)?)
        }
        "normalize-text" => Command::NormalizeText(parse_io("normalize-text", rest)?),
        // Unreachable while `COMMANDS` and these arms agree; kept so a command added to one
        // and not the other degrades to the argparse error instead of a panic.
        other => return Err(invalid_choice(other)),
    };
    Ok((locale, command))
}

/// `same` has no options at all: its two arguments are positional, with `--` ending options.
fn parse_same(rest: &[String]) -> Result<Vec<&str>, Failure> {
    let mut positional = Vec::new();
    let mut positional_only = false;
    for arg in rest {
        let arg = arg.as_str();
        if !positional_only {
            if arg == "--" {
                positional_only = true;
                continue;
            }
            if looks_like_option(arg) {
                return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
            }
        }
        positional.push(arg);
    }
    Ok(positional)
}

fn parse_io(name: &str, rest: &[String]) -> Result<IoArgs, Failure> {
    let mut io_args = IoArgs {
        text: None,
        input: None,
        output: None,
        batch: false,
        allow_fallback: false,
    };
    let mut positional_only = false;
    let mut index = 0;
    while index < rest.len() {
        let arg = rest[index].as_str();
        if positional_only {
            set_text(&mut io_args, arg)?;
            index += 1;
            continue;
        }
        match arg {
            // End of options: every later argument is the positional text.
            "--" => {
                positional_only = true;
                index += 1;
            }
            "-i" | "--input" | "-o" | "--output" => {
                let value = rest
                    .get(index + 1)
                    .ok_or_else(|| {
                        Failure::Usage(format!("argument {arg}: expected one argument"))
                    })?
                    .clone();
                if arg == "-i" || arg == "--input" {
                    io_args.input = Some(value);
                } else {
                    io_args.output = Some(value);
                }
                index += 2;
            }
            "--batch" => {
                io_args.batch = true;
                index += 1;
            }
            "--allow-fallback" if matches!(name, "normalize" | "normalize-text") => {
                io_args.allow_fallback = true;
                index += 1;
            }
            _ => {
                if let Some(value) = split_long(arg, "--input") {
                    io_args.input = Some(value.to_owned());
                } else if let Some(value) = split_long(arg, "--output") {
                    io_args.output = Some(value.to_owned());
                } else if looks_like_option(arg) {
                    return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
                } else {
                    set_text(&mut io_args, arg)?;
                }
                index += 1;
            }
        }
    }
    Ok(io_args)
}

/// Store the single positional TEXT; a second one is an argparse "unrecognized arguments" error.
fn set_text(io_args: &mut IoArgs, arg: &str) -> Result<(), Failure> {
    if io_args.text.is_some() {
        return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
    }
    io_args.text = Some(arg.to_owned());
    Ok(())
}

fn execute(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    make_shaper: &dyn Fn(Locale) -> Shaper,
) -> Result<i32, Failure> {
    let (locale, command) = parse(args)?;
    let shaper = match command {
        Command::Help => {
            stdout.write_all(HELP.as_bytes())?;
            return Ok(0);
        }
        Command::Version => {
            writeln!(stdout, "mongol-norm {}", crate::version())?;
            return Ok(0);
        }
        _ => make_shaper(locale),
    };
    match command {
        Command::Same(a, b) => {
            let same = shaper.same_shape(&a, &b)?;
            writeln!(stdout, "{}", if same { "true" } else { "false" })?;
            Ok(if same { 0 } else { 1 })
        }
        Command::Shape(io_args) => run_op(&io_args, stdin, stdout, |text| shaper.shape_str(text)),
        Command::Normalize(io_args) => {
            let allow_fallback = io_args.allow_fallback;
            run_op(&io_args, stdin, stdout, |text| {
                if allow_fallback {
                    shaper.normalize_allow_fallback(text)
                } else {
                    shaper.normalize(text)
                }
            })
        }
        Command::NormalizeWrittenUnits(io_args) => run_op(&io_args, stdin, stdout, |text| {
            let units = shaper.parse_written_units(text)?;
            shaper.normalize_written_units(&units)
        }),
        Command::NormalizeText(io_args) => {
            let allow_fallback = io_args.allow_fallback;
            run_op(&io_args, stdin, stdout, |text| {
                if allow_fallback {
                    shaper.normalize_text_allow_fallback(text)
                } else {
                    shaper.normalize_text(text)
                }
            })
        }
        Command::Help | Command::Version => unreachable!("handled above"),
    }
}

fn run_op(
    io_args: &IoArgs,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    op: impl Fn(&str) -> Result<String, Error>,
) -> Result<i32, Failure> {
    let text = read_input(io_args, stdin)?;
    let result = if io_args.batch {
        process_batch(&text, &op)?
    } else {
        op(&text)?
    };
    write_output(&result, io_args.output.as_deref(), stdout)?;
    Ok(0)
}

/// Python `_read_input`: `-i FILE` wins; else the positional text, with `-` (or no text) = stdin.
fn read_input(io_args: &IoArgs, stdin: &mut dyn Read) -> Result<String, Failure> {
    if let Some(path) = &io_args.input {
        return std::fs::read_to_string(path)
            .map_err(|error| Failure::Operation(format!("cannot read {path}: {error}")));
    }
    match io_args.text.as_deref() {
        None | Some("-") => {
            let mut text = String::new();
            stdin
                .read_to_string(&mut text)
                .map_err(|error| Failure::Operation(format!("cannot read stdin: {error}")))?;
            Ok(text)
        }
        Some(text) => Ok(text.to_owned()),
    }
}

/// Python `_write_output`: exact bytes to `-o FILE`; on stdout a trailing newline is added when
/// missing.
fn write_output(text: &str, output: Option<&str>, stdout: &mut dyn Write) -> Result<(), Failure> {
    if let Some(path) = output {
        return std::fs::write(path, text)
            .map_err(|error| Failure::Operation(format!("cannot write {path}: {error}")));
    }
    stdout.write_all(text.as_bytes())?;
    if !text.ends_with('\n') {
        stdout.write_all(b"\n")?;
    }
    Ok(())
}

/// Python `_process_batch`: one result per input line (split like `str.splitlines()`), joined
/// with `\n`; errors carry the line number; a trailing newline of the input is preserved.
fn process_batch(
    lines: &str,
    op: &dyn Fn(&str) -> Result<String, Error>,
) -> Result<String, Failure> {
    let mut out = Vec::new();
    for (number, line) in split_lines_like_python(lines).enumerate() {
        match op(line) {
            Ok(result) => out.push(result),
            Err(error) => return Err(Failure::Operation(format!("line {}: {error}", number + 1))),
        }
    }
    let mut result = out.join("\n");
    if lines.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

/// Every character Python's `str.splitlines()` treats as a line boundary (`\r\n` counts once).
const LINE_BOUNDARIES: [char; 10] = [
    '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// Split `text` the way Python's `str.splitlines()` does — `std::str::lines` would keep `\r`,
/// `\x0b`, U+2028 … inside a line and so disagree with `_process_batch` on the line count.
fn split_lines_like_python(text: &str) -> SplitLines<'_> {
    SplitLines { rest: text }
}

struct SplitLines<'a> {
    rest: &'a str,
}

impl<'a> Iterator for SplitLines<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        let Some(offset) = self.rest.find(LINE_BOUNDARIES) else {
            let line = self.rest;
            self.rest = "";
            return Some(line);
        };
        let (line, tail) = self.rest.split_at(offset);
        let boundary = tail.chars().next().expect("`find` located a boundary");
        let mut tail = &tail[boundary.len_utf8()..];
        if boundary == '\r' {
            tail = tail.strip_prefix('\n').unwrap_or(tail);
        }
        self.rest = tail;
        Some(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAIN: &str = "\u{1830}\u{1820}\u{1822}\u{1828}";
    const MONGGOL: &str = "\u{182E}\u{1823}\u{1829}\u{182D}\u{1823}\u{182F}";

    /// Python `test_cli.py` monkeypatches an empty normalize table into the CLI's shaper.
    fn run_empty_table(args: &[&str], stdin: &str) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(
            &args,
            &mut stdin.as_bytes(),
            &mut out,
            &mut err,
            Shaper::with_empty_normalize_table,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn run_real(args: &[&str], stdin: &str) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &mut stdin.as_bytes(), &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn strict_fallback_prints_error_and_exits_nonzero() {
        let (code, _, stderr) = run_empty_table(&["normalize", SAIN], "");
        assert_eq!(code, 2);
        assert!(stderr.contains("normalization fallback"), "{stderr}");
    }

    #[test]
    fn strict_normalize_text_reports_the_failing_word() {
        let (code, _, stderr) =
            run_empty_table(&["normalize-text", &format!("Hello {SAIN} world")], "");
        assert_eq!(code, 2);
        assert!(stderr.contains("normalization fallback"), "{stderr}");
    }

    #[test]
    fn strict_batch_reports_the_failing_line() {
        let cases = [
            ("normalize", format!("{SAIN}\n{MONGGOL}"), "line 1"),
            (
                "normalize-text",
                format!("English only\nHello {SAIN}"),
                "line 2",
            ),
        ];
        for (command, text, expected_line) in cases {
            let (code, _, stderr) = run_empty_table(&[command, "--batch", &text], "");
            assert_eq!(code, 2, "{command}");
            assert!(
                stderr.contains(&format!("{expected_line}: normalization fallback")),
                "{command}: {stderr}"
            );
        }
    }

    #[test]
    fn allow_fallback_returns_uncovered_input() {
        for (command, text) in [
            ("normalize", SAIN.to_owned()),
            ("normalize-text", format!("Hello {SAIN} world")),
        ] {
            let (code, stdout, stderr) = run_empty_table(&[command, "--allow-fallback", &text], "");
            assert_eq!(code, 0, "{command}: {stderr}");
            assert_eq!(stdout, format!("{text}\n"));
        }
    }

    #[test]
    fn batch_preserves_line_structure_and_trailing_newline() {
        let (code, stdout, _) = run_real(&["shape", "--batch", "-"], &format!("{SAIN}\n{SAIN}"));
        assert_eq!(code, 0);
        assert_eq!(stdout, "S+A+I+I+A\nS+A+I+I+A\n");
        let (code, stdout, _) = run_real(&["shape", "--batch", "-"], &format!("{SAIN}\n{SAIN}\n"));
        assert_eq!(code, 0);
        assert_eq!(stdout, "S+A+I+I+A\nS+A+I+I+A\n");
        let (code, stdout, _) = run_real(&["shape", "--batch", "-"], "");
        assert_eq!((code, stdout.as_str()), (0, "\n"));
    }

    /// Python: `"a\rb\u{2028}c".splitlines() == ["a", "b", "c"]`, and `\r\n` is one boundary.
    #[test]
    fn split_lines_matches_python_splitlines() {
        let cases: [(&str, &[&str]); 8] = [
            ("", &[]),
            ("a", &["a"]),
            ("a\n", &["a"]),
            ("a\n\n", &["a", ""]),
            ("a\r\nb", &["a", "b"]),
            ("a\rb", &["a", "b"]),
            ("a\u{2028}b\u{2029}c\u{85}d", &["a", "b", "c", "d"]),
            (
                "a\u{b}b\u{c}c\u{1c}d\u{1d}e\u{1e}f",
                &["a", "b", "c", "d", "e", "f"],
            ),
        ];
        for (text, expected) in cases {
            let lines: Vec<&str> = split_lines_like_python(text).collect();
            assert_eq!(lines, expected, "{text:?}");
        }
    }

    #[test]
    fn argument_errors_are_usage_errors() {
        for args in [
            &["--bogus"][..],
            &["shape", "--bogus"],
            &["same", "a"],
            &["same", "--batch", "a"],
            &["shape", "a", "b"],
            &["shape", "--", "a", "b"],
            &["--locale"],
            &["--locale=XX", "shape", "a"],
            // CMD is resolved before `-h`, so an unknown command is an invalid choice.
            &["bogus", "--help"],
        ] {
            let (code, _, stderr) = run_real(args, "");
            assert_eq!(code, 2, "{args:?}");
            assert!(stderr.starts_with("usage:"), "{args:?}: {stderr}");
        }
        let (code, _, stderr) = run_real(
            &["shape", "-i", "/definitely/missing/mongol-norm-input.txt"],
            "",
        );
        assert_eq!(code, 2);
        assert!(stderr.starts_with("error: cannot read"), "{stderr}");
    }

    #[test]
    fn double_dash_ends_option_parsing() {
        let (code, stdout, stderr) = run_real(&["normalize-text", "--", "--batch"], "");
        assert_eq!((code, stdout.as_str()), (0, "--batch\n"), "{stderr}");
        // `-a` is taken as TEXT1, so this fails while shaping (runtime), not while parsing.
        let (code, _, stderr) = run_real(&["same", "--", "-a", "-a"], "");
        assert_eq!(code, 2);
        assert!(stderr.starts_with("error: non-Mongolian"), "{stderr}");
    }
}
