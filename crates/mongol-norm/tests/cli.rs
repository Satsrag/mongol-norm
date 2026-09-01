//! Command-line tests: every test spawns the real `mongol-norm` binary. Port of
//! `tests/test_written_units_api.py::TestNormalizeWrittenUnitsCli` (+ the CLI check of the
//! positioned API tests and smoke tests of the remaining subcommands).

mod common;

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::mgl;
use mongol_norm::{Locale, Shaper, WrittenUnit};

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn spawn<I, S>(args: I, stdin: Option<&str>) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(env!("CARGO_BIN_EXE_mongol-norm"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mongol-norm");
    {
        let mut input = child.stdin.take().unwrap();
        if let Some(text) = stdin {
            // A child that fails before reading stdin closes the pipe: EPIPE is not a test failure.
            let _ = input.write_all(text.as_bytes());
        }
    }
    let output = child.wait_with_output().unwrap();
    Output {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn run(args: &[&str], stdin: Option<&str>) -> Output {
    spawn(args, stdin)
}

fn shaper() -> Shaper {
    Shaper::new(Locale::Mng)
}

/// A scratch directory for the file-I/O tests, removed when the test ends (pass or panic).
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> TempDir {
        let path =
            std::env::temp_dir().join(format!("mongol-norm-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }

    fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("temp paths are UTF-8")
}

fn assert_clean_failure(output: &Output, needle: &str) {
    assert_eq!(output.code, 2, "stderr: {}", output.stderr);
    assert!(
        output.stderr.contains(needle),
        "stderr {:?} lacks {needle:?}",
        output.stderr
    );
    assert!(!output.stderr.contains("panicked"), "{}", output.stderr);
}

#[test]
fn test_missing_subcommand_fails_cleanly() {
    let output = run(&[], None);
    assert_clean_failure(&output, "CMD");
    // The usage line advertises `-V`, which the Python CLI does not have.
    assert!(output.stderr.contains("[-V]"), "{}", output.stderr);
}

#[test]
fn test_invalid_choice_quotes_the_choices() {
    // argparse ≤ 3.13 quotes each choice; 3.14 prints them bare. We pin the ≤ 3.13 spelling.
    let output = run(&["bogus", "a"], None);
    assert_clean_failure(&output, "invalid choice: 'bogus'");
    assert!(
        output.stderr.contains("choose from 'shape'"),
        "{}",
        output.stderr
    );
}

#[test]
fn test_help_after_an_unknown_command_is_an_invalid_choice() {
    // argparse resolves CMD before a sub-parser can act on `-h`, so this is not a help request.
    for args in [&["bogus", "--help"][..], &["bogus", "-h"]] {
        let output = run(args, None);
        assert_clean_failure(&output, "invalid choice: 'bogus'");
        assert!(output.stdout.is_empty(), "{args:?}: {}", output.stdout);
    }
}

#[test]
fn test_help_after_a_known_command_prints_the_global_help() {
    // Python prints the sub-parser's help here; this CLI has one help text.
    for args in [&["shape", "--help"][..], &["same", "-h"]] {
        let output = run(args, None);
        assert_eq!(output.code, 0, "{args:?}: {}", output.stderr);
        assert!(
            output.stdout.contains("normalize-written-units"),
            "{args:?}: {}",
            output.stdout
        );
    }
}

#[test]
fn test_positioned_records_have_no_cli_subcommand() {
    let output = run(&["normalize-positioned-written-units"], None);
    assert_clean_failure(&output, "invalid choice");
}

#[test]
fn test_shape_cli_pascal_case_output_pipes_back() {
    let shaped = run(&["shape", "\u{182A}\u{200D}"], None);
    assert_eq!(shaped.code, 0, "{}", shaped.stderr);
    assert_eq!(shaped.stdout, "B+Zwj\n");
    let normalized = run(
        &[
            "normalize-written-units",
            shaped.stdout.trim_end_matches('\n'),
        ],
        None,
    );
    assert_eq!(normalized.code, 0, "{}", normalized.stderr);
    assert_eq!(
        shaper()
            .shape(normalized.stdout.trim_end_matches('\n'))
            .unwrap(),
        [WrittenUnit::B, WrittenUnit::Zwj]
    );
}

#[test]
fn test_compact_pascal_case_units() {
    let output = run(&["normalize-written-units", "BZwj"], None);
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(
        shaper()
            .shape(output.stdout.trim_end_matches('\n'))
            .unwrap(),
        [WrittenUnit::B, WrittenUnit::Zwj]
    );
}

#[test]
fn test_compact_units_are_segmented_before_shape_validation() {
    let output = run(&["normalize-written-units", "AAaBZwj"], None);
    assert_clean_failure(&output, "no canonical MNG encoding");
    assert!(!output.stderr.contains("is unknown: 'AAaBZwj'"));
}

#[test]
fn test_all_pascal_case_control_spellings() {
    let shaper = shaper();
    let cases: [(&[WrittenUnit], &str); 3] = [
        (&[WrittenUnit::Mvs, WrittenUnit::Aa], "Mvs+Aa"),
        (&[WrittenUnit::Nirugu, WrittenUnit::U], "Nirugu+U"),
        (&[WrittenUnit::Zwj, WrittenUnit::Dd], "Zwj+Dd"),
    ];
    for (units, spelled) in cases {
        let expected = shaper.normalize_written_units(units).unwrap();
        let output = run(&["normalize-written-units", spelled], None);
        assert_eq!(output.code, 0, "{}", output.stderr);
        assert_eq!(output.stdout, format!("{expected}\n"));
    }
}

#[test]
fn test_lowercase_control_spellings_are_rejected() {
    for control in ["mvs", "nirugu", "zwj"] {
        assert_clean_failure(
            &run(&["normalize-written-units", control], None),
            "is unknown",
        );
    }
}

#[test]
fn test_canonical_control_capitalization() {
    let units = [
        WrittenUnit::T,
        WrittenUnit::A,
        WrittenUnit::L,
        WrittenUnit::Mvs,
        WrittenUnit::Aa,
    ];
    let expected = shaper().normalize_written_units(&units).unwrap();
    let output = run(&["normalize-written-units", "T+A+L+Mvs+Aa"], None);
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, format!("{expected}\n"));
}

#[test]
fn test_inline_plus_delimited_units() {
    let output = run(&["normalize-written-units", "B+Aa"], None);
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "\u{182A}\u{1820}\u{180B}\n");
}

#[test]
fn test_non_batch_stdin_accepts_one_transport_newline() {
    let output = run(&["normalize-written-units", "-"], Some("B+Aa\n"));
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "\u{182A}\u{1820}\u{180B}\n");
}

#[test]
fn test_file_input_and_output() {
    let dir = TempDir::new("file-io");
    let input = dir.join("units.txt");
    let output_path = dir.join("canonical.txt");
    std::fs::write(&input, "B+Aa\n").unwrap();
    let output = run(
        &[
            "normalize-written-units",
            "-i",
            path_str(&input),
            "-o",
            path_str(&output_path),
        ],
        None,
    );
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "");
    assert_eq!(
        std::fs::read_to_string(&output_path).unwrap(),
        "\u{182A}\u{1820}\u{180B}"
    );
}

#[test]
fn test_long_option_equals_forms() {
    let dir = TempDir::new("equals-forms");
    let input = dir.join("units.txt");
    let output_path = dir.join("canonical.txt");
    std::fs::write(&input, "B+Aa\n").unwrap();
    let input_arg = format!("--input={}", path_str(&input));
    let output_arg = format!("--output={}", path_str(&output_path));
    let output = run(&["normalize-written-units", &input_arg, &output_arg], None);
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "");
    assert_eq!(
        std::fs::read_to_string(&output_path).unwrap(),
        "\u{182A}\u{1820}\u{180B}"
    );
}

#[test]
fn test_batch_writes_every_line_to_the_output_file() {
    let dir = TempDir::new("batch-output");
    let output_path = dir.join("canonical.txt");
    let output = run(
        &[
            "normalize-written-units",
            "--batch",
            "-",
            "-o",
            path_str(&output_path),
        ],
        Some("B+Aa\nB+Aa\n"),
    );
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "");
    assert_eq!(
        std::fs::read_to_string(&output_path).unwrap(),
        "\u{182A}\u{1820}\u{180B}\n".repeat(2)
    );
}

#[test]
fn test_unknown_and_unencodable_sequences_fail_cleanly() {
    for (text, message) in [
        ("Unknown", "is unknown"),
        ("O", "no canonical MNG encoding"),
    ] {
        assert_clean_failure(&run(&["normalize-written-units", text], None), message);
    }
}

#[test]
fn test_stdin_batch_processes_one_sequence_per_line() {
    let output = run(
        &["normalize-written-units", "--batch", "-"],
        Some("B+Aa\nB+Aa\n"),
    );
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "\u{182A}\u{1820}\u{180B}\n".repeat(2));
}

#[test]
fn test_surrounding_whitespace_is_rejected() {
    for text in [" B+Aa", "B+Aa ", "\tB+Aa", "B+Aa\t"] {
        assert_clean_failure(
            &run(&["normalize-written-units", text], None),
            "cannot be empty or contain whitespace",
        );
    }
}

#[test]
fn test_internal_whitespace_is_rejected() {
    for text in ["A A", "A A+B"] {
        assert_clean_failure(&run(&["normalize-written-units", text], None), "whitespace");
    }
}

#[test]
fn test_empty_unit_name_fails_without_a_traceback() {
    assert_clean_failure(
        &run(&["normalize-written-units", "B++Aa"], None),
        "cannot be empty or contain whitespace",
    );
}

#[test]
fn test_same_reports_visual_identity_via_exit_code() {
    let same = run(&["same", &mgl("s a i n"), &mgl("s e i n")], None);
    assert_eq!((same.code, same.stdout.as_str()), (0, "true\n"));
    let different = run(&["same", &mgl("s a i n"), &mgl("n a i fvs3 m a")], None);
    assert_eq!((different.code, different.stdout.as_str()), (1, "false\n"));
}

#[test]
fn test_shape_normalize_and_normalize_text_smoke() {
    let canonical = mgl("s a i fvs3 i fvs3 a fvs2");
    let shaped = run(&["shape", &mgl("s a i n")], None);
    assert_eq!((shaped.code, shaped.stdout.as_str()), (0, "S+A+I+I+A\n"));
    let normalized = run(&["normalize", &mgl("s e i n")], None);
    assert_eq!(
        (normalized.code, normalized.stdout.as_str()),
        (0, format!("{canonical}\n").as_str())
    );
    let text = run(
        &["normalize-text", &format!("Hello {} world", mgl("s e i n"))],
        None,
    );
    assert_eq!(
        (text.code, text.stdout.as_str()),
        (0, format!("Hello {canonical} world\n").as_str())
    );
    let batch = run(
        &["normalize", "--batch", "-"],
        Some(&format!("{}\n{}\n", mgl("s e i n"), mgl("s a i n"))),
    );
    assert_eq!(
        (batch.code, batch.stdout.as_str()),
        (0, format!("{canonical}\n{canonical}\n").as_str())
    );
    assert_clean_failure(
        &run(
            &["normalize", &format!("{} {}", mgl("s a i n"), mgl("a"))],
            None,
        ),
        "non-Mongolian character",
    );
}

#[test]
fn test_locale_option_help_and_version() {
    let tod = run(&["--locale", "TOD", "shape", "\u{1820}"], None);
    assert_eq!(tod.code, 0, "{}", tod.stderr);
    assert_clean_failure(
        &run(&["--locale", "XX", "shape", "\u{1820}"], None),
        "unknown locale",
    );
    let help = run(&["--help"], None);
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("normalize-written-units"));
    let version = run(&["--version"], None);
    assert_eq!(
        version.stdout,
        format!("mongol-norm {}\n", mongol_norm::version())
    );
}

#[test]
fn test_locale_equals_form_and_short_version_flag() {
    let equals = run(&["--locale=TOD", "shape", "\u{1820}"], None);
    let spaced = run(&["--locale", "TOD", "shape", "\u{1820}"], None);
    assert_eq!(equals.code, 0, "{}", equals.stderr);
    assert_eq!(equals.stdout, spaced.stdout);
    assert_clean_failure(
        &run(&["--locale=XX", "shape", "\u{1820}"], None),
        "unknown locale",
    );
    let short = run(&["-V"], None);
    assert_eq!(short.code, 0, "{}", short.stderr);
    assert_eq!(short.stdout, run(&["--version"], None).stdout);
}

#[test]
fn test_double_dash_ends_option_parsing() {
    // Without `--` a hyphen-initial TEXT would be an unrecognized option.
    let text = format!("-{}", mgl("s e i n"));
    let canonical = mgl("s a i fvs3 i fvs3 a fvs2");
    let dashed = run(&["normalize-text", "--", &text], None);
    assert_eq!(
        (dashed.code, dashed.stdout.as_str()),
        (0, format!("-{canonical}\n").as_str()),
        "{}",
        dashed.stderr
    );
    assert_clean_failure(
        &run(&["normalize-text", &text], None),
        "unrecognized arguments",
    );
    // `--batch` after `--` is text, not the flag.
    let literal = run(&["normalize-text", "--", "--batch"], None);
    assert_eq!((literal.code, literal.stdout.as_str()), (0, "--batch\n"));
}

#[test]
fn test_same_rejects_options() {
    assert_clean_failure(
        &run(&["same", "--batch", &mgl("s a i n")], None),
        "unrecognized arguments",
    );
}

#[test]
fn test_batch_splits_lines_like_python_splitlines() {
    let expected = "\u{182A}\u{1820}\u{180B}\n";
    // Python `"B+Aa\rB+Aa\n".splitlines()` has two entries; `str::lines` would see one.
    let carriage = run(
        &["normalize-written-units", "--batch", "-"],
        Some("B+Aa\rB+Aa\n"),
    );
    assert_eq!(carriage.code, 0, "{}", carriage.stderr);
    assert_eq!(carriage.stdout, expected.repeat(2));
    let separators = run(
        &["normalize-written-units", "--batch", "-"],
        Some("B+Aa\u{2028}B+Aa\r\nB+Aa\n"),
    );
    assert_eq!(separators.code, 0, "{}", separators.stderr);
    assert_eq!(separators.stdout, expected.repeat(3));
}

#[cfg(unix)]
#[test]
fn test_non_utf8_argument_fails_cleanly() {
    use std::os::unix::ffi::OsStrExt;

    let output = spawn([OsStr::new("shape"), OsStr::from_bytes(b"\xff\xfe")], None);
    assert_clean_failure(&output, "not valid UTF-8");
}
