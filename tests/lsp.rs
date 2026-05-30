use ravel::formatter::{FormatStyle, format_with_style};
use ravel::lsp::compute_format_edits;

#[test]
fn reformats_unformatted_input_with_full_document_edit() {
    let input = "x<-1\n";
    let style = FormatStyle::default();
    let expected = format_with_style(input, style).expect("formats");
    assert_ne!(expected, input, "fixture must require reformatting");

    let edits = compute_format_edits(input, style).expect("formatter accepts input");
    assert_eq!(edits.len(), 1, "expected a single whole-document edit");

    let edit = &edits[0];
    assert_eq!(edit.new_text, expected);
    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.range.start.character, 0);
    assert_eq!(edit.range.end.line, 1);
    assert_eq!(edit.range.end.character, 0);
}

#[test]
fn no_edits_when_input_already_formatted() {
    let style = FormatStyle::default();
    let formatted = format_with_style("x <- 1\n", style).expect("formats");

    let edits = compute_format_edits(&formatted, style).expect("idempotent input");
    assert!(
        edits.is_empty(),
        "formatted input should produce no edits, got: {edits:?}"
    );
}

#[test]
fn returns_none_when_input_has_parse_errors() {
    let style = FormatStyle::default();
    // Unclosed parenthesis is a parser diagnostic; the formatter refuses.
    let result = compute_format_edits("function(x\n", style);
    assert!(result.is_none(), "expected None, got {result:?}");
}

#[test]
fn empty_document_produces_no_edits() {
    let style = FormatStyle::default();
    let edits = compute_format_edits("", style).expect("formatter accepts empty input");
    assert!(edits.is_empty(), "empty document should produce no edits");
}

#[test]
fn end_position_handles_input_without_trailing_newline() {
    let style = FormatStyle::default();
    // Force a reformat so we exercise the full-range computation. The result
    // must reach the last character of the trailing line when there's no `\n`.
    let input = "x<-1";
    let expected = format_with_style(input, style).expect("formats");
    if expected == input {
        // If a future formatter accepts this as already-canonical, the test
        // becomes uninteresting; fail loudly so we re-pick a fixture.
        panic!("fixture must require reformatting");
    }
    let edits = compute_format_edits(input, style).expect("formats");
    let edit = edits.first().expect("one edit");
    assert_eq!(edit.range.start.line, 0);
    assert_eq!(edit.range.end.line, 0);
    assert_eq!(edit.range.end.character, input.len() as u32);
}
