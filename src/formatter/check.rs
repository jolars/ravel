use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use super::{FormatError, format};
use crate::file_discovery::{FileDiscoveryError, collect_r_files};
use crate::incremental::{IncrementalDatabase, SourceFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub checked_files: usize,
    pub changed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    MissingPaths,
    NoRFiles,
    NonRFilePath { path: PathBuf },
    WalkError { path: PathBuf, message: String },
    ReadError { path: PathBuf, source: String },
    FormatError { path: PathBuf, source: FormatError },
}

impl fmt::Display for CheckError {
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
            Self::FormatError { path, source } => {
                write!(f, "failed to format {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for CheckError {}

impl From<FileDiscoveryError> for CheckError {
    fn from(value: FileDiscoveryError) -> Self {
        match value {
            FileDiscoveryError::NonRFilePath { path } => Self::NonRFilePath { path },
            FileDiscoveryError::WalkError { path, message } => Self::WalkError { path, message },
        }
    }
}

pub fn check_paths(paths: &[PathBuf]) -> Result<CheckResult, CheckError> {
    if paths.is_empty() {
        return Err(CheckError::MissingPaths);
    }

    let files = collect_r_files(paths).map_err(CheckError::from)?;
    if files.is_empty() {
        return Err(CheckError::NoRFiles);
    }

    let mut db = IncrementalDatabase::default();
    let mut tracked: HashMap<PathBuf, SourceFile> = HashMap::new();
    let mut changed_files = Vec::new();

    for path in files {
        let content = fs::read_to_string(&path).map_err(|err| CheckError::ReadError {
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

        let _ = db.parse(file);
        let formatted = format(&content).map_err(|err| CheckError::FormatError {
            path: path.clone(),
            source: err,
        })?;
        if formatted != content {
            changed_files.push(path);
        }
    }

    Ok(CheckResult {
        checked_files: tracked.len(),
        changed_files,
    })
}
