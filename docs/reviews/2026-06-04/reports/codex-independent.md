# Reviewer G — Codex independent

Model: GPT (Codex CLI 0.131.0, default model). Scope: whole crate + binding, read-only sandbox.

Independent second opinion alongside the six Claude-agent reviewers. Findings cross-checked against the other reviewers' reports during synthesis.

## selector/parser.rs

### WARN — integer syntax errors lose structured context

- Location: `crates/marxml/src/selector/parser.rs:297`
- What: `read_unsigned_int` maps both scan-budget overflow and parse overflow through `syntax_error("integer out of range")`, which becomes `SyntaxKind::Expected { what: ... }`.
- Why it matters: `SyntaxKind::IntegerOutOfRange` and `ExpectedDigit` exist but are not used, so callers cannot reliably machine-match these cases.
- Suggested fix: return `SyntaxKind::IntegerOutOfRange` at overflow sites and `SyntaxKind::ExpectedDigit` when no digits were read.

> **Invalidated during verification.** Re-reading `parser.rs:288-316` showed the code already emits the structured `SyntaxKind::IntegerOutOfRange` and `SyntaxKind::ExpectedDigit` directly — Codex misread which arms used `syntax_error("...")`. Dropped from the consolidated SUMMARY.

## bindings/node/src/lib.rs

### WARN — unknown schema attribute kinds silently weaken validation

- Location: `bindings/node/src/lib.rs:382`
- What: Unknown `AttrConstraintShape.kind` values default to `AttrKind::String`.
- Why it matters: A typo like `"regexp"` silently disables intended validation for that attribute.
- Suggested fix: reject unknown kinds with `napi::Error(Status::InvalidArg, ...)`.

### ERROR — clippy allows need local justification

- Location: `bindings/node/src/lib.rs:18`
- What: Crate-level `#[allow(clippy::needless_pass_by_value)]` has no nearby rationale.
- Why it matters: This codebase treats unjustified allows as errors; removing it would re-trigger clippy on napi-shaped APIs.
- Suggested fix: add a short reason, e.g. `// napi-rs requires owned JS bridge values for generated FFI signatures.`

## selector/matcher.rs

### ERROR — `too_many_arguments` allow is unjustified

- Location: `crates/marxml/src/selector/matcher.rs:62`
- What: `walk` suppresses `clippy::too_many_arguments` without documenting why the shape is intentional.
- Why it matters: This is an allow that would re-trigger clippy under CI if removed.
- Suggested fix: either add a short justification or bundle stable traversal state into a small context struct.

## mutate.rs

### WARN — invalid internal splice ranges are reported as overlap skips

- Location: `crates/marxml/src/mutate.rs:343`
- What: `apply_splices` silently skips out-of-bounds or reversed ranges and increments `skipped_overlaps`.
- Why it matters: Invalid ranges are internal invariant failures, not selector overlaps; silently folding them into the report can hide byte-offset bugs.
- Suggested fix: use `debug_assert!`/`expect` for internal range validity, or add a distinct internal error path if this helper becomes public.

## lib.rs

### INFO — docs.rs root URL is stale

- Location: `crates/marxml/src/lib.rs:23`
- What: `html_root_url` points at `marxml/0.0.0` while the workspace version is `0.1.3`.
- Why it matters: Generated rustdoc links can target the wrong version.
- Suggested fix: update it during release prep or remove it unless cross-version doc links are needed.

## Clean — tokenizer.rs

Verified the tokenizer's state machine, comment/CDATA skipping, attribute parsing, duplicate attr detection, and byte-offset narrowing. No regex tokenization, AST rebuild, or obvious line/offset off-by-one found.

## Clean — parse.rs

Verified stack assembly, same-tag nesting, max depth handling, content ranges, and sibling-scoped duplicate-id tracking.

## Clean — document.rs / types.rs

Verified `Markdown` accessors, `ElementRef` borrowing, subtree selection entry points, and `TextSegments` child/trivia skipping.

## Clean — selector AST/errors/mod

Verified selector AST shape, public `Selector::parse`, union dedupe plumbing, and structured selector error types apart from the parser context loss above (later invalidated).

## Clean — serialize.rs / schema.rs / validate.rs / escape.rs / error.rs

Verified escaping paths, serializer trivia handling, schema compilation, validation walks, and parse error structure. No forbidden tokenizer regex or byte-preserving mutation violation found.

## Clean — bindings/node/marxml.mjs / marxml.d.ts / Cargo.toml

Verified public wrapper shape, exported TypeScript surface, workspace lints, MSRV, and dependency layout. The Node public API correctly hides raw `NativeMarkdown`.

## Checked

- `Cargo.toml` lines 1–41
- `crates/marxml/src/lib.rs` lines 1–49
- `crates/marxml/src/tokenizer.rs` lines 1–548
- `crates/marxml/src/parse.rs` lines 1–249
- `crates/marxml/src/document.rs` lines 1–220
- `crates/marxml/src/types.rs` lines 1–271
- `crates/marxml/src/selector/ast.rs` lines 1–52
- `crates/marxml/src/selector/error.rs` lines 1–87
- `crates/marxml/src/selector/matcher.rs` lines 1–179
- `crates/marxml/src/selector/mod.rs` lines 1–68
- `crates/marxml/src/selector/parser.rs` lines 1–353
- `crates/marxml/src/mutate.rs` lines 1–361
- `crates/marxml/src/serialize.rs` lines 1–325
- `crates/marxml/src/schema.rs` lines 1–428
- `crates/marxml/src/validate.rs` lines 1–287
- `crates/marxml/src/escape.rs` lines 1–307
- `crates/marxml/src/error.rs` lines 1–206
- `bindings/node/src/lib.rs` lines 1–450
- `bindings/node/marxml.mjs` lines 1–80
- `bindings/node/marxml.d.ts` lines 1–125
