use ravel::ast::{
    ArgList, AssignmentExpr, AstNode, BinaryExpr, BlockExpr, CallExpr, ForExpr, FunctionExpr,
    IfExpr,
};
use ravel::parser::parse;

#[test]
fn casts_core_expression_wrappers() {
    let parsed = parse(
        "x <- function(a, b) { a + b }\nz <- fn(1, 2)\nif (x) { y <- 1 + 2 }\nfor (i in 1:5) i\n",
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "fixture should parse cleanly: {:?}",
        parsed.diagnostics
    );

    let mut saw_assignment = false;
    let mut saw_call = false;
    let mut saw_arg_list = false;
    let mut saw_if = false;
    let mut saw_block = false;
    let mut saw_binary = false;
    let mut saw_for = false;
    let mut saw_function = false;

    for node in parsed.cst.descendants() {
        if AssignmentExpr::cast(node.clone()).is_some() {
            saw_assignment = true;
        }
        if CallExpr::cast(node.clone()).is_some() {
            saw_call = true;
        }
        if ArgList::cast(node.clone()).is_some() {
            saw_arg_list = true;
        }
        if IfExpr::cast(node.clone()).is_some() {
            saw_if = true;
        }
        if BlockExpr::cast(node.clone()).is_some() {
            saw_block = true;
        }
        if BinaryExpr::cast(node.clone()).is_some() {
            saw_binary = true;
        }
        if ForExpr::cast(node.clone()).is_some() {
            saw_for = true;
        }
        if FunctionExpr::cast(node).is_some() {
            saw_function = true;
        }
    }

    assert!(saw_assignment);
    assert!(saw_call);
    assert!(saw_arg_list);
    assert!(saw_if);
    assert!(saw_block);
    assert!(saw_binary);
    assert!(saw_for);
    assert!(saw_function);
}
