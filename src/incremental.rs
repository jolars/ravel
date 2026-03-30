use std::sync::{Arc, Mutex};

use salsa::Setter;

use crate::parser::parse;

#[salsa::input]
pub struct SourceFile {
    #[returns(ref)]
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryKind {
    FileText,
    ParseFile,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryLogEntry {
    pub kind: QueryKind,
    pub file: SourceFile,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ParseDiagnosticData {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ParsedFile {
    pub cst_debug: String,
    pub diagnostics: Vec<ParseDiagnosticData>,
}

#[salsa::db]
pub trait IncrementalDb: salsa::Database {
    fn record_query(&self, entry: QueryLogEntry);
}

#[salsa::tracked]
pub fn file_text(db: &dyn IncrementalDb, file: SourceFile) -> String {
    db.record_query(QueryLogEntry {
        kind: QueryKind::FileText,
        file,
    });
    file.text(db).clone()
}

#[salsa::tracked]
pub fn parse_file(db: &dyn IncrementalDb, file: SourceFile) -> ParsedFile {
    db.record_query(QueryLogEntry {
        kind: QueryKind::ParseFile,
        file,
    });

    let parsed = parse(file_text(db, file).as_str());
    let diagnostics = parsed
        .diagnostics
        .into_iter()
        .map(|diagnostic| ParseDiagnosticData {
            message: diagnostic.message,
            start: diagnostic.start,
            end: diagnostic.end,
        })
        .collect();

    ParsedFile {
        cst_debug: format!("{:#?}", parsed.cst),
        diagnostics,
    }
}

#[salsa::db]
pub struct IncrementalDatabase {
    storage: salsa::Storage<Self>,
    query_log: Arc<Mutex<Vec<QueryLogEntry>>>,
}

impl Default for IncrementalDatabase {
    fn default() -> Self {
        Self {
            storage: salsa::Storage::new(None),
            query_log: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl IncrementalDatabase {
    pub fn add_file(&self, text: impl Into<String>) -> SourceFile {
        SourceFile::new(self, text.into())
    }

    pub fn set_file_text(&mut self, file: SourceFile, text: impl Into<String>) {
        file.set_text(self).to(text.into());
    }

    pub fn parse(&self, file: SourceFile) -> ParsedFile {
        parse_file(self, file)
    }

    pub fn clear_query_log(&self) {
        self.query_log
            .lock()
            .expect("query log mutex poisoned")
            .clear();
    }

    pub fn query_log(&self) -> Vec<QueryLogEntry> {
        self.query_log
            .lock()
            .expect("query log mutex poisoned")
            .clone()
    }
}

#[salsa::db]
impl salsa::Database for IncrementalDatabase {}

#[salsa::db]
impl IncrementalDb for IncrementalDatabase {
    fn record_query(&self, entry: QueryLogEntry) {
        self.query_log
            .lock()
            .expect("query log mutex poisoned")
            .push(entry);
    }
}
