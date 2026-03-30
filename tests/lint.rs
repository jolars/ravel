use std::process::{Command, Stdio};

use ravel::lint_check::{LintStatus, check_paths};
use tempfile::tempdir;

#[test]
fn lint_check_reports_rules_not_implemented_for_parseable_files() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("ok.R");
    std::fs::write(&path, "x <- 1 + 2\n").expect("failed to write file");

    let result = check_paths(std::slice::from_ref(&path)).expect("lint check should succeed");
    assert_eq!(result.checked_files, 1);
    assert_eq!(result.reports.len(), 1);
    assert_eq!(result.reports[0].path, path);
    assert_eq!(result.reports[0].status, LintStatus::RulesNotImplemented);
}

#[test]
fn lint_check_reports_parse_diagnostics_pathway() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("bad.R");
    std::fs::write(&path, "x <-\n").expect("failed to write file");

    let result = check_paths(std::slice::from_ref(&path)).expect("lint check should succeed");
    assert_eq!(result.checked_files, 1);
    assert_eq!(result.reports.len(), 1);
    assert_eq!(result.reports[0].path, path);
    match result.reports[0].status {
        LintStatus::ParseDiagnostics { count } => assert!(count > 0),
        LintStatus::RulesNotImplemented => panic!("expected parse diagnostics status"),
    }
}

#[test]
fn cli_lint_check_reports_not_implemented_explicitly() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("ok.R");
    std::fs::write(&path, "x <- 1 + 2\n").expect("failed to write file");

    let output = run_cli([
        "lint",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lint not yet implemented:"));
    assert!(stderr.contains("ok.R"));
}

#[test]
fn cli_lint_check_reports_parse_diagnostics_pathway() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("bad.R");
    std::fs::write(&path, "x <-\n").expect("failed to write file");

    let output = run_cli([
        "lint",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lint blocked by parse diagnostics:"));
    assert!(stderr.contains("bad.R"));
}

#[test]
fn cli_lint_requires_check_flag() {
    let output = run_cli(["lint"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lint currently requires --check"));
}

#[test]
fn cli_lint_check_requires_paths() {
    let output = run_cli(["lint", "--check"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--check requires at least one input path"));
}

fn run_cli<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ravel"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli")
}
