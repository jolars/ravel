use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::file_discovery::{FileDiscoveryError, collect_r_files};
use crate::incremental::{IncrementalDatabase, SourceFile};
use crate::parser::lexer::{TokKind, Token, lex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintStatus {
    Clean,
    Findings { count: usize },
    ParseDiagnostics { count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub path: PathBuf,
    pub rule_id: &'static str,
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFileReport {
    pub path: PathBuf,
    pub status: LintStatus,
    pub diagnostics: Vec<LintDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintResult {
    pub checked_files: usize,
    pub total_findings: usize,
    pub reports: Vec<LintFileReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintError {
    MissingPaths,
    NoRFiles,
    NonRFilePath { path: PathBuf },
    WalkError { path: PathBuf, message: String },
    ReadError { path: PathBuf, source: String },
}

impl fmt::Display for LintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPaths => {
                write!(
                    f,
                    "--check requires at least one input path (file or directory)"
                )
            }
            Self::NoRFiles => {
                write!(f, "no .R files found under the provided input paths")
            }
            Self::NonRFilePath { path } => {
                write!(
                    f,
                    "input file {} is not an .R file; --check only supports .R files",
                    path.display()
                )
            }
            Self::WalkError { path, message } => {
                write!(f, "failed while scanning {}: {message}", path.display())
            }
            Self::ReadError { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for LintError {}

impl From<FileDiscoveryError> for LintError {
    fn from(value: FileDiscoveryError) -> Self {
        match value {
            FileDiscoveryError::NonRFilePath { path } => Self::NonRFilePath { path },
            FileDiscoveryError::WalkError { path, message } => Self::WalkError { path, message },
        }
    }
}

pub fn check_paths(paths: &[PathBuf]) -> Result<LintResult, LintError> {
    if paths.is_empty() {
        return Err(LintError::MissingPaths);
    }

    let files = collect_r_files(paths).map_err(LintError::from)?;
    if files.is_empty() {
        return Err(LintError::NoRFiles);
    }

    let mut db = IncrementalDatabase::default();
    let mut tracked: HashMap<PathBuf, SourceFile> = HashMap::new();
    let mut reports = Vec::new();
    let mut total_findings = 0usize;

    for path in files {
        let content = fs::read_to_string(&path).map_err(|err| LintError::ReadError {
            path: path.clone(),
            source: err.to_string(),
        })?;

        let file = match tracked.get(&path).copied() {
            Some(file) => {
                db.set_file_text(file, content.clone());
                file
            }
            None => {
                let file = db.add_file(content.clone());
                tracked.insert(path.clone(), file);
                file
            }
        };

        let parsed = db.parse(file);
        let (status, diagnostics) = if parsed.diagnostics.is_empty() {
            let diagnostics = run_rules(&path, &content);
            total_findings += diagnostics.len();
            let status = if diagnostics.is_empty() {
                LintStatus::Clean
            } else {
                LintStatus::Findings {
                    count: diagnostics.len(),
                }
            };
            (status, diagnostics)
        } else {
            (
                LintStatus::ParseDiagnostics {
                    count: parsed.diagnostics.len(),
                },
                Vec::new(),
            )
        };

        reports.push(LintFileReport {
            path,
            status,
            diagnostics,
        });
    }

    Ok(LintResult {
        checked_files: tracked.len(),
        total_findings,
        reports,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleDiagnostic {
    rule_id: &'static str,
    message: String,
    start: usize,
    end: usize,
}

struct LintContext<'a> {
    tokens: &'a [Token],
}

type RuleFn = fn(&LintContext<'_>, &mut Vec<RuleDiagnostic>);

struct Rule {
    id: &'static str,
    run: RuleFn,
}

const RULES: &[Rule] = &[Rule {
    id: "assignment-spacing",
    run: check_assignment_spacing,
}];
const ASSIGNMENT_SPACING_RULE_ID: &str = "assignment-spacing";

fn run_rules(path: &Path, source: &str) -> Vec<LintDiagnostic> {
    let tokens = lex(source);
    let ctx = LintContext { tokens: &tokens };
    let mut raw = Vec::new();
    for rule in RULES {
        debug_assert!(!rule.id.is_empty());
        (rule.run)(&ctx, &mut raw);
    }
    raw.sort_by(|a, b| {
        (a.start, a.end, a.rule_id, a.message.as_str()).cmp(&(
            b.start,
            b.end,
            b.rule_id,
            b.message.as_str(),
        ))
    });

    raw.into_iter()
        .map(|diag| {
            let (line, column) = byte_offset_to_line_col(source, diag.start);
            LintDiagnostic {
                path: path.to_path_buf(),
                rule_id: diag.rule_id,
                message: diag.message,
                start: diag.start,
                end: diag.end,
                line,
                column,
            }
        })
        .collect()
}

fn check_assignment_spacing(ctx: &LintContext<'_>, out: &mut Vec<RuleDiagnostic>) {
    for (idx, token) in ctx.tokens.iter().enumerate() {
        if token.kind != TokKind::AssignLeft {
            continue;
        }

        let left_ok = idx > 0
            && ctx.tokens[idx - 1].kind == TokKind::Whitespace
            && ctx.tokens[idx - 1].text == " ";
        let right_ok = idx + 1 < ctx.tokens.len()
            && ctx.tokens[idx + 1].kind == TokKind::Whitespace
            && ctx.tokens[idx + 1].text == " ";

        if left_ok && right_ok {
            continue;
        }

        out.push(RuleDiagnostic {
            rule_id: ASSIGNMENT_SPACING_RULE_ID,
            message: "assignment operator `<-` must be surrounded by single spaces".to_string(),
            start: token.start,
            end: token.end,
        });
    }
}

fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    let clamped = offset.min(source.len());
    for ch in source[..clamped].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::{LintStatus, check_paths};
    use tempfile::tempdir;

    #[test]
    fn detects_assignment_spacing_violations() {
        let dir = tempdir().expect("failed to create temp dir");
        let path = dir.path().join("bad.R");
        std::fs::write(&path, "x<-1\nx  <-1\nx<- 1\n").expect("failed to write file");

        let result = check_paths(std::slice::from_ref(&path)).expect("lint should run");
        assert_eq!(result.total_findings, 3);
        assert_eq!(result.reports[0].diagnostics.len(), 3);
        assert_eq!(
            result.reports[0].diagnostics[0].rule_id,
            "assignment-spacing"
        );
        assert_eq!(result.reports[0].diagnostics[0].line, 1);
        assert_eq!(result.reports[0].diagnostics[1].line, 2);
        assert_eq!(result.reports[0].diagnostics[2].line, 3);
    }

    #[test]
    fn passes_when_assignment_spacing_is_correct() {
        let dir = tempdir().expect("failed to create temp dir");
        let path = dir.path().join("ok.R");
        std::fs::write(&path, "x <- 1\ny <- x + 1\n").expect("failed to write file");

        let result = check_paths(std::slice::from_ref(&path)).expect("lint should run");
        assert_eq!(result.total_findings, 0);
        assert_eq!(result.reports[0].diagnostics.len(), 0);
        assert_eq!(result.reports[0].status, LintStatus::Clean);
    }
}
