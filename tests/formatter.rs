use std::io::Write;
use std::process::{Command, Stdio};

use ravel::formatter::{FormatError, format};
use tempfile::tempdir;

#[test]
fn formats_assignment_binary_and_paren_stably() {
    let input = "x<-(1+2)*3^4\n";
    let expected = "x <- (1 + 2) * 3 ^ 4\n";
    let formatted = format(input).expect("should format input");
    assert_eq!(formatted, expected);
    let reformatted = format(&formatted).expect("should remain formatable");
    assert_eq!(reformatted, expected);
}

#[test]
fn formats_if_else_blocks_with_comments_and_strings() {
    let input = "if(x){# keep\nmsg<-'a+b'\n}else{y<-1+2}\n";
    let expected = "if (x) {\n  # keep\n  msg <- 'a+b'\n} else {\n  y <- 1 + 2\n}\n";
    let formatted = format(input).expect("should format if/else blocks");
    assert_eq!(formatted, expected);
}

#[test]
fn preserves_comment_only_lines() {
    let input = "x<-1\n# untouched\n";
    let expected = "x <- 1\n# untouched\n";
    let formatted = format(input).expect("should format and preserve comments");
    assert_eq!(formatted, expected);
}

#[test]
fn rejects_unsupported_constructs_explicitly() {
    let err = format("x %foo% y\n").expect_err("user operators are unsupported currently");
    assert!(matches!(err, FormatError::UnsupportedConstruct { .. }));
}

#[test]
fn cli_format_verify_formats_stdin() {
    let output = run_cli(["format", "--verify"], "if(x){y<-1+2}else{z<-3}\n");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "if (x) {\n  y <- 1 + 2\n} else {\n  z <- 3\n}\n"
    );
}

#[test]
fn cli_format_reports_unsupported_constructs() {
    let output = run_cli(["format"], "x %foo% y\n");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported construct for formatter"));
}

#[test]
fn cli_format_check_reports_changed_files() {
    let dir = tempdir().expect("failed to create temp dir");
    let changed = dir.path().join("changed.R");
    let unchanged = dir.path().join("unchanged.R");

    std::fs::write(&changed, "x<-1+2\n").expect("failed to write changed file");
    std::fs::write(&unchanged, "x <- 1 + 2\n").expect("failed to write unchanged file");

    let output = run_cli_no_stdin([
        "format",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("would reformat:"));
    assert!(stderr.contains("changed.R"));
    assert!(!stderr.contains("unchanged.R"));
}

#[test]
fn cli_format_check_succeeds_for_unchanged_files() {
    let dir = tempdir().expect("failed to create temp dir");
    let unchanged = dir.path().join("unchanged.R");
    std::fs::write(&unchanged, "x <- 1 + 2\n").expect("failed to write unchanged file");

    let output = run_cli_no_stdin([
        "format",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_format_check_requires_paths() {
    let output = run_cli_no_stdin(["format", "--check"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--check requires at least one input path"));
}

#[test]
fn cli_format_check_disallows_verify() {
    let output = run_cli_no_stdin(["format", "--check", "--verify", "."]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--verify cannot be combined with --check"));
}

fn run_cli<const N: usize>(args: [&str; N], stdin_input: &str) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ravel"));
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn ravel cli");
    let mut stdin = child.stdin.take().expect("failed to open stdin");
    stdin
        .write_all(stdin_input.as_bytes())
        .expect("failed to write stdin");
    drop(stdin);

    child.wait_with_output().expect("failed to wait for cli")
}

fn run_cli_no_stdin<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ravel"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli")
}
