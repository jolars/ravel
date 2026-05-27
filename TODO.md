# Ravel roadmap

## Goal

Build a robust Rust-based foundation for R tooling with this implementation
order. **Strategy: bring the parser + formatter foundation to (near-)completion
first; defer the LSP and linter until that foundation is solid.**

- [x] Parser/CST foundation (initial bootstrap completed; continue expanding)
- [x] Formatter (first consumer)
- [ ] Linter and language server integration (later phases)

## Architecture decisions

- [x] Use a **lossless CST** built with `rowan` (preserve all tokens and
      trivia).
- [x] Use a **hand-written parser**:
      - [x] recursive descent for structural forms
- [x] Pratt parser for expressions and operator precedence
- [x] Use an **event-based parser pipeline** (`start node` / `token` /
      `finish node`) and then lower into rowan.
- [ ] Keep semantics **static** (no R code evaluation).
- [x] Use `salsa` for file text and parse caching first; expand to dependency
      graph tracking in later phases.

## Phased plan

## Phase 0: Parser foundations

- [x] Define initial token kinds and syntax kinds (expand for full R operator
      surface in next iterations).
- [x] Implement a lossless lexer:
      - [x] preserve whitespace/newlines
      - [x] preserve comments
      - [x] lex `%...%` operators as single tokens
      - [x] distinguish `[[` and `]]` cleanly
- [x] Build initial parser infrastructure:
      - [x] token source (minimal, lexer-backed)
      - [ ] event sink
      - [ ] marker/checkpoint utilities
      - [x] parser diagnostics container (initial assignment error coverage)

## Phase 1: Expression parsing

- [x] Implement Pratt parser skeleton with explicit binding powers and
      associativity (`+`, `*`, `^`).
- [x] Cover infix precedence and parenthesized expression baseline.
- [x] Handle right-associative power (`^`) and assignment integration
      (`a <- 1 + 2`).
- [x] Add focused parser tests per operator group, including malformed infix
      cases (`1 +`, `* 2`).

## Phase 2: Structural forms and statements

- [x] Parse control and structural constructs (`if`, `for`, `while`, `function`,
      blocks).
- [x] Define statement boundary rules, especially newline-sensitive cases.
- [x] Handle ambiguous contexts such as `=` in argument lists vs assignment.
      (done: `is_named_arg` in `src/parser/expr.rs`)
- [x] Add recovery rules that keep CST shape stable after syntax errors.

## Phase 2.5: Parsing completeness and hardening

- [x] Expand operator/assignment coverage (`=`, `<<-`, `->`, `->>`) with
      explicit precedence and associativity decisions. (lexer + Pratt binding
      powers cover all assignment operators)
- [ ] Formalize newline-sensitive statement boundary behavior for edge cases
      (continuations, dangling constructs, nested forms).
- [ ] Add targeted parsing fixtures for ambiguous contexts (argument defaults,
      named arguments, chained assignments, mixed control-flow/assignment
      forms).
- [ ] Consolidate parser diagnostics for consistency (message style, span
      precision, recovery node shape guarantees).

## Phase 2.6: AIR parser snapshot hardening backlog

Use AIR snapshot cases as incremental parser-hardening input. Execute in order:
easy -> medium -> hard.

- [x] Phase A (easy): port easy `ok` + `error` cases
- [x] Phase B (medium): port medium `ok` cases
- [x] Phase C (hard): implement grammar needed for hard `ok`/`error`/`undefined`
      cases

### AIR `ok` cases (29)

- [x] `ok/binary_expressions.R` (easy)
- [x] `ok/braced_expressions.R` (easy)
- [x] `ok/calls.R` (easy)
- [x] `ok/comments.R` (easy)
- [x] `ok/parenthesized_expression.R` (easy)
- [x] `ok/semicolons/semicolon-end-of-file-01.R` (easy)
- [x] `ok/semicolons/semicolon-end-of-file-02.R` (easy)
- [x] `ok/semicolons/semicolon-end-of-file-03.R` (easy)
- [x] `ok/semicolons/semicolon-start-of-file-01.R` (easy)
- [x] `ok/semicolons/semicolon-start-of-file-02.R` (easy)
- [x] `ok/semicolons/semicolons.R` (easy)
- [x] `ok/if_statement.R` (easy)
- [x] `ok/unary_expressions.R` (medium)
- [x] `ok/subset.R` (medium)
- [x] `ok/subset2.R` (medium)
- [x] `ok/extract_expression.R` (medium)
- [x] `ok/namespace_expression.R` (medium)
- [x] `ok/function_definition.R` (medium)
- [x] `ok/for_statement.R` (medium)
- [x] `ok/while_statement.R` (medium)
- [x] `ok/value/double_value.R` (medium)
- [x] `ok/value/integer_value.R` (medium)
- [x] `ok/value/string_value.R` (medium)
- [x] `ok/crlf/multiline_string_value.R` (medium)
- [x] `ok/keyword.R` (hard)
- [x] `ok/repeat_statement.R` (hard)
- [x] `ok/dots.R` (hard)
- [x] `ok/dot_dot_i.R` (hard)
- [x] `ok/value/complex_value.R` (hard) — ⚠️ fixture ported but lexing is
      **incorrect**: the imaginary suffix `i` is not lexed, so `1i` becomes
      `INT "1"` + `IDENT "i"`. See "Known issues / follow-ups" below.

### AIR `error` cases (7)

- [x] `error/call/side_by_side_arguments.R` (easy)
- [x] `error/parenthesized_expression/empty.R` (easy)
- [x] `error/parenthesized_expression/multiple.R` (easy)
- [x] `error/namespace_expression/call_lhs_double_colon.R` (hard)
- [x] `error/namespace_expression/call_lhs_triple_colon.R` (hard)
- [x] `error/namespace_expression/chained_double_colon.R` (hard)
- [x] `error/namespace_expression/chained_triple_colon.R` (hard)

### AIR `undefined` cases (2)

- [x] `undefined/extract_expression_error.R` (hard)
- [x] `undefined/namespace_expression_error.R` (hard)

## Phase 3: Rowan CST + validation

- [x] Build direct rowan CST construction and expose debug-tree output.

- [x] Guarantee losslessness by round-trip checks (source -> CST -> source
      text).

- [x] Add snapshot-style CST tests for initial fixture corpus (expand to broader
      representative/malformed set next).

- [x] Expand fixture corpus for lexer coverage (comments, strings, floats,
      `%...%`, `[[`/`]]`) with snapshots and losslessness checks.

- [x] Snapshot parser diagnostics per fixture, including malformed input
      (`assignment_missing_rhs`).

## Phase 3.2: Typed AST wrappers over CST (rowan)

Done: implemented in `src/ast/nodes.rs` with tests in `tests/ast_wrappers.rs`.

- [x] Introduce typed AstWrappers using rowan's built-in AST support (`AstNode`,
      `ast::support`).
- [x] Add wrapper coverage for current core nodes (`AssignmentExpr`,
      `BinaryExpr`, `IfExpr`, `ForExpr`, `WhileExpr`, `FunctionExpr`,
      `BlockExpr`).
- [x] Keep wrappers zero-cost over lossless CST (no semantic evaluation, no data
      duplication).
- [x] Add tests validating wrapper casting/traversal against snapshot fixtures.

## Phase 3.5: CLI bootstrap

- [x] Expose parse CLI surface (`ravel parse [file] [--quiet] [--verify]`).

- [x] Support parsing from file path or stdin.

- [x] Wire `--verify` to parser losslessness invariant.

## Phase 4: Incremental and project model (`salsa`)

- [x] Model file text, token stream, parse events, and CST as salsa queries.
- [x] Implement targeted invalidation for file edits.
- [ ] Add parse performance and incremental-reparse benchmarks.

## Phase 5: Formatter v1 (first consumer)

- [x] Implement formatter rules over CST while preserving comments and
      semantics.
- [x] Add stable formatting tests (idempotence and regression suites).
- [x] Expose formatter CLI surface (`format`, `--check`).

## Phase 5.2: Formatter v2 quality and coverage

- [ ] Expand formatter coverage for additional parsed constructs and edge cases
      while preserving comments/trivia.
- [ ] Add configurable formatting knobs aligned with `ravel.toml` defaults (line
      width, indentation, selected style toggles).
- [ ] Improve stability/perf with larger fixture corpus and deterministic output
      across multi-file runs.
- [ ] Add migration/regression tests to ensure v2 changes remain predictable and
      safe.

## Phase 5.3: Formatter IR (layout) architecture

Done: replaced the ad-hoc "render to String then measure" model with a
Wadler/Prettier-style document IR (`src/formatter/ir.rs`) and a single best-fit
layout engine (`src/formatter/printer.rs`). Construct formatters build an `Ir`
tree; the printer makes all line-break decisions and the whole document is one
IR tree printed once. Migrated behavior-preserving (byte-identical across the
fixture/idempotence/round-trip suite).

- [x] `Ir` enum (text/concat/line/soft-line/hard-line/empty-line/indent/group/
      if_break/verbatim) + `Printer` layout engine with width-aware `fits`.
- [x] Migrate scalar/operator/control-flow-loop/block/paren/root constructs to
      native IR (atoms, assignment, unary, binary incl. sticky ops + pipes,
      paren, block, for/while/repeat, statement sequence + external bodies).
- [x] Bridge if/else and subset/call/function into the IR via `Verbatim` (kept
      their specialized renderers): if/else gains nothing from IR width logic,
      and the arg-list constructs have an idiosyncratic string-based wrapping
      algorithm that cannot be ported byte-identically. See follow-ups.

## Phase 5.5: Project configuration (TOML, Ruff-inspired)

- [ ] Define `ravel.toml` configuration schema and defaults (human-friendly,
      explicit, and forward-compatible).
- [ ] Support configuration discovery hierarchy (cwd -> parent dirs) and
      precedence with CLI flags.
- [ ] Add sections for formatter and linter settings (start minimal,
      expandable).
- [ ] Validate and report configuration errors with clear file/field context.
- [ ] Add tests for config parsing, discovery, precedence, and invalid files.

## Known issues / follow-ups

Foundation-hardening items to address before (or alongside) wrapping up the
parser + formatter foundation, and ahead of the LSP/linter phases.

### Parser

- [ ] **Complex literals are mis-lexed.** The imaginary suffix `i` is not
      recognized, so `1i`, `2.5i`, `1e6i`, `0x123Fi` lex as a numeric token
      followed by `IDENT "i"` instead of a single imaginary literal. Fix the
      lexer (`src/parser/lexer.rs`) to consume a trailing `i` on numeric
      literals, add a dedicated token/`SyntaxKind` if warranted, and refresh the
      `air_ok_value_complex_value` snapshot.

### Formatter

- [x] **Native IR arg-wrapping for subset/call/function.** All three now build
      their arg/param lists natively on the IR (group/soft-line based, with a
      `group_hug` trailing-block primitive); the `Verbatim` bridge is gone for
      the common cases. Came out byte-identical on every fixture; the one
      intentional change is that a single-statement function body that is a
      named call argument now flattens to a bare body, matching the flatten rule
      already used elsewhere (`call_named_function_argument` guards it).
- [x] **Function-definition call args + trailing-function hug → native IR.**
      `ir_call_expr` no longer defers to the legacy renderer for natively
      renderable function args: a sole function arg hugs the parens
      (pass-through), and a trailing positional `function(...) { ... }` hugs via
      the `group_hug` primitive (no more build-time `fits_with_newlines` over a
      verbatim string). Function args that themselves need the string renderer
      (comments, brace-token defaults, bare body embedding a block) keep the
      whole call on legacy via `function_expr_needs_legacy`. The
      named-function-args force-multiline rule is ported
      (`should_force_multiline_named_functions`). One intentional layout change
      (`call_trailing_inline_function` guards it): a multi-arg call whose
      trailing function's params must break now expands one arg per line instead
      of hugging `callee(x, function(` — ravel's single-pass printer cannot
      reproduce the legacy two-phase "format the function, then measure" hug.
- [ ] **Migrate the remaining legacy call/param fallbacks to native IR.**
      `ir_call_expr` / `ir_function_expr` still defer to the string renderers for
      arg/param lists carrying comments (relocation unported) and for curly-curly
      `{{ }}` args. Porting comment relocation is the last blocker for removing
      the fallbacks.
- [ ] Once those fallbacks are gone, retire the now-dead string renderers
      (`format_call_expr`, `format_function_expr`, and their param/arg helpers)
      and the retained `fits_inline` / `fits_with_newlines` width helpers
      (`src/formatter/context.rs`) — the latter survive only for the
      string-based control-flow loop-body / external-body helpers in `rules/`,
      so they go when control flow is migrated.

## Phase 6: Linter and LSP integration (deferred)

- [ ] Add semantic layers: symbols, scopes, and lightweight inference.
- [ ] Build diagnostics and lint passes on CST + semantic model.
- [ ] Integrate with `tower-lsp-server` for IDE features.
