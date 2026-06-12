# Reviewer F — napi-rs Node binding

Model: Opus. Scope: `bindings/node/src/lib.rs`, `marxml.mjs`, `marxml.d.ts`, `package.json`, the napi-rs raw output (`index.js`, `index.d.ts`) as confirmation, and the Rust public API for parity comparison.

## Two-track API parity table

Comparing public surfaces of `marxml::Markdown` (in `crates/marxml/src/document.rs` + `crates/marxml/src/lib.rs`) against `NativeMarkdown` (in `bindings/node/src/lib.rs`) and the JS factory product `MarkdownDoc` (in `marxml.mjs` / `marxml.d.ts`).

| Rust method (`crates/marxml`) | Node method (`marxml.mjs` / `marxml.d.ts`) | Status |
|---|---|---|
| `parse(&str)` (`parse.rs:47`) | `parse(source: string)` (`marxml.mjs:17`) | OK |
| `parse_fragment(&str)` (`parse.rs:60`) | — | Missing — INFO (currently identical to `parse`, by design) |
| `parse_owned(String)` (`parse.rs:71`) | — | Missing — INFO (perf shortcut, not a semantic gap) |
| `Markdown::raw()` (`document.rs:54`) | `doc.raw` getter (`marxml.mjs:23`) | OK |
| `Markdown::root_elements()` (`document.rs:59`) | `doc.elements` getter (`marxml.mjs:26`) | OK (materializes — see WARN below) |
| `Markdown::root_count()` (`document.rs:69`) | — | Missing — INFO (JS can read `elements.length`, but that re-walks the tree) |
| `Markdown::select(&Selector)` (`document.rs:85`) | `doc.select(selector: string)` (`marxml.mjs:29`) | Diverges — selector re-parsed every call (see ERROR below) |
| `Markdown::update(&Selector, &[(&str,&str)])` (`document.rs:111`, panicking variant) | — | Missing — INFO (Rust crate intentionally hides this; Node exposes only the `try_*` flavor — correct call) |
| `Markdown::try_update(...)` (`document.rs:155`) | `doc.updateAttrs(selector, AttrUpdate[])` (`marxml.mjs:32`, via `lib.rs:227`) | OK, but loses the `MutationReport` (applied/skipped counts) — see WARN |
| `Markdown::replace_content(&Selector, &str)` (`document.rs:118`) | `doc.replaceContent(selector, newBody)` (`marxml.mjs:35`) | OK |
| `Markdown::replace_content_report(...)` (`document.rs:168`) | — | Missing — WARN (report counts are unreachable from JS) |
| `Markdown::replace_in(&Selector, &Regex, &str)` (`document.rs:128`) | `doc.replaceInContent(selector, pattern, replacement)` (`marxml.mjs:41`) | OK |
| `Markdown::replace_in_report(...)` (`document.rs:176`) | — | Missing — WARN |
| `Markdown::replace_text(&Selector, &str)` (`document.rs:136`) | `doc.replaceText(selector, newText)` (`marxml.mjs:38`) | OK |
| `Markdown::replace_text_in(&Selector, &Regex, &str)` (`document.rs:143`) | — | Missing — WARN (escape-then-regex-replace; commonly needed for user input) |
| `Markdown::to_xml(&SerializeOpts)` (`document.rs:190`) | `doc.toXml(opts?)` (`marxml.mjs:48`) | OK |
| `Markdown::to_json() -> serde_json::Value` (`document.rs:207`) | `doc.toJson(): unknown` (`marxml.mjs:51`) | OK, but double-serializes (see WARN) |
| `validate(&Markdown, &Schema)` (`crates/marxml/src/lib.rs:46`) | `doc.validate(schema)` (`marxml.mjs:54`) | OK |
| `Schema::builder()` + `try_build` (`lib.rs:42`, `schema.rs`) | inlined into `validate`'s POJO arg (`lib.rs:376-415`) | Diverges — Rust API is a compiled, reusable schema; Node rebuilds it on every `validate` call (see WARN) |
| `escape_attr`, `escape_text`, `is_valid_name` (`lib.rs:39`) | — | Missing — INFO (low-priority helpers; users could use the `replaceText` shortcut instead) |
| `MAX_DEPTH`, `MAX_INPUT_BYTES` (`lib.rs:41`) | — | Missing — INFO (diagnostic constants) |
| `VERSION` (`lib.rs:49`) | — | Missing — INFO (Node has `package.json.version`) |

## ERROR — `Markdown.select` violates the compile-once selector contract

- Location: `bindings/node/src/lib.rs:210-218`, `bindings/node/marxml.mjs:29-31`, `bindings/node/marxml.d.ts:51`
- What: Every Node-side `doc.select("css")` / `doc.updateAttrs("css", ...)` / `doc.replaceContent("css", ...)` / `doc.replaceText("css", ...)` / `doc.replaceInContent("css", ...)` calls `parse_selector(&selector)` at the top of the method. The selector AST is rebuilt and thrown away on every invocation.
- Why it matters: `AGENTS.md` calls `Selector::parse` "the hot-path API. Compiled once, reused many. The benches assume this. Don't introduce a `select_str(src, "css")` convenience that re-parses on every call as the primary API." The Node binding currently *only* exposes the re-parsing form, so any JS workflow that runs the same selector across many docs (or many mutator calls on the same doc) pays the parse cost every time — a direct contradiction of the documented contract.
- Suggested fix: Expose a `#[napi]` class `Selector` wrapping `marxml::Selector`, with `Selector.parse(css: string) -> Selector` as the factory. Add overloads on `select` / `updateAttrs` / `replaceContent` / `replaceText` / `replaceInContent` that accept either a `string` (current behavior) or a `Selector` instance (compiled). Keep the string path for ergonomics; document the `Selector` form as the perf path in `marxml.d.ts`.

## ERROR — `parse` / mutators block the Node event loop on large documents

- Location: `bindings/node/src/lib.rs:328-333` (`parse`), `lib.rs:211, 227, 243, 251, 265, 279, 292, 305` (every method)
- What: `parse` is `#[napi] pub fn parse(source: String) -> Result<NativeMarkdown>` — synchronous. Same for every method on `NativeMarkdown`. `marxml::parse` accepts inputs up to `MAX_INPUT_BYTES = u32::MAX - 1` ≈ 4 GiB (`crates/marxml/src/parse.rs:37`). A multi-MB or multi-GB document parses on the Node main thread and freezes the event loop.
- Why it matters: Sync calls block the Node event loop on multi-MB inputs. Server-side users (the Node binding's most likely audience) will stall on a single large input.
- Suggested fix: Add `parseAsync(source: string): Promise<MarkdownDoc>` (in addition to keeping `parse` for small docs / scripts). napi-rs v3 makes this trivial: change the signature to `pub async fn parse_async(source: String) -> Result<NativeMarkdown>` and it returns a JS `Promise` automatically. The expensive mutator paths (`update_attrs`, `replace_content`, `replace_in_content`, `to_xml`) deserve async variants for the same reason.

## WARN — No `From<marxml::*Error> for napi::Error` impls; every call site repeats stringified `.map_err`

- Location: `bindings/node/src/lib.rs:236, 294, 332, 338, 364, 373, 414`
- What: There is no `From<ParseError> for napi::Error`, no `From<SelectorError>`, no `From<MutateError>`, no `From<SchemaError>`. Every fallible call site closes with the same `.map_err` boilerplate that throws away the structured error and stringifies it. `Status` is hardcoded to `InvalidArg` everywhere except `to_json` (`GenericFailure`).
- Suggested fix: One `From` impl per crate error type (or an `IntoNapi` extension trait, since the orphan rule blocks the direct From). Then `parse_selector` becomes `marxml::Selector::parse(s).map_err(Into::into)` and the call sites lose the closures.

## WARN — `toJson` double-serializes through a JSON string

- Location: `bindings/node/src/lib.rs:291-295`, `bindings/node/marxml.mjs:51-53`, `bindings/node/marxml.d.ts:100-107`
- What: The napi method returns a `String` produced by `serde_json::to_string(&self.inner.to_json())`. The JS wrapper then calls `JSON.parse(m.toJson())`. Two full passes over the tree-as-JSON: once on Rust serializing, once on V8 parsing.
- Suggested fix: Add the `napi = { version = "3", features = ["serde-json", ...] }` feature and change `to_json` to `pub fn to_json(&self) -> serde_json::Value`. Drop the `JSON.parse` from `marxml.mjs`.

## WARN — Docstring says JS regex flags map via `(?flags:…)` prefix; implementation uses `RegexBuilder` setters

- Location: `bindings/node/src/lib.rs:259` (the doc claim) vs `bindings/node/src/lib.rs:346-370` (the implementation)
- What: The doc comment on `replace_in_content` (lines 256-263) says "JS regex flags `i`/`m`/`s`/`x` are honored via Rust's `(?flags:…)` prefix". `compile_regex` (lines 341-374) actually configures a `RegexBuilder` with `.case_insensitive(true)` etc. — no `(?…:…)` rewriting happens.
- Why it matters: Doc claim contradicts behavior. User-observable behavior is identical; the misstatement is purely about the mechanism. Still worth fixing because the next maintainer might "simplify" by removing the `RegexBuilder` setup and trusting the (nonexistent) prefix.

## WARN — `package.json` `napi.targets` missing `aarch64-unknown-linux-musl`

- Location: `bindings/node/package.json:49-55`
- What: Five targets are built/published: macOS x64 + arm64, linux-gnu x64 + arm64, linux-musl x64. linux-musl arm64 (`aarch64-unknown-linux-musl`) is absent.
- Why it matters: Alpine on ARM (AWS Graviton + Alpine container images is a common combo) will fail to load any binary.
- Suggested fix: **Ask first** per `AGENTS.md`. If approved: add `aarch64-unknown-linux-musl` to `napi.targets`, scaffold `bindings/node/npm/linux-arm64-musl/`, and verify the CI cross-compile matrix in `.github/workflows/release.yml` picks it up.

## WARN — `NativeMarkdown.elements` getter re-walks and re-materializes the whole tree on every JS read

- Location: `bindings/node/src/lib.rs:200-206`, JSDoc on `marxml.d.ts:36-40`
- What: The getter calls `self.inner.root_elements().map(|e| Element::from_ref(&e)).collect()` every access. `Element::from_ref` (lines 69-93) recursively clones tag names, attribute keys/values, content slices, and span data into POJO `String`s. Each `doc.elements` access pays the full tree-clone cost.
- Why it matters: Idiomatic JS APIs treat property reads as cheap. A user looping `for (const el of doc.elements)` looks fine but actually re-clones the entire tree per iteration if they read `doc.elements` repeatedly. The docstring does warn ("cache the result if you read it repeatedly") — kudos for honesty — but the *getter shape* is the trap.
- Suggested fix: Cache the materialization in `NativeMarkdown` (`OnceCell<Vec<Element>>` — doc is immutable so invalidation is moot); or rename the getter to a method to signal cost.

## WARN — `validate` rebuilds the `Schema` on every call

- Location: `bindings/node/src/lib.rs:303-321`, `lib.rs:376-415`
- What: The JS `validate(schema)` accepts a POJO. Inside the binding, `build_schema` rebuilds a `marxml::Schema` from scratch per call (running `try_build`, recompiling any embedded regex patterns). The Rust crate exposes `Schema::builder()` precisely so users *can* build once and reuse.
- Suggested fix: Expose a `#[napi]` class `Schema` with `Schema.build(decl) -> Schema` factory. Overload `validate` to accept `Schema` (compiled, fast) or `Record<string, TagSchemaShape>` (sugar that compiles inline). Mirrors the Selector fix above.

## WARN — `updateAttrs` discards the `MutationReport`

- Location: `bindings/node/src/lib.rs:227-237`
- What: `try_update` returns `MutationReport { output, applied, skipped }`. The binding returns only `report.output` to JS.
- Suggested fix: Either change the return shape to `{ output: string; applied: number; skipped: number }`, or expose a sibling `updateAttrsReport` (mirrors `replace_content_report` / `replace_in_report` in the crate).

## WARN — `replace_content`, `replace_text`, `replace_in_content` declare `Result<String>` but never fail

- Location: `bindings/node/src/lib.rs:243-247, 251-254, 265-274`
- What: These three methods have `Result<String>` return types, but the only fallible step is `parse_selector`. Once that succeeds, `self.inner.replace_content(...)` / `replace_text(...)` / `replace_in(...)` are infallible (they return `String`, not `Result`).
- Suggested fix: Falls out for free when the From-impls fix lands.

## WARN — `selector: String` / `new_body: String` / etc. take owned strings unnecessarily

- Location: `bindings/node/src/lib.rs:211, 227, 243, 251, 265, 329`
- What: Every public method takes its string arguments as owned `String`. napi-rs v3 supports `&str` for some signatures. The crate-side calls all want `&str` anyway, so the owned `String`s are consumed by `.as_str()` / `&` calls.
- Why it matters: One extra heap allocation per JS→Rust string crossing for arguments that are then borrowed. For `selector` and `new_body` the win is small; for `parse(source: String)` on a multi-MB document it's a real copy.
- Suggested fix: Verify in napi-rs v3 docs whether `&str` is now supported for arguments. If yes, change all `String` params to `&str`. Low confidence — verify against current examples before refactoring.

## WARN — JSDoc on `MarkdownDoc.toJson` claims "no string round-trip needed at the call site"

- Location: `bindings/node/marxml.d.ts:100-107`, `bindings/node/marxml.mjs:51-53`
- What: The doc says "already parsed; no string round-trip needed at the call site." Technically accurate (the *call site* sees a parsed value) but misleading — the round-trip happens inside the wrapper (`JSON.parse(m.toJson())`).

## INFO — Rust 2024 edition migration not done; workspace still on 2021

- Location: `bindings/node/Cargo.toml:7` inherits `edition.workspace = true`

## INFO — `#[allow(clippy::needless_pass_by_value)]` etc. at crate root without per-site justification

- Location: `bindings/node/src/lib.rs:18-20`
- What: Three blanket `#[allow]`s at crate root: `needless_pass_by_value`, `missing_errors_doc`, `missing_panics_doc`. The first two are because napi takes owned types and the binding intentionally undocs its errors as bridge code. The third is because `#[napi]`-derived code may panic-on-FFI-error.
- Suggested fix: Move the rationale comment from `Cargo.toml:25-27` to a doc-style comment immediately above the `#![allow]` block in `lib.rs`.

## INFO — `error_tag` clones the tag `String` even though `e` is `&ValidationError`

- Location: `bindings/node/src/lib.rs:430-439`

## INFO — `index.js` references `marxml.win32-x64-msvc.node` but win32 is disabled

- Location: `bindings/node/index.js:110-179`
- What: The loader generated by napi-rs still includes a full win32 branch. `package.json.napi.targets` excludes win32, so no binary exists for those `require` calls — they fall through to `loadErrors` and the loader eventually throws. Generated code — fine to leave.

## Checked

- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/src/lib.rs` — full file (1-451)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/marxml.mjs` — full file (1-81)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/marxml.d.ts` — full file (1-126)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/package.json` — full file (1-62)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/index.js` — full file (1-585) for loader + export verification
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/index.d.ts` — full file (1-203) for raw type surface verification
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/Cargo.toml` — full file (1-35)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/crates/marxml/src/lib.rs` — full file (1-50)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/crates/marxml/src/document.rs` — public surface, lines 1-219
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/crates/marxml/src/parse.rs` — entry signatures, lines 1-80
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/crates/marxml/src/mutate.rs` — `replace_in`, `NoExpand`, `replace_text_in`, lines 95-210
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/__test__/binding.test.ts` — lines 1-220 (sampled to confirm `parse`, `select`, `updateAttrs`, `toJson`, `validate` covered; no test for compiled selector or async path because neither exists)
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/bindings/node/npm/<target>/package.json` — spot-checked `linux-arm64-gnu`, `linux-x64-musl`, `win32-x64-msvc`
- `/Users/zacrosenbauer/Code/thebytefarm/marxml/.github/workflows/release.yml` — grep for `pre-publish`

**Conclusion:** Two ERROR-grade issues — Node binding re-parses every selector on every call (violates documented compile-once contract); `parse` and every mutator are synchronous and block the Node event loop. Notable WARN-grade gaps in error mapping, JSON FFI shape, missing napi target, and report-variant coverage.
