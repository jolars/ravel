use std::{fs, path::Path};

use ravel::{
    formatter::format,
    parser::{parse, reconstruct},
};

#[test]
fn parse_format_fixtures_are_stable_and_parseable() {
    for name in fixture_names() {
        let input = fixture_text(name, "input.R");
        let expected = fixture_text(name, "expected.R");

        let parsed_input = parse(&input);
        assert!(
            parsed_input.diagnostics.is_empty(),
            "fixture {name} input should be parseable, got diagnostics: {:#?}",
            parsed_input.diagnostics
        );

        let formatted = format(&input).unwrap_or_else(|err| {
            panic!("failed to format fixture {name}: {err}");
        });
        assert_eq!(
            formatted, expected,
            "formatted output mismatch for integration fixture {name}"
        );

        let reparsed = parse(&formatted);
        assert!(
            reparsed.diagnostics.is_empty(),
            "fixture {name} formatted output should be parseable, got diagnostics: {:#?}",
            reparsed.diagnostics
        );
        assert_eq!(
            reconstruct(&formatted),
            formatted,
            "fixture {name} formatted output should round-trip losslessly"
        );

        let reformatted = format(&formatted).unwrap_or_else(|err| {
            panic!("failed to reformat fixture {name}: {err}");
        });
        assert_eq!(
            reformatted, formatted,
            "fixture {name} formatting should be idempotent"
        );
    }
}

fn fixture_text(name: &str, file: &str) -> String {
    let path = Path::new("tests")
        .join("fixtures")
        .join("formatter_e2e")
        .join(name)
        .join(file);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read fixture {}: {err}", path.display());
    })
}

fn fixture_names() -> &'static [&'static str] {
    &["assignment_precedence", "if_else_block", "inline_comment"]
}
