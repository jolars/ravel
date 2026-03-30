use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::file_discovery::{FileDiscoveryError, collect_r_files};
use crate::incremental::{IncrementalDatabase, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintStatus {
    RulesNotImplemented,
    ParseDiagnostics { count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFileReport {
    pub path: PathBuf,
    pub status: LintStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintResult {
    pub checked_files: usize,
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

    for path in files {
        let content = fs::read_to_string(&path).map_err(|err| LintError::ReadError {
            path: path.clone(),
            source: err.to_string(),
        })?;

        let file = match tracked.get(&path).copied() {
            Some(file) => {
                db.set_file_text(file, content);
                file
            }
            None => {
                let file = db.add_file(content);
                tracked.insert(path.clone(), file);
                file
            }
        };

        let parsed = db.parse(file);
        let status = if parsed.diagnostics.is_empty() {
            LintStatus::RulesNotImplemented
        } else {
            LintStatus::ParseDiagnostics {
                count: parsed.diagnostics.len(),
            }
        };

        reports.push(LintFileReport { path, status });
    }

    Ok(LintResult {
        checked_files: tracked.len(),
        reports,
    })
}
