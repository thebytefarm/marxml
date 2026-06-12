# marxml — Consolidated audit summary (2026-06-04)

**Date:** 2026-06-04
**Reviewers:** 6 Claude agents (3 Opus, 3 Sonnet, parallelized) + 1 Codex (GPT) independent run.
**Scope:** Every `.rs` file in `crates/marxml/src/`, the napi-rs binding in `bindings/node/`, `Cargo.toml`, `marxml.mjs`, `marxml.d.ts`, plus `docs/ARCHITECTURE.md` and `docs/dsl/selectors.md` as contract-of-record.
**Rubric:** See [`RUBRIC.md`](./RUBRIC.md).
**Independence check:** Every ERROR was verified by reading the cited code at the cited line. One Codex finding (`parser.rs:297` "integer overflow loses structured kind") was **invalidated** during verification — the code correctly emits `SyntaxKind::IntegerOutOfRange` and `SyntaxKind::ExpectedDigit`. Findings below are net of that drop.

Headline: the parser core (tokenizer, parse, selector matcher, schema/validate) is in good shape — zero soundness bugs found across 6 reviewers. The action items concentrate in three buckets: (1) **two doc/code contradictions** that will trip the next contributor, (2) **Node binding violates the Rust crate's compile-once doctrine** for both `Selector` and `Schema`, and (3) **three undocumented `#[allow]` suppressions** that the repo's own `CLAUDE.md` flags as never-allowed.

> **Status reminder.** Items below describe what the audit found — see the [`README.md`](./README.md) status section for which have been shipped vs. deferred since the audit. Many of the doc/comment ERRORs and several WARNs have already landed in commits `ed50801` and `773cb8a`.

---

## ERRORs (must fix)

### E1 — `docs/ARCHITECTURE.md:92` claims descending splice sort; `mutate.rs:329` sorts ascending

Verified by reading both files.

- `mutate.rs:329-333` sorts splices **ascending** by start offset (with descending tie-break on end) and applies them left-to-right with a monotonic cursor copying untouched bytes into a fresh `String`.
- `docs/ARCHITECTURE.md:92` says "Sort splices by *descending* start offset so earlier mutations don't shift later positions."
- The code is the right strategy for a copy-into-fresh-buffer approach. The doc is correct only for an in-place `String::replace_range` strategy that this crate doesn't use.
- **Fix:** rewrite the Mutation section step 3 to: "Sort splices ascending by start offset; a monotonic cursor copies untouched bytes between splices into the output buffer. Overlapping splices are skipped and reported via `MutationReport::skipped_overlaps`."
- Doc change only. No code change.

### E2 — Node binding violates the Rust crate's "Selector compiled once" doctrine

Verified at `bindings/node/src/lib.rs:211-218` (`select`), `:227-237` (`update_attrs`), `:243-246` (`replace_content`), `:251-254` (`replace_text`), `:265-274` (`replace_in_content`). Every method calls `parse_selector(&selector)?` at the top.

- Per `AGENTS.md`: `Selector::parse` "compiled once, reused many. The benches assume this. Don't introduce a `select_str(src, "css")` convenience that re-parses on every call as the primary API."
- The Node binding currently has **only** the re-parsing form. There is no JS `Selector` class. Any JS workflow that runs the same selector across many docs (or many mutators on the same doc) re-parses the AST every call.
- The top-of-file comment at `bindings/node/src/lib.rs:9-10` actually claims "no per-call reparse" — that statement is true for the document but **false for the selector**, which makes the contradiction worse: it looks like the contract is honored.
- **Fix:** expose a `#[napi]` class `Selector` with `Selector.parse(css)` as the factory. Each `select`/`updateAttrs`/`replaceContent`/`replaceText`/`replaceInContent` method accepts either a `string` (sugar, current behavior) or a compiled `Selector` (perf path). Document the compiled form as the recommended path in `marxml.d.ts`. **Ask first** — public Node-binding surface change.

### E3 — `crates/marxml/src/selector/matcher.rs:62`: unjustified `#[allow(clippy::too_many_arguments)]`

Verified at `matcher.rs:62`. The attribute is bare; no rationale comment above it.

- `CLAUDE.md` `### Never` block: "Clippy warnings papered over with `#[allow(...)]` without an explanatory comment." This is the project's own rule.
- Reviewers B and G both surfaced this independently.
- **Fix:** one-line comment immediately above the attribute, e.g. `// 8 args is a deliberate trade — packing them into a context struct adds indirection on the hot tree walk, and benches show ~5–8% regression.` (substitute the actual reason). Or split out a `WalkCtx<'a>` struct if the perf claim doesn't hold up under criterion.

### E4 — `bindings/node/src/lib.rs:18-20`: three unjustified `#[allow]`s at crate root

Verified. Lines 18-20:
```rust
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
```

- The design-notes block at lines 1-16 explains the architecture but does not justify each allow.
- The justification *does* live elsewhere — in `bindings/node/Cargo.toml:25-27` — which is exactly the problem: a maintainer reading `lib.rs` sees no rationale.
- Same `CLAUDE.md` `Never` rule as E3.
- Flagged by Codex as ERROR; F reviewer noted as INFO. Strict reading of the project rule makes it ERROR.
- **Fix:** a comment immediately above the `#![allow]` block in `lib.rs` (the existing Cargo.toml comment can stay), naming the napi-derive constraint that motivates each lint.

### E5 — `crates/marxml/src/lib.rs:23`: `html_root_url` pinned to `0.0.0`

Verified at `lib.rs:23`. Workspace version is `0.1.3` (`Cargo.toml:7`).

- `#![doc(html_root_url = "https://docs.rs/marxml/0.0.0")]` — every cross-crate doc link from a downstream crate that uses `[`Link`]` style refs resolves to `https://docs.rs/marxml/0.0.0/...` which does not exist on docs.rs.
- This is a real, visible breakage for any crate that documents an integration with `marxml` — silently 404s.
- Flagged by reviewers E and G (independently — same evidence both arrived at).
- **Fix:** simplest — **remove the attribute entirely**. `html_root_url` is largely superseded by docs.rs's own link rewriting.

---

## WARNs (should fix this quarter)

### Node binding (heaviest cluster — `bindings/node/src/lib.rs`)

| # | Where | What | Why |
|---|---|---|---|
| W1 | every `#[napi]` method | sync `parse`/mutators block the Node event loop on multi-MB input | `MAX_INPUT_BYTES ≈ 4 GiB`. Server-side users will stall. Add `parseAsync` + async siblings, keep sync forms. |
| W2 | every error-mapping site | no `From<marxml::*Error> for napi::Error` impls; every `?` site repeats `.map_err(\|e\| Error::new(Status::InvalidArg, e.to_string()))` | Loses variant identity; `Status` is hardcoded `InvalidArg` everywhere. One `From` impl per crate error type fixes it. |
| W3 | `lib.rs:291-295` + `marxml.mjs:51-53` | `toJson` double-serializes (Rust `serde_json::to_string` → JS `JSON.parse`) | Two full passes over tree-as-JSON. Return `serde_json::Value` across FFI via napi's `serde-json` feature, drop the `JSON.parse`. |
| W4 | `lib.rs:303-321` + `lib.rs:376-415` | `validate` rebuilds `Schema` on every call | Mirrors E2 — same compile-once contract violated. Expose a `Schema` napi class. |
| W5 | `lib.rs:227-237` | `updateAttrs` discards `MutationReport.applied`/`.skipped` | JS callers can't distinguish "0 matches" from "5 skipped". Add a `updateAttrsReport` sibling. |
| W6 | `lib.rs:200-206` | `elements` getter re-materializes the whole tree on every JS read | Cache in a `OnceCell<Vec<Element>>` (doc is immutable so invalidation is moot), or rename to a method to signal cost. |
| W7 | `package.json` `napi.targets` | missing `aarch64-unknown-linux-musl` | Alpine on Graviton is a common deployment target. **Ask first** — new napi target. |
| W8 | `lib.rs:256-263` doc vs `lib.rs:346-370` impl | Docstring says JS regex flags map via `(?flags:…)` prefix; implementation uses `RegexBuilder` setters | User-observable behavior identical, but the doc lies about mechanism. Fix the doc. |
| W9 | `lib.rs:243, 251, 265` | Three methods declare `Result<String>` but the only fallible step is `parse_selector` | Falls out for free when W2 lands. |

### Schema + validate

| # | Where | What |
|---|---|---|
| W10 | `validate.rs:32-42, 259-287` | `ValidationError::InvalidAttr::reason: String` and `check_kind → Option<String>` discard structured constraint info (allowed enum values, regex pattern). Replace with `AttrFailureReason` enum; `Display` reproduces the current messages. **Public API change → changeset.** |
| W11 | `validate.rs:108` | `ValidationReport::iter()` returns concrete `std::slice::Iter` (semver-leaky). Switch to `impl Iterator<Item = &'_ ValidationError>`. |
| W12 | `schema.rs:169-177` | `SchemaError::InvalidRegex::reason: String` strips structured `regex::Error`. Lower priority — `regex::Error` doesn't impl `Clone`/`PartialEq` so wrapping is required. Document the tradeoff. |

### Selector parser

| # | Where | What |
|---|---|---|
| W13 | `selector/error.rs:14-15` | `SelectorError::UnexpectedEnd` has no `at: usize`. Selectors like `[id="foo` (unterminated string) lose the byte offset the parser already knew. **Public API change.** |
| W14 | `selector/parser.rs:270-286` | Only `"` quoted attr values accepted; `[id='t1']` errors. CSS3 accepts both. Either accept `'` (one-line change) or document the restriction visibly in `docs/dsl/selectors.md`. |
| W15 | `selector/parser.rs:270-286` | No `\"` escape; `[id="a\"b"]` silently parses as `AttrEquals(id, "a\\")` then errors on `b"]` — confusing cascade. Reject `\` inside attr values with a dedicated `SyntaxKind::EscapeNotSupported`, or document that the only escape mechanism is `&quot;`. |

### Mutate + serialize + escape

| # | Where | What |
|---|---|---|
| W16 | `mutate.rs:177-184` | `splice_regex` is a dead one-line wrapper around `splice_regex_with` from an incomplete refactor. Inline + delete. |
| W17 | `mutate.rs:142-157` | `pub(crate) fn try_replace_content` / `try_replace_in` return `MutationReport`, not `Result` — `try_` prefix is a lie. Rename to `splice_content_report` / `splice_regex_report` (or drop `try_` entirely). Internal naming; the public API in `document.rs` already uses the right names. |
| W18 | `mutate.rs:264`, `serialize.rs:96` | `rewrite_open_tag` and `to_xml` use `String::new()` on hot paths. Both have readily computable capacity hints (`tag + attrs` and `raw.len()` respectively). |
| W19 | `serialize.rs:80-93` | `pub fn self_close_empty(self) -> Self` shadows the `pub` field `self_close_empty: bool`. Rename the builder to `with_self_close_empty` or `collapse_empty`. |
| W20 | `escape.rs:210` | `chars().next().expect("non-empty tail")` in library code; the invariant is sound but the principle is "express via types, not runtime assertions." Switch to `unwrap_or_else(|| unreachable!())` at minimum, or restructure to `char_indices()` and drop manual byte tracking. |

### Tokenizer

| # | Where | What |
|---|---|---|
| W21 | `tokenizer.rs:46-54, 36-38` | `Token::Open.body_start: usize` and `Close.body_end: usize` co-exist with `SourcePosition.offset: u32` in the same struct hierarchy. `offset_u32` narrowing carries an `.expect()`. Introduce a `ByteOffset(u32)` newtype (C-NEWTYPE) shared across `SourcePosition`, `Range`, and the token fields. |
| W22 | `tokenizer.rs:289-346, 443-453` | `parse_attribute` reports `line: start_line` for unterminated values — points at the attribute name far above the actual EOF. Thread a `value_open_line` and use it. |
| W23 | `tokenizer.rs:364-382` | `record_seen_attr` allocates a fresh `HashSet<String>` and clones every key. Switch to `HashSet<&str>` borrowing from `attrs` (lifetime is bounded by `parse_attribute_list`'s frame). |

### Public API surface

| # | Where | What |
|---|---|---|
| W24 | `escape.rs:141-149` vs `lib.rs:39` | `is_name_start` and `is_name_char` are `pub` in the private module but not re-exported; the module rustdoc names them as part of the public API. Either add to `lib.rs:39` or downgrade to `pub(crate)`. **Ask first** — public API change. |
| W25 | `Cargo.toml:22` | `once_cell = "1.19"` is declared in `[workspace.dependencies]` and used **nowhere**. Remove. (Since 1.70 `std::sync::OnceLock` and 1.80 `LazyLock` cover the common cases anyway.) |
| W26 | `schema.rs:118-123` | `pub struct AttrConstraint` has `pub(crate)` fields and no accessors. Users can construct but not inspect — ergonomics hole. Add `pub fn kind(&self) -> &AttrKind` + `pub fn is_required(&self) -> bool`. **Ask first**. |
| W27 | `types.rs:67` | `ElementRef<'a>` missing `PartialEq`/`Eq`. The underlying `ElementData` derives both. Add the derives (and decide pointer-identity vs value-equality semantics — value-equality is probably what users want). **Ask first**. |

---

## INFOs (defer; mention but don't push)

- **B/parser**: `docs/dsl/selectors.md:84` says `:not(:not(...))` is rejected, but `parser.rs:227-243` allows nested `:not` up to `MAX_NOT_DEPTH = 64`. Decide: tighten the parser, or soften the doc.
- **C/serialize**: `Markdown::to_xml` re-serializes (it's a serializer, not a mutator — the architecture doc covers this correctly) but the public doc on `to_xml` doesn't warn that round-tripping the output through `parse` invalidates any stored `SourcePosition` from the original parse. One-line doc note.
- **E/Cargo**: No `[package.metadata.docs.rs]` section. Cost-free to add.
- **Edition 2024**: workspace is on 2021. Migration is non-breaking after the MSRV bump to ≥1.85. Visible wins: RPIT lifetime capture would drop `+ 'a` noise from `types.rs:96, 121, 163`. Not urgent.
- **G/mutate**: `apply_splices` silently buckets out-of-bounds or reversed ranges into `skipped_overlaps`. These are internal invariant failures, not selector overlaps. Use `debug_assert!` for these paths, or add a distinct `skipped_invalid` counter to `MutationReport`.
- **F/lib.rs:9-10 top-of-file comment**: claims "no per-call reparse" — true for the document, false for the selector. Update the comment when E2 lands.
- **A/tokenizer**: `try_skip_comment` / `try_skip_cdata` could share a `try_skip_bracketed` helper. Defer until a third bracketed trivia form lands (PI, DOCTYPE).
- **F/index.js**: napi-rs-generated `index.js` references `marxml.win32-x64-msvc.node` despite win32 being intentionally excluded from `napi.targets`. Cosmetic — generated code.

---

## What was checked and judged clean

Every reviewer was asked to write a `## Checked` block. Aggregated coverage:

| File | Lines | Coverage |
|---|---|---|
| `crates/marxml/src/lib.rs` | 1–49 (full) | E, G + spot-checks from A,B,C,D,F |
| `crates/marxml/src/tokenizer.rs` | 1–549 (full) | A (deep state-machine walk), G |
| `crates/marxml/src/parse.rs` | 1–249 (full) | B (deep), G |
| `crates/marxml/src/document.rs` | 1–220 (full) | B, E, F, G |
| `crates/marxml/src/types.rs` | 1–272 (full) | A, B, C, E, G |
| `crates/marxml/src/error.rs` | 1–207 (full) | C, D, E, G |
| `crates/marxml/src/escape.rs` | 1–308 (full) | A, B, C, E, G |
| `crates/marxml/src/mutate.rs` | 1–362 (full) | C (deep), E, F (spot), G |
| `crates/marxml/src/serialize.rs` | 1–326 (full) | C (deep), E, G |
| `crates/marxml/src/schema.rs` | 1–428 (full) | D (deep), E, G |
| `crates/marxml/src/validate.rs` | 1–288 (full) | D (deep), E, G |
| `crates/marxml/src/selector/{mod,ast,error,parser,matcher}.rs` | full | B (deep), G |
| `bindings/node/src/lib.rs` | 1–451 (full) | F (deep), G |
| `bindings/node/marxml.{mjs,d.ts}` | full | F (deep), G |
| `bindings/node/package.json`, `Cargo.toml`, `bindings/node/Cargo.toml` | full | E, F, G |
| `bindings/node/index.{js,d.ts}` | full | F (spot) |
| `docs/ARCHITECTURE.md` | 1–147 (full) | A, B, C, G |
| `docs/dsl/selectors.md` | 1–147 (full) | B, G |

**Clean explicitly verified:**

- **UTF-8 safety across the tokenizer** (A). Every `&str` slice site uses byte indices that are forced onto codepoint boundaries by ASCII-only name predicates and ASCII-only value-scan stop bytes. `\n` line counting on raw bytes is correct because `0x0A` cannot appear inside a UTF-8 multi-byte sequence.
- **Same-tag nesting** (A). Walked `<task><task>inner</task></task>` byte-by-byte; tokenizer is correct, nesting is the parser's responsibility (`parse.rs:98-187`) which uses a typed stack with name match.
- **EOF behavior in every tokenizer state** (A). No reachable panic on any input `parse::parse` will accept.
- **Byte-offset bookkeeping** (A). Spans are consistently half-open `[start, end)`. CDATA trivia ranges cannot underflow.
- **No regex in the tokenizer** (A, G). Confirmed clean.
- **Parser stack underflow / unclosed-tag handling** (B). Deterministic via `let Some(frame) = stack.pop() else { return Err(StrayClose) }`. No `unwrap`/`expect` in the hot path.
- **`Document::select` compile-once contract on the Rust side** (B). All five mutator entry points take `&Selector`, never `&str`. `#[must_use]` correctly on every pure-return mutator.
- **Selector parser & matcher core** (B). Descendant vs child semantics correct; dedup via pointer identity bounded to union selectors only; recursion bounded by `MAX_DEPTH = 1024`.
- **Mutator byte-preserving splice contract** (C). Verified in `splice_content`, `splice_regex`, `try_update`, `apply_splices`. Untouched ranges are always copied verbatim from `raw`.
- **Escape table** (C). All five XML entities present, `&amp;` first in every match (no double-escape), `&apos;` correctly not emitted in attribute context, numeric character references decoded with code-point validation.
- **Schema regex compile-once** (D). Compiled in `SchemaBuilder::build`, stored as `Regex` in `CompiledAttrKind`, never recompiled per validation call.
- **Validation engine** (D). Walks every node, aggregates all errors (no short-circuit), per-error `line: u32` sourced from `SourcePosition`. Behavior matches docs.
- **`thiserror` discipline** (D, E). All variants structured; no `String` catch-all. `#[non_exhaustive]` on every public error enum.
- **lib.rs re-export coverage** (E). Every type reachable through a `pub` method is re-exported. Only gap is `is_name_start`/`is_name_char` (W24).
- **MSRV `rust-version = "1.75"` correctness** (E). Most-recent stable API used is `Option::is_some_and` (1.70). Conservative.
- **Workspace lints flow to member crate** (E). `crates/marxml/Cargo.toml:17` sets `[lints] workspace = true`. The `marxml-node` binding crate has its own `[lints.clippy]` block with a documented justification (napi-derive FFI glue).
- **Two-track API parity for the common surface** (F). 22-row table comparing `marxml::Markdown` to `NativeMarkdown`/`MarkdownDoc` — basic mutator/serializer surface is covered; gaps are the `*_report` variants and the missing compiled-handle classes for `Selector` and `Schema`.
- **Public Node API correctly hides `NativeMarkdown`** (F, G). `marxml.mjs` factory shape is sound; `index.js`/`index.d.ts` are correctly *not* the documented user surface.
- **No `unsafe` anywhere in the crate code** (workspace `unsafe_code = "forbid"` confirmed across the survey).

---

## Independent verification log

For every ERROR finding I read the cited code at the cited line and confirmed the claim. One Codex finding was invalidated.

| Finding | Cited location | Reproduced? | Notes |
|---|---|---|---|
| E1 doc-vs-code splice sort | `mutate.rs:329`, `ARCHITECTURE.md:92` | Yes | Code sorts ascending with descending tie-break on end. Doc says descending. |
| E2 Node selector recompile | `bindings/node/src/lib.rs:211, 227, 243, 251, 265` | Yes | Every method opens with `parse_selector(&selector)?`. No `Selector` class exposed. |
| E3 matcher.rs unjustified allow | `selector/matcher.rs:62` | Yes | Bare `#[allow(clippy::too_many_arguments)]`. No comment. |
| E4 node lib.rs unjustified allows | `bindings/node/src/lib.rs:18-20` | Yes | Three bare `#![allow]`s. Design notes block above does not justify them by name. |
| E5 html_root_url stale | `crates/marxml/src/lib.rs:23` | Yes | Hardcoded `0.0.0`; workspace at `0.1.3`. |
| ~~G/parser:297 integer-error context~~ | `selector/parser.rs:288-316` | **No — invalidated** | Codex claimed both overflow paths route through `syntax_error("integer out of range")`. Actual code emits `SyntaxKind::IntegerOutOfRange` and `SyntaxKind::ExpectedDigit` directly. Dropped from findings. |
| once_cell unused | `Cargo.toml:22` | Yes | `grep -rn once_cell` finds it only in the workspace manifest. |
| mutate.rs `try_*` misnaming | `mutate.rs:142, 150` | Yes | Both return `MutationReport`, not `Result`. |
| `splice_regex` dead wrapper | `mutate.rs:177-184` | Yes | One-line pass-through to `splice_regex_with`. |
| AttrConstraint no accessors | `schema.rs:118-123` | Yes | `pub(crate)` fields, no getter methods. |

---

## Suggested fix order (as written at audit time)

1. **E1 + E5** — doc/version one-liners. ~5 minutes each.
2. **E3 + E4** — add justification comments to the three `#[allow]` sites. ~10 minutes.
3. **W25 (`once_cell`)** — delete the workspace dep line. ~1 minute.
4. **E2 + W4 (Node `Selector` and `Schema` compiled classes)** — one feature. Justifies a changeset and a minor bump. Mirror the Rust crate's shape.
5. **W18 + W23 (capacity hints, borrow-not-clone)** — hot-path microopts. Bench first under criterion / CodSpeed; merge if the numbers show >2% wins.
6. **W10 + W11 (ValidationError + ValidationReport ergonomics)** — schedule for the same release if you're already touching the binding for #4 (changeset already required).
7. **Everything else** — backlog with the report.
