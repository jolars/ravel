use std::{fs, path::Path};

use insta::assert_snapshot;

use ravel::parser::{debug_tree, reconstruct};

#[test]
fn assignment_simple_cst_snapshot() {
    let input = fixture_input("assignment_simple");
    let tree = debug_tree(&input);
    assert_snapshot!("assignment_simple_cst", tree);
}

#[test]
fn assignment_simple_is_lossless() {
    let input = fixture_input("assignment_simple");
    let reconstructed = reconstruct(&input);
    assert_eq!(reconstructed, input);
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
