use std::io::Write;
use std::process::{Command, Stdio};

use ravel::formatter::{FormatError, format};

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
    assert!(stderr.contains("unsupported construct for formatter v1"));
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
