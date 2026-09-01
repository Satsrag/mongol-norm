//! The `mongol-norm` command line — a port of `mongol_norm/shaper.py::main` and its helpers
//! (`_read_input`, `_write_output`, `_process_batch`, argument handling). Hidden from the docs:
//! it exists so the binary is a shim and the crate's own tests can drive every path
//! in-process, including normalization fallback (which no real input triggers).

use std::io::{self, Read, Write};

use crate::{Error, Locale, Shaper};

const COMMANDS: [&str; 5] = [
    "shape",
    "normalize",
    "normalize-written-units",
    "normalize-text",
    "same",
];

const USAGE: &str = "usage: mongol-norm [-h] [-V] [--locale LOCALE] CMD ...";

const HELP: &str = "\
usage: mongol-norm [-h] [-V] [--locale LOCALE] CMD ...

Mongolian shaping / normalization tool (UTN #57 v4 + GB/T 25914-2023).
蒙古文字形 / 规范化工具 (UTN #57 v4 + GB/T 25914-2023)。

options:
  -h, --help            show this help and exit
  -V, --version         print the version and exit
  --locale LOCALE       MNG (default), TOD, SIB or MCH

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
  file output :  mongol-norm <cmd> -i input.txt -o output.txt
  batch       :  mongol-norm <cmd> --batch -i words.txt -o out.txt
                 (one word/text per line in, one result per line out)
  --allow-fallback (normalize, normalize-text): keep an uncovered word instead of failing

Examples / 示例:
  mongol-norm normalize 'ᠰᠡᠢᠨ'
  mongol-norm shape 'ᠰᠠᠢᠨ'                          # → S+A+I+I+A
  mongol-norm normalize-written-units 'B+Aa'           # → ᠪᠠ᠋
  mongol-norm normalize-text 'Hello ᠰᠡᠢᠨ world'
  echo 'ᠰᠡᠢᠨ' | mongol-norm normalize -
";

/// Run the CLI on the process arguments and standard streams; returns the exit code.
pub fn main() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
pub fn run(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    run_with(args, stdin, stdout, stderr, Shaper::new)
}

/// [`run`] with a custom shaper factory (tests inject a table-less shaper).
pub fn run_with(
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

fn parse(args: &[String]) -> Result<(Locale, Command), Failure> {
    let mut locale = Locale::Mng;
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "-h" | "--help" => return Ok((locale, Command::Help)),
            "-V" | "--version" => return Ok((locale, Command::Version)),
            "--locale" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    Failure::Usage("argument --locale: expected one argument".to_owned())
                })?;
                locale = parse_locale(value)?;
                index += 2;
            }
            _ if arg.starts_with("--locale=") => {
                locale = parse_locale(&arg["--locale=".len()..])?;
                index += 1;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
            }
            _ => break,
        }
    }
    let Some(name) = args.get(index) else {
        return Err(Failure::Usage(
            "the following arguments are required: CMD".to_owned(),
        ));
    };
    let rest = &args[index + 1..];
    if rest.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok((locale, Command::Help));
    }
    let command = match name.as_str() {
        "same" => match rest {
            [a, b] => Command::Same(a.clone(), b.clone()),
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
        other => {
            let choices: Vec<String> = COMMANDS.iter().map(|c| format!("'{c}'")).collect();
            return Err(Failure::Usage(format!(
                "argument CMD: invalid choice: '{other}' (choose from {})",
                choices.join(", ")
            )));
        }
    };
    Ok((locale, command))
}

fn parse_io(name: &str, rest: &[String]) -> Result<IoArgs, Failure> {
    let mut io = IoArgs {
        text: None,
        input: None,
        output: None,
        batch: false,
        allow_fallback: false,
    };
    let mut index = 0;
    while index < rest.len() {
        let arg = rest[index].as_str();
        match arg {
            "-i" | "--input" | "-o" | "--output" => {
                let value = rest
                    .get(index + 1)
                    .ok_or_else(|| {
                        Failure::Usage(format!("argument {arg}: expected one argument"))
                    })?
                    .clone();
                if arg == "-i" || arg == "--input" {
                    io.input = Some(value);
                } else {
                    io.output = Some(value);
                }
                index += 2;
            }
            "--batch" => {
                io.batch = true;
                index += 1;
            }
            "--allow-fallback" if matches!(name, "normalize" | "normalize-text") => {
                io.allow_fallback = true;
                index += 1;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
            }
            _ => {
                if io.text.is_some() {
                    return Err(Failure::Usage(format!("unrecognized arguments: {arg}")));
                }
                io.text = Some(arg.to_owned());
                index += 1;
            }
        }
    }
    Ok(io)
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
        Command::Shape(io) => run_op(&io, stdin, stdout, |text| shaper.shape_str(text)),
        Command::Normalize(io) => {
            let allow_fallback = io.allow_fallback;
            run_op(&io, stdin, stdout, |text| {
                if allow_fallback {
                    shaper.normalize_allow_fallback(text)
                } else {
                    shaper.normalize(text)
                }
            })
        }
        Command::NormalizeWrittenUnits(io) => run_op(&io, stdin, stdout, |text| {
            let units = shaper.parse_written_units(text)?;
            shaper.normalize_written_units(&units)
        }),
        Command::NormalizeText(io) => {
            let allow_fallback = io.allow_fallback;
            run_op(&io, stdin, stdout, |text| {
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
    io: &IoArgs,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    op: impl Fn(&str) -> Result<String, Error>,
) -> Result<i32, Failure> {
    let text = read_input(io, stdin)?;
    let result = if io.batch {
        process_batch(&text, &op)?
    } else {
        op(&text)?
    };
    write_output(&result, io.output.as_deref(), stdout)?;
    Ok(0)
}

/// Python `_read_input`: `-i FILE` wins; else the positional text, with `-` (or no text) = stdin.
fn read_input(io: &IoArgs, stdin: &mut dyn Read) -> Result<String, Failure> {
    if let Some(path) = &io.input {
        return std::fs::read_to_string(path)
            .map_err(|error| Failure::Operation(format!("cannot read {path}: {error}")));
    }
    match io.text.as_deref() {
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

/// Python `_process_batch`: one result per input line; errors carry the line number; the
/// trailing newline of the input is preserved.
fn process_batch(
    lines: &str,
    op: &dyn Fn(&str) -> Result<String, Error>,
) -> Result<String, Failure> {
    let mut out = Vec::new();
    for (number, line) in lines.lines().enumerate() {
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

    #[test]
    fn argument_errors_are_usage_errors() {
        for args in [
            &["--bogus"][..],
            &["shape", "--bogus"],
            &["same", "a"],
            &["shape", "a", "b"],
            &["--locale"],
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
}
