# Rust + napi-rs best practices for marxml

What "idiomatic" means inside this repo, current as of 2026-06. The Rust toolchain in CI is 1.95 stable; the workspace targets `edition = "2021"` with MSRV `1.75`. This file is the rubric reviewers and reviewer-agents work from. AGENTS.md / CLAUDE.md have the hard *Never / Always / Ask First* rules; this file fills in the *Idiomatic / Why* layer below them.

If you change a rule here, update [AGENTS.md](../AGENTS.md) too if it touches the same surface (lint policy, mutator contract, MSRV, edition).

## Crate doctrine in 60 seconds

- **One pass over `&str` for parsing. Owned `String` only for the document handle.** The tokenizer is a hand-rolled state machine on `&[u8]`. The parser hands the source to `Markdown` once; everything else borrows.
- **Mutators return `String` by byte-preserving splice.** Never re-emit from the typed tree (that's what `to_xml` is for). Untouched bytes flow through verbatim.
- **`Selector::parse` compiles once, reuses many.** Anywhere a hot path takes `&str` for a selector instead of `&Selector` is a smell; benches and the public API both assume the compiled form.
- **`thiserror` for libraries.** Variants carry structured fields (line, byte offset, expected/found, regex pattern) — no `reason: String` payloads.
- **No `unsafe` in crate code.** Workspace `unsafe_code = "forbid"`. napi-rs derives unsafe FFI glue and that's allowed; your code isn't.
- **No regex in the tokenizer.** Same-tag nesting and byte-offset bookkeeping depend on the state machine. Regex is fine in selector attr matching and schema validation.
- **Two-track API.** Rust crate is source of truth; Node binding is a thin `#[napi]` wrapper. New surface lands in Rust first. Public Node API is `marxml.mjs` + `marxml.d.ts`; `index.js` / `index.d.ts` are napi-rs raw output.

## Rust idiom checklist

The reviewer agents grade against this list. Lints below are warn / deny in the workspace `Cargo.toml` (`clippy::all + clippy::pedantic` at warn, `unsafe_code` forbid, `missing_docs` warn). CI runs `clippy -D warnings`, so any local warning fails CI.

### Types

- **Prefer `&str` over `String` in arguments** unless ownership is required. Returns are `String` (owned) or `Cow<'a, str>` when sometimes-borrowed-from-input.
- **Newtype raw byte offsets** crossing module boundaries: `pub struct ByteOffset(u32)` reads better than `usize` and the bound is compile-checked.
- **Derive eagerly.** Public types derive `Debug, Clone, PartialEq, Eq` (and `Copy` / `Hash` / `PartialOrd` / `Ord` where applicable). `SourcePosition` and `SourceSpan` are the model — `Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord`. The C-COMMON-TRAITS guideline.
- **`#[non_exhaustive]` on every public error enum.** Adding variants stays additive across minor releases.
- **`#[must_use]` on pure constructors and builder methods** that return `Self`. Silently dropping a `MutationReport` is a bug; the lint prevents it.
- **Use `From` / `Into` / `TryFrom`**, not `new_from_X`. `impl FromStr for Markdown` + `impl TryFrom<String> for Markdown` is the model.

### Errors

- **One variant per logical failure.** No `Other(String)` catch-alls.
- **Structured fields, not string payloads.** When the failure is "value not in [a, b, c]" the variant carries `allowed: Vec<String>`, not `reason: "expected one of [...]"`. The error's `Display` impl reproduces the human message.
- **`#[from]` for transparent foreign-error conversion** (e.g. `regex::Error` → `SchemaError::InvalidRegex`). When the foreign type doesn't impl `Clone` / `Eq` (regex::Error doesn't), drop the conflicting derive on the wrapping enum and document why.
- **Every variant carries a position** (1-based `line: u32` minimum, byte offset preferred for tooling consumers). The only exception is failures raised *before* a position is established (`InputTooLarge`).

### Control flow

- **Iterators over index loops** when they read cleanly. `enumerate`, `filter_map`, `partition_point`, `windows`, `chunks` are all preferred over hand-rolled `for i in 0..len`.
- **`let-else` for unwrapping early-return paths.** `let Some(frame) = stack.pop() else { return Err(StrayClose) };` is the model.
- **Pattern-match `Option` and `Result`** — don't treat them like TS nullables.
- **No `.unwrap()` / `.expect()` in library code outside `#[cfg(test)]`.** Use `?`, `let-else`, or `unreachable!()` with a justifying comment when an invariant is type-system-guaranteed.

### Performance

- **`String::with_capacity(...)`** anywhere a hot path builds a buffer of a known-bounded size. `rewrite_open_tag` and `to_xml` are the models — the existing-attr + new-attr byte sum (or `raw.len()` for whole-document serializers) avoids the reallocation chain.
- **Borrow over clone.** `.clone()` is a fine escape hatch for prototyping; it's a smell in code review. `HashSet<&str>` is fine when the borrowed slice's lifetime spans the lookup. Use `Cow<'a, str>` when a function sometimes returns the input verbatim and sometimes returns owned (entity decoding is the model).
- **Lazy-compile regex.** Regex is allowed in selector attr matching and schema validation, but compile once and reuse. Schema's `SchemaBuilder::build` is the model — `Regex` lives inside `CompiledAttrKind` and `validate` only calls `.is_match(value)`.
- **Compiled-once handles in public API.** `Selector::parse` is the model: callers compile once, reuse against many docs. Don't add a `select_str(src, "css")` convenience that re-parses on every call.

### Comments + docs

- **Default to writing no comments.** Only add one when the *why* is non-obvious: a hidden invariant, a workaround, behavior that would surprise a reader.
- **Don't restate what the code does.** Well-named identifiers do that already.
- **Justify every `#[allow(...)]`.** The repo rule is hard: a bare `#[allow]` without a one-line explanatory comment is a CI failure waiting to happen. The model is `selector/matcher.rs::walk` — the comment names the trade and points at criterion as the arbiter.
- **rustdoc on every public item.** `missing_docs = "warn"` is on; CI escalates warnings. Crate-level rustdoc lives at `src/lib.rs`'s top and includes a runnable example.

### Lint policy

The workspace already encodes most of the above. If you want to add an `#[allow]`, the test is: would the next reviewer (human or agent) understand why? If not, fix the underlying design, not the lint output. The carve-outs we accept in `[workspace.lints.clippy]` are:

| Carve-out                  | Why                                                                 |
| -------------------------- | ------------------------------------------------------------------- |
| `module_name_repetitions`  | We deliberately name types `ParseError`, `MutateError`, etc. for grep-ability. |
| `must_use_candidate`       | Every pure return is already `#[must_use]` where it matters; the lint over-fires on internals. |
| `missing_errors_doc`       | The error doc lives on the `Error` enum, not at every call site that returns `Result`. |
| `missing_panics_doc`       | Panic paths are unreachable on caller-shaped input; the documented invariant lives near the `expect`/`unreachable!`. |

The napi binding (`bindings/node/Cargo.toml`) adds a similar carve-out for `needless_pass_by_value`, `missing_errors_doc`, and `missing_panics_doc`. The justification lives at the top of `bindings/node/src/lib.rs` — *every* `#[allow]` there names the napi-derive constraint that motivates it.

## napi-rs v3 specifics

The Node binding wraps the Rust crate. It must not invent semantics — any divergence between the Rust API and the Node API is a smell. Concrete rules:

- **Use `#[napi]` for everything that crosses the FFI.** No raw `napi-sys` calls; let the derive macro handle the FFI signatures.
- **Map crate errors via the `IntoNapi` extension trait in `bindings/node/src/lib.rs`** — every crate `Result<T, E>` becomes `Result<T, napi::Error>` with a single `.into_napi()?`. The orphan rule prevents a direct `From<marxml::*Error> for napi::Error` impl in this crate, so the trait stands in. Don't repeat `.map_err(|e| napi::Error::new(Status::InvalidArg, e.to_string()))` inline — add the conversion to the trait if it's missing.
- **Compiled-once handles must be exposed as napi classes** when their Rust counterpart is compile-once. Today: `Selector::parse` is compile-once in the crate but the Node API still parses the selector string per call. Same for `Schema::build`. Closing that gap is on the roadmap (see follow-ups in the review report). Until then, document the gap on the JS-side method.
- **Don't block the event loop on multi-MB inputs.** `parse` is currently sync; async variants are roadmap. If you add a heavy operation, default to `async fn` (napi-rs v3 returns a JS Promise automatically) and let the sync form be the convenience.
- **`Buffer` / `Uint8Array` for large binary payloads.** Strings already copy across the boundary; that's unavoidable for source documents. For anything we serialize back out and don't need V8 to inspect, prefer `Buffer`.
- **Public Node API lives in `marxml.mjs` + `marxml.d.ts`.** `index.js` / `index.d.ts` are napi-rs raw output. Document on `marxml.d.ts`; ts-types must match what `marxml.mjs` actually exports.
- **Target list.** `bindings/node/package.json#napi.targets` is the source of truth for which platforms ship. Adding a target is **Ask First** — it requires a matching `bindings/node/npm/<target>/` sub-package directory and changes to the release workflow's cross-compile matrix.

## What the 2024 edition would change (deferred)

The workspace is on 2021. Rust 2024 is stable since 1.85 (Feb 2025). The wins relevant to this crate:

- **RPIT lifetime capture.** `impl Iterator<Item = ... + 'a>` returns lose the explicit `+ 'a`; the lifetime is captured by default.
- **`if let` temporary scope tightening.** Drops at the end of the `if let`, not the enclosing block.
- **`gen` reserved.** Not used in marxml; trivially renamed if a contributor introduces it.

Migration is non-breaking but requires bumping MSRV to ≥1.85. **Ask First** per AGENTS.md before flipping the workspace `edition` field.

## Reviewer rubric — what we treat as severity

When a reviewer (human or agent) audits a PR or a slice of the crate:

- **ERROR** — must fix before merge. Soundness, byte-offset off-by-one, mutation that doesn't byte-preserve, API guideline violation that breaks downstream consumers, `#[allow]` without a justifying comment, doc that contradicts behavior.
- **WARN** — should fix this quarter. Idiomatic gap (`.clone()` reflex, `String` where `&str` would do, `Box<dyn>` where generics fit), missing `#[must_use]` on a pure constructor, newtype opportunity, missing `From` impl a user would expect, error variant losing context.
- **INFO** — defer; mention but don't push. Rust 2024 migration opportunities, doc nits, microopts that need a benchmark to justify.

## When in doubt

- Read `docs/ARCHITECTURE.md` before changing the tokenizer state machine, the parser stack, or the mutation strategy. Those three are load-bearing and the architecture doc spells out why.
- Read `docs/dsl/selectors.md` before touching the selector grammar.
- Run `just check` (= `fmt-check + clippy + test`) before pushing. CI runs the same set on Linux + macOS + Windows.
- If you can't tell whether your change should add a changeset, the test is: would a downstream user reading the changelog care? If yes, add one. If no (pure internal refactor, test, doc nit), skip it.
