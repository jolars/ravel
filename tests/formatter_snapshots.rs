use std::{fs, path::Path};

use insta::assert_snapshot;

use ravel::formatter::format;

#[test]
fn formatter_fixtures_match_expected_and_snapshots() {
    for name in fixture_names() {
        let input = fixture_text(name, "input.R");
        let expected = fixture_text(name, "expected.R");
        let formatted = format(&input).unwrap_or_else(|err| {
            panic!("failed to format fixture {name}: {err}");
        });

        assert_eq!(formatted, expected, "formatted output mismatch for {name}");
        assert_snapshot!(format!("{name}_formatted"), formatted);
    }
}

fn fixture_text(name: &str, file: &str) -> String {
    let path = Path::new("tests")
        .join("fixtures")
        .join("formatter")
        .join(name)
        .join(file);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read fixture {}: {err}", path.display());
    })
}

fn fixture_names() -> &'static [&'static str] {
    &["noop_assignment", "noop_if_else_block", "noop_comments"]
}
