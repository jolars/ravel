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
- [x] `ok/value/complex_value.R` (hard) --- ⚠️ fixture ported but lexing is
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
      - [x] Phase A: port first batch of air formatter specs as `air_*`
            formatter fixtures (`air_smoke`, `air_comment`,
            `air_parenthesized_expression`, `air_value_double_value`,
            `air_value_integer_value`, `air_value_string_value`). All six pass
            the existing equality/parse/idempotence/losslessness/snapshot
            invariants in `tests/formatter.rs`. The seventh candidate
            `binary_expression.R` was dropped: ravel's parser emits 93
            diagnostics on the spec (`:=`, the `?`/`??`/`???` help-operator
            family, and other infix shapes not yet supported); see "Known
            issues / Parser" for the resulting follow-ups.
      - [x] Phase B: ported 13 of the 16 candidate air formatter specs as
            `air_*` fixtures. All pass equality/parse/idempotence/
            losslessness/snapshot invariants in `tests/formatter.rs`.
            Four ports (`air_for_statement`, `air_keyword`,
            `air_repeat_statement`, `air_while_statement`) match air
            byte-for-byte. Eight ports (`air_braced_expressions`, `air_call`,
            `air_dot_dot_i`, `air_function_definition`, `air_pipelines`,
            `air_program`, `air_subset2`, `air_test_that`) intentionally
            diverge: ravel's deterministic rule set drops persistent line
            breaks, collapses blank lines between a comment and the next
            statement, and does not name-special-case calls like `test_that`,
            so each `expected.R` records ravel's actual rule-based output as
            the locked regression baseline. The
            `air_binary_expression_sticky_subset` port subsets the spec to
            `$`/`::`/`:::`/`^`/`:`; ravel currently splits some "sticky" ops
            across lines instead of keeping them glued (regression baseline
            for a follow-up). Deferred (parser/formatter holes; not viable
            even as subsets without more work): `binary_expression_sticky`
            full (needs `?`, `**`, `@` --- the first two block parsing, the
            third blocks formatting), `if_statement` (parser doesn't allow
            comments between `if (` and `)`; formatter still rejects several
            comment-bracketed `if ... else` shapes as ambiguous),
            `subset` (parser fails on newline-between-args and on inner-
            subset arg-list newlines when followed by certain trivia),
            `unary_expression` (parser blocker resolved; formatter still
            needs air's complex-vs-terminal-operand spacing rule for
            unary `~` to match the spec byte-for-byte).
            Permanently out of scope (incompatible with ravel's tenets or
            missing features): the `persistent-line-breaks/`, `directives/`,
            `skip/`, `table/`, `crlf/` subdirs and `call_table.R`.
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

- [x] **Walrus assignment `:=`.** Lexed as `TokKind::Walrus` and treated as an
      assignment-level binary operator (same `(1, 1)` binding power as `<-` /
      `=`), producing `ASSIGNMENT_EXPR` with a `WALRUS` token. Fixture:
      `tests/fixtures/parser/expr_walrus`. Unblocks `air_binary_expression` for
      the formatter fixture batch.
- [x] **Help operator `?` (with chained forms `??`, `???`, …).** `?` now
      parses as both unary (`?topic`) and binary (`pkg?topic`) at lowest
      precedence (binding power `(0, 1)`, below assignment so `x <- 1 ? 2`
      becomes `(x <- 1) ? 2`). There is no separate `??` token: chains like
      `pkg??"x"` and `pkg???"x"` parse via repeated unary/binary application
      (`pkg ? (? "x")`, `pkg ? (? (? "x"))`), matching R itself. Fixture:
      `tests/fixtures/parser/expr_help_operator`. Note: the pre-existing
      `next_operator` newline-continuation bug also applies to `?`, so
      consecutive `?`-headed lines are still merged across newlines and
      formatter idempotence is not guaranteed for them — same root cause as
      the unary `~` follow-up below.
- [ ] **Comments inside `if (...)` condition break parsing.**
      `if (\n  a\n  # c\n) { ... }` reports "expected ')' after if condition";
      `if # c\n(a) TRUE` reports "expected '(' after 'if'". Surfaced by the
      air `if_statement.R` port.
- [ ] **Newline between subset args breaks parsing in some contexts.**
      `dt[, j\n  , by = col]` and inner `map[\n  names(df)\n]` followed by a
      comment block report "expected ',' between subset arguments" / "expected
      closing bracket". The same fragments parse standalone; the failure is
      context-dependent on surrounding trivia. Surfaced by the air `subset.R`
      port.

### Formatter

- [ ] **`@` slot extraction is unsupported.** Parsing succeeds but formatting
      raises `UnsupportedConstruct { kind: AT }`. Surfaced by the air
      `binary_expression_sticky.R` port (excluded from the subset).
- [ ] **`} else` separated by blank line / comment inside `{ ... }` is
      rejected as ambiguous.** `if/else` shapes like
      `{\n  if (c) this\n  # comment\n  else that\n}` raise
      `"ambiguous construct for formatter (root): "{\n  a\n}else""`. Surfaced
      by the air `if_statement.R` port.

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
      of hugging `callee(x, function(` --- ravel's single-pass printer cannot
      reproduce the legacy two-phase "format the function, then measure" hug.
- [x] **Curly-curly `{{ }}` call args → native IR.** Dropped the curly-curly
      check from `call_needs_legacy`; `ir_call_argument` now builds `{{ x }}`
      natively via `ir_curly_curly` (flat `{{ x }}`, or a group the printer
      re-indents when the symbol overflows) instead of bridging a `Verbatim`
      string. Byte-identical on every fixture and additionally fixes the
      mis-indented multi-line `{{ <long symbol> }}` case the verbatim bridge got
      wrong. Commented curly-curly forms still route to legacy via the comment
      gate (folds into comment relocation below).
- [x] **Native IR comment relocation for call/param arg lists.** Comments no
      longer force the legacy renderer: the
      `descendants_with_tokens().any(COMMENT)` gate is gone from both calls and
      function definitions. Calls with comments take an always-broken
      item-stream layout (`ir_call_args_with_comments`, the IR port of
      `format_arg_list_multiline`) that classifies each comment as trailing the
      previous line, leading on its own line, or standing alone using the same
      `leading_newline` / `newline_after` signals; every argument expression is
      built as real IR (`ir_call_arg_value`), comment-bearing curly-curly is
      lifted natively (`ir_curly_curly_with_comments`). Function definitions
      relocate leading-`function` comments (hoisted above), param-list comments
      (raw multiline, `ir_function_params_with_comments`), and body-outer
      comments (lifted into / bracing the body via
      `ir_block_expr_with_prefixed_comments` / `brace_wrap_body_with_comments`).
      Byte-identical on every fixture (`call_comments_*`,
      `function_definition_comments`, `braced_curly_curly_advanced`) and on the
      whole air R corpus (218 .R files); idempotent + lossless throughout. Two
      intentional improvements, both absent from the corpus and aligned with the
      prior curly-curly / native-IR work: (1) a *nested* commented function
      definition (e.g. a `.f = function(...) # c { ... }` call arg) that the
      legacy verbatim bridge mis-indented now lays out correctly (real IR, no
      retrospective measurement); (2) a commented *named* curly-curly value
      (`m = {{ # c\n x }}`) is now lifted to `{{ … }}` just like the no-comment
      path and positional curly-curly, instead of legacy's nested-block
      rendering --- so a sibling comment no longer changes how `m = {{ x }}`
      prints. Remaining legacy fallbacks: a function-definition *argument* whose
      own renderer needs legacy still routes its call to `format_call_expr`
      (`call_has_legacy_function` → `function_expr_needs_legacy`: a direct
      comment, brace-token default, or bare body embedding a block); brace-token
      param defaults (`function_has_brace_default`); a bare body carrying a
      forced break (control flow). The rare `ASSIGNMENT_EXPR`-arg-with-comment
      shape (not producible from diagnostic-free input) is kept on legacy via
      `call_comment_path_unsupported`.
- [x] **Function-definition-as-argument → native IR.** Dropped the
      `call_has_legacy_function` gate from `ir_call_expr`; a function arg with a
      brace-token default or a bare body embedding a block no longer routes its
      *call* through legacy --- only the function arg itself falls back,
      locally, via the function-level gates. To preserve the legacy "hug the
      prefix" layout (`map(x, function(a = { 1 }) { 1 })`), taught the printer's
      `first_line_fits` to measure the first line of a multi-line `Verbatim`
      instead of bailing on `force_break: true`; single-line force-break
      Verbatims (standalone comments) still fail, so the comment path is
      unaffected. Deleted `call_has_legacy_function`,
      `function_expr_needs_legacy`, `arg_is_legacy_function`,
      `bare_body_embeds_block`, `arg_function_node`. Byte-identical across the
      air corpus + repo fixtures; idempotent (modulo the pre-existing
      `air_ok_for_statement` `for`-quirk).
- [x] **Retire the dead `format_call_expr` / `format_function_expr` string
      renderers and their \~30 param/arg helpers.** Migrated the three remaining
      gates that kept them alive: brace-token param defaults (now a
      `Verbatim`-bridged native path in `ir_function_param_default` /
      `ir_brace_token_default`, with a nested-block heuristic that mirrors
      legacy's `param.contains("= {\n  {\n")` to force-break the params list);
      `call_comment_path_unsupported` (gate dropped --- the shape isn't
      producible from diagnostic-free input); and the
      `body_ir.contains_forced_break()` fallback (replaced by
      `Ir::ConditionalGroupAllLines` + `Printer::all_lines_fit`, the IR port of
      `fits_with_newlines`). The bare-body branch now builds two body IRs (one
      at `indent`, one at `indent + 1`) so a verbatim-bridged control-flow body
      lines up correctly when the body is wrapped in braces. Also dropped
      `fits_with_newlines` from `context.rs`; `fits_inline` keeps one remaining
      caller (`format_while_header` in `control_flow.rs`) and stays for a later
      migration. Byte-identical across the air corpus + repo fixtures; a new
      `function_bare_control_flow_body` fixture exercises bare `if`/`for`/
      `while`/`repeat` bodies plus a long-param auto-bracing case.
- [x] **Lift the single-pass printer limit (conditional-group / candidate
      layouts).** Added `Ir::ConditionalGroup(Rc<[Ir]>)` plus a break-aware
      `first_line_fits` measurement to the printer: the printer picks the first
      candidate whose first line fits at the current column (letting nested
      groups break naturally; success is the first emitted newline) and renders
      it flat, else renders the last candidate broken. With a single candidate
      this is a "break-aware group" --- flat if its first line fits, broken
      otherwise. Wired the trailing positional `function(...) ...` arg shape
      through it via `build_arg_hug_conditional`, restoring the uniform rule "a
      positional trailing function-callback hugs its call as long as
      `callee(leading, function(` fits, otherwise expands." The rule applies to
      all positional `FUNCTION_EXPR` trailing args (bare or block bodies), so
      the legacy auto-bracing workaround is no longer needed at the call level
      and idempotence holds without special-casing block-bodied vs bare. Plain
      trailing blocks (`map(xs, { ... })`) and subset trailing blocks keep the
      flat-only `group_hug`. `group_hug` is now a 2-state conditional in spirit
      and could be reframed onto `ConditionalGroup` as a follow-up. Verified
      byte-identical to HEAD across the air corpus + repo fixtures except the
      intentional `call_trailing_inline_function` diffs (4 cases moved to hug
      form, including the original target
      `map(x, function(<long       params>) {1})`); idempotent and lossless
      throughout (the only remaining non-idempotence is the pre-existing
      `air_ok_for_statement` `for`-quirk noted in memory).

## Phase 6: Linter and LSP integration (deferred)

- [ ] Add semantic layers: symbols, scopes, and lightweight inference.
- [ ] Build diagnostics and lint passes on CST + semantic model.
- [ ] Integrate with `tower-lsp-server` for IDE features.
