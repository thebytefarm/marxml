# Reviewer brief — 2026-06-04 audit

The rubric each reviewer (Claude agents + Codex) was given before reading any source. Preserved verbatim so future audits can reuse it.

## Repo facts

- Rust workspace, MSRV 1.75, edition 2021. Toolchain in use: rustc 1.95.0.
- Workspace lints already enforce: `unsafe_code = "forbid"`, `missing_docs = "warn"`, `clippy::all + clippy::pedantic` at warn, with `module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc` allowed.
- CI runs `fmt --check`, `clippy -D warnings`, `test`, coverage, on Linux/macOS/Windows.
- Hard rules in `CLAUDE.md` / `AGENTS.md`:
  - No `unsafe` in crate code.
  - No regex in the tokenizer (it is a hand-rolled state machine; regex breaks same-tag nesting + byte-offset bookkeeping).
  - Mutators take `&str` and return `String` by **byte-preserving splice** — never AST-rebuild.
  - `Selector::parse` is compile-once / reuse-many; do not propose helpers that re-parse per call.
  - Two-track API: Rust crate is source of truth; Node binding is a thin `#[napi]` wrapper. Public Node API is `marxml.mjs` + `marxml.d.ts` — `index.js`/`index.d.ts` are napi-rs raw output.

## Author profile

- Senior engineer, beginner in Rust itself. Catch TS-brain traps:
  - Factory functions vs `From`/`Into`/builders.
  - `Option`/`Result` treated as nullable instead of values to pattern match.
  - Async-is-free assumption.
  - `Box<dyn Trait>` where `impl Trait` + generics fit.
- Defaults:
  - `Result<T,E>` + `?`, `thiserror` for libs.
  - `&str` over `String` in arguments unless ownership is required.
  - Avoid the `.clone()` reflex.
  - No `unwrap()` outside tests / binary `main`.
  - Don't fight the borrow checker.
  - Iterators over loops when it reads cleanly.

## Latest Rust idioms reviewers should weigh (2025–2026)

These are *signals* — not mandatory upgrades. Flag the gap, don't auto-rewrite.

- **Rust 2024 edition** is stable since 1.85 (Feb 2025). This crate is still on 2021. Notable 2024 deltas:
  - RPIT lifetime capture: `impl Trait` now captures all in-scope lifetimes by default — fewer `use<>` workarounds needed for return-position `impl Trait`.
  - `if let` temporary scope tightened — drops happen at the end of the `if let`, not the enclosing block.
  - Tail-expression temporary scope shortened.
  - `gen` is a reserved keyword.
  - `unsafe_op_in_unsafe_fn` is warn-by-default (moot here — crate forbids `unsafe`).
  - Cargo resolver v3 is default.
- **Clippy pedantic** is already on. Flag any lint suppressions (`#[allow(...)]`) that lack a justifying comment.
- **API guidelines hot list** (https://rust-lang.github.io/api-guidelines/):
  - C-COMMON-TRAITS: public types should derive `Debug`, `Clone`, `Copy`/`Eq`/`Hash` where they fit.
  - C-CONV: `From`/`Into`/`TryFrom` instead of ad-hoc `new_from_x`.
  - C-GETTER: getter naming follows convention (`thing()` not `get_thing()` unless setter exists).
  - C-ITER: methods returning iterators use `iter` / `iter_mut` / `into_iter`.
  - C-NEWTYPE: prefer newtypes over `bool` / raw `String` for distinct domains.
  - C-VALIDATE: validate args at the boundary.
  - C-DEBUG: every public type implements `Debug`.
  - C-SEND-SYNC: types are `Send`/`Sync` where feasible (parser types are good candidates).
  - C-DEREF: only smart pointers implement `Deref`.
  - C-CTOR: constructors are inherent static methods (`new`, `with_capacity`, etc.).
- **Error doctrine** (`thiserror` 2.0):
  - One enum per logical surface; `#[from]` for transparent conversions.
  - Avoid `String` payloads in error variants when a structured field (span, byte offset) is available.
- **`&str` vs `String` vs `Cow<'a, str>`**:
  - Args: `&str`. Returns: `String` (owned) or `Cow<'a, str>` if the function sometimes borrows the input verbatim — this is a common parser optimization.
- **napi-rs v3 patterns** (this repo uses napi v3):
  - `#[napi]` on functions / classes; async functions return Promises automatically.
  - Prefer `Buffer` / `Uint8Array` over `Vec<u8>` for large payloads to avoid copy across the JS boundary.
  - `napi::Error::from_reason` is the convenience; map crate errors via `From` impl to keep call sites clean.
  - Compile-once selector should be exposed as a JS class, not a string passed per call (matches the Rust hot-path contract).
- **Parser-specific idioms**:
  - Byte-index iteration (`source.as_bytes()` + indexed loop) is idiomatic for ASCII-fast paths; flag any `chars().enumerate()` that loses byte offsets in the tokenizer.
  - Stack-based assembly should keep the stack typed (not `Vec<usize>` of opaque indices). Look for "stringly typed" frame data.
  - `&[u8]` slices over `&str` when the parser only inspects ASCII boundaries — but watch for UTF-8 correctness at user-visible boundaries.

## What to look for, by severity

- **ERROR** (must fix before next release):
  - Soundness: panics on valid input, byte-offset arithmetic that can land mid-codepoint, off-by-one in splice ranges, mutation paths that do not byte-preserve.
  - API guideline violation that breaks downstream usage (`Debug` missing on a re-exported type, error variants that lose context).
  - Clippy `-D warnings` would fail (look for `#[allow]` without justification).
  - Doc claim contradicts behavior (e.g., docs say "compile once" but implementation re-parses).
- **WARN** (should fix this quarter):
  - Idiomatic gaps: `.clone()` reflex, `String` where `&str` would do, `Box<dyn ...>` where generics fit, `match` on a single arm that could be `if let`.
  - `Result<T, E>` ignored or eaten by `unwrap`/`expect` outside test code.
  - Missing `#[must_use]` on a pure constructor that returns an unused value silently.
  - Newtype opportunities: raw `usize` byte offsets across module boundaries.
  - Public API ergonomics: `From` impl missing where the user would expect one.
- **INFO** (nice-to-have, mention but don't push):
  - Rust 2024 edition migration opportunities.
  - Reorganization suggestions (a 545-line `tokenizer.rs` is fine if cohesive; only split if there's a real reason).
  - Bench-driven micro-opts only worth raising if a profiler / criterion would back them.

## Output format every reviewer must use

```
## <module under review>

### ERROR — <one-line summary>
- Location: `file.rs:LN`
- What: <observation, in 1–3 sentences>
- Why it matters: <consequence in 1 sentence>
- Suggested fix: <minimal change, code if useful>

### WARN — ...
### INFO — ...
```

If a section has no findings, write `### Clean — <module>` and a one-line note on what you checked.

End every report with a `## Checked` block listing the files + line ranges you actually read so the lead can audit coverage.

## What NOT to do

- Do not propose AST-rebuild mutators. Mutation must remain byte-splice.
- Do not propose regex in the tokenizer.
- Do not propose hand-bumping versions or changing release scripts.
- Do not propose adding deps without flagging them as `Ask first`.
- Do not edit any source file. Review pass only — findings go to a markdown report.
