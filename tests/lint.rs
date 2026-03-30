use std::process::{Command, Stdio};

use ravel::linter::{LintStatus, check_paths};
use tempfile::tempdir;

#[test]
fn lint_check_reports_clean_status_for_parseable_files() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("ok.R");
    std::fs::write(&path, "x <- 1 + 2\n").expect("failed to write file");

    let result = check_paths(std::slice::from_ref(&path)).expect("lint check should succeed");
    assert_eq!(result.checked_files, 1);
    assert_eq!(result.total_findings, 0);
    assert_eq!(result.reports.len(), 1);
    assert_eq!(result.reports[0].path, path);
    assert_eq!(result.reports[0].status, LintStatus::Clean);
    assert!(result.reports[0].diagnostics.is_empty());
}

#[test]
fn lint_check_reports_assignment_spacing_findings() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("bad.R");
    std::fs::write(&path, "x<-1\nx  <-1\nx<- 1\n").expect("failed to write file");

    let result = check_paths(std::slice::from_ref(&path)).expect("lint check should succeed");
    assert_eq!(result.checked_files, 1);
    assert_eq!(result.total_findings, 3);
    assert_eq!(result.reports.len(), 1);
    assert_eq!(result.reports[0].path, path);
    assert_eq!(result.reports[0].diagnostics.len(), 3);
    assert_eq!(
        result.reports[0].diagnostics[0].rule_id,
        "assignment-spacing"
    );
    match result.reports[0].status {
        LintStatus::Findings { count } => assert_eq!(count, 3),
        _ => panic!("expected findings status"),
    }
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
        _ => panic!("expected parse diagnostics status"),
    }
}

#[test]
fn cli_lint_check_passes_when_no_findings() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("ok.R");
    std::fs::write(&path, "x <- 1 + 2\n").expect("failed to write file");

    let output = run_cli([
        "lint",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);

    assert!(output.status.success());
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.is_empty());
}

#[test]
fn cli_lint_check_reports_assignment_spacing_findings() {
    let dir = tempdir().expect("failed to create temp dir");
    let path = dir.path().join("bad.R");
    std::fs::write(&path, "x<-1\nx  <-1\nx<- 1\n").expect("failed to write file");

    let output = run_cli([
        "lint",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad.R:1:2: [assignment-spacing]"));
    assert!(stderr.contains("bad.R:2:4: [assignment-spacing]"));
    assert!(stderr.contains("bad.R:3:2: [assignment-spacing]"));
    assert!(stderr.contains("must be surrounded by single spaces"));
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
    assert_eq!(stderr.trim(), "error: lint currently requires --check");
}

#[test]
fn cli_lint_check_requires_paths() {
    let output = run_cli(["lint", "--check"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--check requires at least one input path"));
}

#[test]
fn cli_lint_check_reports_multiple_files_deterministically() {
    let dir = tempdir().expect("failed to create temp dir");
    let good = dir.path().join("a_ok.R");
    let bad = dir.path().join("b_bad.R");
    std::fs::write(&good, "x <- 1\n").expect("failed to write good file");
    std::fs::write(&bad, "x<-1\n").expect("failed to write bad file");

    let output = run_cli([
        "lint",
        "--check",
        dir.path().to_str().expect("temp dir path should be utf-8"),
    ]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("a_ok.R"));
    assert!(stderr.contains("b_bad.R:1:2: [assignment-spacing]"));
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
