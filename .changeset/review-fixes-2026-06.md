---
default: minor
---

#### Review sweep: doc/code parity, idiom fixes, typed `InvalidAttrKind`

Multi-reviewer audit pass (6 Claude agents + 1 Codex independent run, verified against Rust 1.95 / Rust API Guidelines / napi-rs v3). Findings cluster into doc/code contradictions, internal cleanup, and one public API tightening.

**Public API (minor):**
- `ValidationError::InvalidAttr.reason: String` → `kind: InvalidAttrKind`. The new typed enum carries the allowed-enum list or the regex pattern as structured fields callers can `match` on, instead of burying them in a diagnostic string. `Display` output is byte-identical to the prior format, so log lines, snapshot tests, and human-facing messages are unchanged. `InvalidAttrKind` is re-exported at the crate root. Breaking only for consumers that constructed `ValidationError::InvalidAttr` directly or destructured the `reason` field.

**Doc/code parity fixes:**
- `docs/ARCHITECTURE.md` Mutation section now describes the *ascending* splice sort with a forward cursor (matches the implementation in `mutate::apply_splices`). The previous "descending" wording was correct only for an in-place `String::replace_range` strategy that this crate doesn't use.
- `docs/dsl/selectors.md` correctly describes nested `:not()` as permitted-up-to-depth-64 (matches the parser); the prior "rejected" wording was misleading.
- `Markdown::to_xml` rustdoc now explicitly warns the output is re-serialized, not byte-preserved, and that `SourcePosition` values from the parsed document don't apply to the output.
- Removed stale `#![doc(html_root_url = "https://docs.rs/marxml/0.0.0")]` that broke cross-crate doc links after the first published release.
- Node binding regex-flags docstring now describes the `RegexBuilder` setter mechanism (matches the implementation); the prior `(?flags:…)` prefix claim was wrong about the mechanism though correct about observable behavior.
- Node binding `toJson` JSDoc no longer claims "no string round-trip" — the wrapper hides one inside `marxml.mjs` and that's worth being honest about.
- Crate-root design notes in `bindings/node/src/lib.rs` clarify "no per-call reparse" applies to the document, not the selector (selector recompile-once is roadmap work).

**`#[allow]` justifications (the repo Never rule on unjustified suppressions):**
- `selector/matcher.rs::walk` — `clippy::too_many_arguments` now carries a comment naming the trade-off (no indirection on the hot recursive call) and pointing at criterion as the arbiter.
- `bindings/node/src/lib.rs` — the three crate-level `#![allow]`s (`needless_pass_by_value`, `missing_errors_doc`, `missing_panics_doc`) each get a per-lint rationale comment.

**Internal cleanup:**
- Renamed `mutate::try_replace_content` / `try_replace_in` → `splice_content_report` / `splice_regex_report`. The `try_*` prefix is reserved for fallible operations; these always return a `MutationReport` so they're not fallible. Internal (`pub(crate)`); public re-export names (`replace_content_report`, `replace_in_report`) unchanged.
- Removed the dead `splice_regex` one-line wrapper. `replace_in` / `splice_regex_report` now call `splice_regex_with` directly.
- Dropped the unused `once_cell = "1.19"` workspace dependency (no source file referenced it).
- Added `[package.metadata.docs.rs]` to `crates/marxml/Cargo.toml` for future feature-gated items.

**Microopts (no benchmark required — straight wins):**
- `mutate::rewrite_open_tag` now allocates with `String::with_capacity(...)` sized from the tag + existing-attr + new-attr byte budget.
- `serialize::to_xml` now allocates with `String::with_capacity(doc.raw().len())`.
- `tokenizer::record_seen_attr` builds the lazy `HashSet` with `iter().map().collect()` instead of `with_capacity` + a manual loop.
- `escape::decode_entities` replaced `.chars().next().expect("non-empty tail")` with `let Some(ch) = ... else { unreachable!(...) }` — same runtime behavior, expressed via the type system.

**Diagnostic fix:**
- `ParseError::MalformedAttribute { kind: UnterminatedValue, .. }` now reports the line at which input ran out, not the line of the attribute name. For attributes whose values span multiple lines and run off the end, the error now points at the actual EOF rather than far above. One snapshot updated accordingly.

**New contributor doc:**
- `contributing/rust-best-practices.md` codifies the rubric the review pass used: Rust idiom checklist, napi-rs v3 specifics, what 2024 edition would change (deferred), reviewer severity levels. Companion to AGENTS.md's hard rules.

### Deferred (Ask First — not in this changeset)

These came out of the same review pass but each requires a design call before implementation. Tracked for follow-up:

- Compile-once `Selector` / `Schema` napi classes (Node API addition).
- Async `parse` / mutator variants for multi-MB inputs.
- `linux-arm64-musl` napi target.
- `SelectorError::UnexpectedEnd { at: usize }` field add.
- `SerializeOpts::self_close_empty()` builder rename to remove field/method shadow.
- `is_name_start` / `is_name_char` visibility decision (re-export vs `pub(crate)` downgrade).
- `ElementRef` `PartialEq` / `Eq` derives + semantic decision (pointer vs value).
- `AttrConstraint` accessor methods.
- Edition 2024 migration + MSRV bump to 1.85.
