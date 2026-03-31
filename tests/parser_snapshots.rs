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
        "assignment_eq",
        "assignment_left2",
        "assignment_right",
        "assignment_right2",
        "comment_only",
        "user_operator_tokens",
        "double_brackets_tokens",
        "assignment_missing_rhs",
        "assignment_missing_rhs_eq",
        "expr_precedence",
        "expr_right_assoc_power",
        "expr_paren_precedence",
        "assignment_expr_rhs",
        "assignment_chain_right_assoc",
        "pipe_simple",
        "pipe_precedence",
        "expr_logical_relational",
        "expr_additive_multiplicative_colon",
        "expr_tilde_userop",
        "expr_extract_namespace",
        "expr_missing_rhs",
        "expr_unexpected_prefix_op",
        "if_simple",
        "if_else_blocks",
        "if_missing_condition_parens",
        "if_missing_condition_expr",
        "if_missing_rparen",
        "if_missing_then_expr",
        "if_missing_else_expr",
        "if_dangling_else",
        "for_simple",
        "for_newline_body",
        "for_missing_in",
        "for_missing_rparen",
        "while_simple",
        "while_newline_body",
        "while_missing_condition",
        "while_missing_rparen",
        "function_simple",
        "function_newline_body",
        "function_missing_body",
        "function_missing_rparen_body",
        "unclosed_block",
    ]
}
