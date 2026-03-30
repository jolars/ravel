use std::collections::HashMap;

use ravel::incremental::{IncrementalDatabase, QueryKind};

fn count_by_kind(entries: &[ravel::incremental::QueryLogEntry]) -> HashMap<QueryKind, usize> {
    let mut counts = HashMap::new();
    for entry in entries {
        *counts.entry(entry.kind).or_insert(0) += 1;
    }
    counts
}

#[test]
fn parse_query_is_reused_when_input_unchanged() {
    let db = IncrementalDatabase::default();
    let file = db.add_file("x <- 1 + 2\n");

    let first = db.parse(file);
    assert!(first.diagnostics.is_empty());

    db.clear_query_log();
    let second = db.parse(file);
    assert_eq!(first, second);

    assert!(
        db.query_log().is_empty(),
        "expected no query re-execution for unchanged input"
    );
}

#[test]
fn editing_one_file_invalidates_only_that_file_queries() {
    let mut db = IncrementalDatabase::default();
    let file_a = db.add_file("x <- 1 + 2\n");
    let file_b = db.add_file("y <- 3 + 4\n");

    let baseline_a = db.parse(file_a);
    let baseline_b = db.parse(file_b);
    assert!(baseline_a.diagnostics.is_empty());
    assert!(baseline_b.diagnostics.is_empty());

    db.clear_query_log();
    db.set_file_text(file_a, "x <- 10 + 2\n");

    let updated_a = db.parse(file_a);
    let stable_b = db.parse(file_b);

    assert!(updated_a.diagnostics.is_empty());
    assert_eq!(baseline_b, stable_b);

    let log = db.query_log();
    let file_a_entries: Vec<_> = log
        .iter()
        .copied()
        .filter(|entry| entry.file == file_a)
        .collect();
    let file_b_entries: Vec<_> = log
        .iter()
        .copied()
        .filter(|entry| entry.file == file_b)
        .collect();

    assert!(
        file_b_entries.is_empty(),
        "expected file_b queries to be reused after file_a edit"
    );

    let counts = count_by_kind(&file_a_entries);
    assert_eq!(counts.get(&QueryKind::ParseFile), Some(&1));
    assert_eq!(counts.get(&QueryKind::FileText), Some(&1));
}
