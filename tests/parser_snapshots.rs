use std::{fs, path::Path};

use insta::assert_snapshot;

use ravel::parser::{parse, reconstruct};

#[test]
fn parser_fixtures_snapshots_and_losslessness() {
    for name in fixture_names() {
        let input = fixture_input(name);
        let output = parse(&input);
        let tree = format!("{:#?}", output.cst);
        assert_snapshot!(format!("{name}_cst"), tree);
        assert_snapshot!(
            format!("{name}_diagnostics"),
            format!("{:#?}", output.diagnostics)
        );

        let reconstructed = reconstruct(&input);
        assert_eq!(
            reconstructed, input,
            "lossless round-trip failed for {name}"
        );
    }
}

fn fixture_input(name: &str) -> String {
    let path = Path::new("tests")
        .join("fixtures")
        .join("parser")
        .join(name)
        .join("input.R");
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read fixture {}: {err}", path.display());
    })
}

fn fixture_names() -> &'static [&'static str] {
    &[
        "assignment_simple",
        "assignment_float",
        "assignment_string",
        "comment_only",
        "user_operator_tokens",
        "double_brackets_tokens",
        "assignment_missing_rhs",
        "expr_precedence",
        "expr_right_assoc_power",
        "expr_paren_precedence",
        "assignment_expr_rhs",
        "expr_missing_rhs",
        "expr_unexpected_prefix_op",
    ]
}
