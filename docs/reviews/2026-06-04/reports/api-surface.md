# Reviewer E — API surface, error enum, shared types

Model: Sonnet. Scope: `crates/marxml/src/lib.rs`, `error.rs`, `types.rs`, the workspace + crate `Cargo.toml`, and `README.md`.

## lib.rs — Re-exports and Crate-Level Docs

### WARN — `is_name_start` and `is_name_char` are `pub` in `escape.rs` but not re-exported

- Location: `escape.rs:141-149`, `lib.rs:39`
- What: `is_name_start(b: u8) -> bool` and `is_name_char(b: u8) -> bool` are marked `pub` inside the private `mod escape` module. Because the module is private (`mod escape;` at `lib.rs:29`), they are already unreachable to external consumers, so there is no soundness or leak problem. The issue is the mismatch in intent: the module-level rustdoc at `escape.rs:10` explicitly names them as part of the API ("The name predicates (`is_name_start`, `is_name_char`, `is_valid_name`) are the single source of truth…"), but only `is_valid_name` is forwarded via `lib.rs:39`. Downstream users who want to test individual bytes against the grammar have no public primitive and must replicate the logic.
- Why it matters: Minor ergonomics gap for library consumers who handle XML name validation at the byte level; the stale intent comment in escape.rs will also confuse future contributors.
- Suggested fix: Either (a) add `is_name_start` and `is_name_char` to the re-export on `lib.rs:39` (`pub use escape::{escape_attr, escape_text, is_name_start, is_name_char, is_valid_name};`) or (b) downgrade both to `pub(crate)` and remove them from the module rustdoc if they are deliberately not part of the public surface. **Ask first** — this changes a published API.

### WARN — `html_root_url` is pinned to `0.0.0`, not the package version

- Location: `lib.rs:23`
- What: `#![doc(html_root_url = "https://docs.rs/marxml/0.0.0")]` uses a hardcoded version string that will never match the actual published version. After release `0.1.3`, cross-crate `[`Link`]` hyperlinks from other crates pointing into marxml's docs resolve to the `0.0.0` URL, which does not exist on docs.rs.
- Why it matters: Any crate that links to `marxml` docs by version will silently 404.
- Suggested fix: Use `env!("CARGO_PKG_VERSION")` in a `const` and inject it, or simply **remove the attribute** — `html_root_url` is largely superseded by docs.rs's own link rewriting.

### INFO — `[package.metadata.docs.rs]` not present in the crate `Cargo.toml`

- Location: `crates/marxml/Cargo.toml` (no line — key is absent)
- What: The crate does not include a `[package.metadata.docs.rs]` section. Without it, docs.rs builds with default features only and uses its own feature inference. For this crate the impact is minimal because there are no optional features, but the section also lets you set `rustdoc-args = ["--cfg", "docsrs"]` for feature-gated items and `targets = [...]` to constrain the platform list.
- Suggested fix: Add a minimal block:
  ```toml
  [package.metadata.docs.rs]
  all-features = true
  rustdoc-args = ["--cfg", "docsrs"]
  ```

### Clean — Crate-level rustdoc completeness

Checked `lib.rs:1-21`. The module-level doc comment is present, explains the four pillars (parse, select, mutate, validate/serialize), links every public type by name, and includes a runnable rust example with an `# Ok::<()…>()` error-type annotation.

### Clean — Re-export completeness against public method return types

Audited every type that appears in the return position or as a parameter in a `pub` method or `pub fn` across `document.rs`, `mutate.rs`, `validate.rs`, `schema.rs`, `selector/mod.rs`, and `serialize.rs`:

| Type | Re-exported |
|---|---|
| `Markdown` | yes — `lib.rs:37` |
| `ParseError`, `MalformedAttrKind`, `MalformedTagKind` | yes — `lib.rs:38` |
| `escape_attr`, `escape_text`, `is_valid_name` | yes — `lib.rs:39` |
| `MutateError`, `MutationReport` | yes — `lib.rs:40` |
| `parse`, `parse_fragment`, `parse_owned`, `MAX_DEPTH`, `MAX_INPUT_BYTES` | yes — `lib.rs:41` |
| `AttrConstraint`, `AttrKind`, `Schema`, `SchemaBuilder`, `SchemaError`, `TagBuilder` | yes — `lib.rs:42` |
| `Selector`, `SelectorError`, `SyntaxKind` | yes — `lib.rs:43` |
| `SerializeOpts` | yes — `lib.rs:44` |
| `ElementRef`, `SourcePosition`, `SourceSpan` | yes — `lib.rs:45` |
| `validate`, `ValidationError`, `ValidationReport` | yes — `lib.rs:46` |
| `regex::Regex` | appears in `Markdown::replace_in` — is a **caller-supplied** argument, not returned; caller constructs it from the `regex` crate directly. Not a leak. |

All types reachable via the public API are properly re-exported. The one gap is `is_name_start`/`is_name_char` noted above (WARN).

## error.rs — `ParseError`, `MalformedTagKind`, `MalformedAttrKind`

### Clean — `#[non_exhaustive]` coverage

`ParseError` (`error.rs:11`), `MalformedTagKind` (`error.rs:146`), and `MalformedAttrKind` (`error.rs:179`) all carry `#[non_exhaustive]`. Adding variants in a future release will not be a breaking change for external `match` arms.

### Clean — One variant per logical failure

Each variant maps to a distinct, non-overlapping failure condition. No two variants describe the same parse state. The `MalformedTag`/`MalformedAttribute` indirection through `MalformedTagKind`/`MalformedAttrKind` sub-enums is the right call — it makes the top-level enum manageable while keeping failure modes machine-matchable.

### Clean — Structured context in every variant

Every variant carries `line: u32` plus at least one structured field (`tag: String`, `found/expected`, `attr`, etc.). `InputTooLarge` is the justified exception — it fires before a source position is established, and the doc comment explains why `line()` returns `None` for it (`error.rs:124`). No bare `String` error messages.

### Clean — `#[from]` usage

No transparent conversions via `#[from]` are needed here: `ParseError` only originates inside the parser, not by wrapping a foreign error type. Correct omission.

### Clean — `Debug`, `Clone`, `PartialEq`, `Eq` on all three enums

All three derive the full set (`error.rs:10`, `145`, `178`). `Hash` is absent, which is the right call.

### INFO — `line: u32` vs. a `SourcePosition` newtype

- Location: `error.rs:19-116` (every `line` field)
- What: The `line` field is a raw `u32` across all variants. `SourcePosition { line, offset }` already exists in `types.rs` and captures both line and byte offset. Callers who need column-level diagnostics get only the line number from the error, even though the tokenizer and parser have the full byte offset available at the point of failure.
- Why it matters: Downstream tooling (LSP servers, editor integrations) typically wants `line + column` or a byte offset to underline the precise character. Changing this is a breaking API change post-1.0 — worth recording before then.

## types.rs — `SourcePosition`, `SourceSpan`, `ElementRef`, `TextSegments`

### WARN — `ElementRef` missing `PartialEq` and `Eq`

- Location: `types.rs:67`
- What: `ElementRef<'a>` derives only `Debug, Clone, Copy` (`types.rs:67`). It does not derive `PartialEq`/`Eq`. The underlying `ElementData` does derive `PartialEq`/`Eq` (`types.rs:45`), and `ElementRef` contains only `&'a ElementData`, `&'a str`, and `&'a [Range<usize>]` — all `PartialEq`-capable. A caller who collects `doc.select(&sel)` into a `Vec<ElementRef<'_>>` and wants to check membership or dedup is blocked.
- Why it matters: C-COMMON-TRAITS (Rust API guidelines) says types returned from public iterators should derive `PartialEq`/`Eq` where they fit. This is the primary user-facing element type.
- Suggested fix: `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` on `ElementRef<'a>`. The pointee equality (`&'a ElementData` pointers being equal) may or may not be the intended semantic — value equality (same tag, same attrs, same content) is probably more useful. If pointer identity is ever the intent, document it explicitly. **Ask first**.

### WARN — `AttrConstraint` public struct with `pub(crate)` fields provides no read-back API

- Location: `schema.rs:118-123`
- What: `AttrConstraint` is a `pub struct` with two fields, `kind: AttrKind` and `required: bool`, both `pub(crate)`. Users can construct it via `AttrKind::required()`, `AttrKind::optional()`, or `From<AttrKind>`, but they cannot read the fields back. There are no accessor methods on `AttrConstraint`. A caller who builds a `Schema`, then later wants to inspect or log the constraints they registered, cannot round-trip the struct.
- Why it matters: The only reason to make `AttrConstraint` a `pub` type (rather than keeping it opaque via the builder closure) is to let users name it in function signatures and store it. A fully opaque struct with no read API is an API ergonomics hole.
- Suggested fix: Either add `pub fn kind(&self) -> &AttrKind` and `pub fn is_required(&self) -> bool` accessors, or make the struct properly opaque (`pub(crate) struct AttrConstraint`) and remove it from the re-export list. Given it is already in `lib.rs:42`, the accessors are the path of least resistance. **Ask first**.

### Clean — `SourcePosition` and `SourceSpan`

Both derive `Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord` (`types.rs:12`, `32`). `Hash` is present alongside `Eq`, satisfying the API guideline requirement. `Copy` is present and appropriate for these small value types. `pub` fields on `SourcePosition` are intentional and documented (`line` is 1-based, `offset` is 0-based byte). No issues.

### Clean — `TextSegments` encapsulation

`TextSegments` and `ElementData` are both `pub(crate)`, not re-exported. `ElementRef::text()` and `ElementRef::children()` return `impl Iterator<...>` so the concrete type is not part of the public surface. Correct.

### Clean — Newtype coverage for byte offsets

The only raw `usize` byte offsets in the public API are inside `Range<usize>` returned by `ElementRef::content_range()` — which is `pub(crate)`. `SourcePosition.offset` is a `u32` named field, not a raw positional integer. No bare `usize` byte offsets are crossing the public API boundary. The `u32` cap at `MAX_INPUT_BYTES` is documented and enforced at parse entry. No newtype gap.

## Cargo.toml (workspace root + crate manifest)

### WARN — `once_cell` is declared as a workspace dependency but never used

- Location: `Cargo.toml:22`
- What: `once_cell = "1.19"` appears in `[workspace.dependencies]` at the root manifest. Searching every `.rs` file under `crates/` and `bindings/` finds zero `use once_cell` or `once_cell::` references. Neither the `marxml` crate nor the `marxml-node` crate lists it in their `[dependencies]`.
- Why it matters: A ghost workspace dependency inflates the Cargo lock file, is pulled into `cargo vendor` outputs, and misleads future contributors about what the crate actually uses.
- Suggested fix: Remove `once_cell = "1.19"` from `[workspace.dependencies]`. Since Rust 1.70, `std::sync::OnceLock` and `std::sync::LazyLock` (1.80) cover the common `once_cell` use cases without a third-party dep.

### Clean — Workspace lints flow to the member crate

`crates/marxml/Cargo.toml:17` sets `[lints] workspace = true`. The `marxml-node` binding crate does **not** inherit workspace lints (it has its own `[lints.clippy]` block at `bindings/node/Cargo.toml:28-35`) — this is explicitly documented in that file as necessary because `napi-derive` expands to unsafe FFI glue. The carve-out is justified and does not weaken the core crate.

### Clean — MSRV declaration correctness

MSRV is declared as `rust-version = "1.75"` in the workspace manifest. The most recent stable API used by the crate is `Option::is_some_and`, stabilized in Rust 1.70. All other APIs in the codebase (`partition_point` → 1.52, `let-else` → 1.65, `BTreeSet` → 1.0, `String::with_capacity` → 1.0) predate 1.75. The MSRV declaration is conservative and correct.

### Clean — Dep version ranges

`thiserror = "2.0"`, `regex = "1.10"`, `serde = "1.0"`, `serde_json = "1.0"`. All are major-version-stable crates with no unstable feature usage. `thiserror 2.0` brought breaking changes from 1.x, and the code is correctly on the `2.0` series. No concerning semver ranges.

### INFO — Edition 2021 vs 2024

- Location: `Cargo.toml:10` (edition), `crates/marxml/Cargo.toml:4` (inherits)
- What: The crate is on edition 2021. Rust 2024 has been stable since Rust 1.85 (Feb 2025), and the toolchain in use is 1.95. Two 2024 idioms that would visibly simplify this codebase: (1) RPIT lifetime capture — `ElementRef::children()` and `ElementRef::text()` return `impl Iterator<Item = ...> + 'a` with explicit lifetime annotations on the bound (`types.rs:96`, `121`, `163`). In edition 2024, `impl Iterator<Item = ...>` automatically captures `'a` from scope, eliminating the `+ 'a` noise.
- Suggested fix: Record the migration opportunity; do not migrate now. When MSRV is bumped past 1.85, switch `edition = "2024"` and use `cargo fix --edition`.

## Checked

| File | Lines read |
|---|---|
| `crates/marxml/src/lib.rs` | 1–49 (full) |
| `crates/marxml/src/error.rs` | 1–207 (full) |
| `crates/marxml/src/types.rs` | 1–272 (full) |
| `Cargo.toml` (workspace root) | 1–48 (full) |
| `crates/marxml/Cargo.toml` | 1–36 (full) |
| `README.md` | 1–101 (full) |
| `crates/marxml/src/mutate.rs` | 1–362 (full, to verify `MutateError`/`MutationReport`) |
| `crates/marxml/src/validate.rs` | 1–288 (full, to verify `ValidationError`/`ValidationReport`) |
| `crates/marxml/src/schema.rs` | 1–429 (full, to verify `AttrConstraint`/`SchemaError`) |
| `crates/marxml/src/document.rs` | 1–221 (full, to verify method return types) |
| `crates/marxml/src/serialize.rs` | 1–326 (full, to verify `SerializeOpts`) |
| `crates/marxml/src/parse.rs` | 1–250 (full, to verify `MAX_DEPTH`/`MAX_INPUT_BYTES`) |
| `crates/marxml/src/escape.rs` | 1–308 (full, to verify `is_name_start`/`is_name_char` visibility) |
| `crates/marxml/src/selector/mod.rs` | 1–68 (full) |
| `crates/marxml/src/selector/error.rs` | 1–88 (full) |
| `crates/marxml/src/selector/ast.rs` | 1–52 (full, to verify internal visibility) |
| `bindings/node/Cargo.toml` | 1–35 (full, to verify lint carve-out justification) |

**Conclusion:** API surface in good shape — all types reachable through public methods are properly re-exported, every public error enum carries `#[non_exhaustive]` and structured (non-string) context fields, MSRV declaration of 1.75 is correct. Four actionable findings: visibility decision on `is_name_*`, stale `html_root_url`, zombie `once_cell` workspace dep, `AttrConstraint` with no read-back API.
