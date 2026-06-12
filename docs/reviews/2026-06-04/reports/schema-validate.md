# Reviewer D — schema + validate

Model: Sonnet. Scope: `crates/marxml/src/schema.rs`, `validate.rs`, with `error.rs`, `document.rs`, `types.rs` cross-checked.

## schema + validate subsystem

### WARN — `check_kind` returns `Option<String>` (stringly-typed reason) instead of a structured enum

- Location: `validate.rs:259-287`
- What: `check_kind` returns `Option<String>` describing why a constraint failed (`"expected one of [...]"`, `"did not match regex /…/"`). That string is embedded as the `reason` field of `ValidationError::InvalidAttr` (also a `String`). The failure mode is already machine-representable — there are exactly two constraint types that can fail (`Enum` and `Regex`), and all the data needed to describe them is already present in `CompiledAttrKind`.
- Why it matters: Callers who want to act on constraint type (e.g., surface different UI for "wrong enum value" vs. "failed regex") must parse the string, which is fragile and an API guideline violation (structured fields preferred over string payloads). The enum values are lost in the message; they're formatted into the string and not recoverable.
- Suggested fix: Introduce an `AttrFailure` enum (`InvalidAttrReason::NotInEnum { allowed: &BTreeSet<String> }` / `InvalidAttrReason::NoRegexMatch { pattern: &str }`) and return it from `check_kind`. Place it in the `reason` field of `ValidationError::InvalidAttr` (replacing `String`). The `Display` impl on that enum produces the existing human messages. This is a public API change — requires a changeset.

### WARN — `ValidationError::InvalidAttr::reason` is a `String` hiding structured info already available

- Location: `validate.rs:32-42`
- What: The `reason: String` field in `ValidationError::InvalidAttr` receives the output of `check_kind`, which formats either "expected one of [...]" or "did not match regex /…/". The original enum variant and allowed values are thrown away after the format call.
- Why it matters: Same consequence as the `check_kind` finding above. This is the public error type (`ValidationError` is re-exported); once shipped, the `String` field locks in an opaque interface.
- Suggested fix: Replace `reason: String` with `reason: AttrFailureReason` (or inline `InvalidAttrKind`). The `Display` impl on that type produces the existing rendered message. This is a public API change — changeset required.

### WARN — `check_kind` builds the enum error message with a manual loop instead of using iterators

- Location: `validate.rs:266-275`
- What: The `Enum` branch manually iterates `allowed`, manages a `first` boolean flag, and pushes `", "` between items — a classic Java-style join. The idiomatic Rust approach is `allowed.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>().join(", ")`.
- Why it matters: WARN-level idiom gap. The manual loop works but reads as TS-brain habit (accumulating into a mutable `String` with a sentinel flag).
- Suggested fix:
  ```rust
  let list = allowed.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>().join(", ");
  Some(format!("expected one of [{list}]"))
  ```
  (If this becomes a structured enum variant per the WARN above, the issue disappears entirely.)

### WARN — `ValidationReport::iter()` returns concrete `std::slice::Iter` leaking an impl detail

- Location: `validate.rs:108-129`
- What: `ValidationReport::iter()` returns `std::slice::Iter<'_, ValidationError>` as a concrete type in the public signature (line 108). Returning `impl Iterator<Item = &'_ ValidationError>` would be more idiomatic (C-ITER guideline) and future-proof — changing the backing store later would not be a breaking API change.
- Why it matters: Exposing `std::slice::Iter` in the return position is a semver commitment. Changing the internal storage from `Vec` to anything else would break callers who pattern-match on the concrete iterator type.
- Suggested fix: `pub fn iter(&self) -> impl Iterator<Item = &'_ ValidationError>`. The consuming `IntoIterator for ValidationReport` at line 122 is fine — that one is necessarily owned.

### WARN — `SchemaError::InvalidRegex::reason` is `String` but the underlying `regex::Error` is structured

- Location: `schema.rs:169-177`
- What: `SchemaError::InvalidRegex` stores `reason: String` as `e.to_string()` from the `regex::Error`. The `regex` crate's error type is public and carries structured information (kind + pattern offset). Storing the rendered string throws it away.
- Why it matters: Callers who want to programmatically act on what failed in the regex (e.g., report a column within the pattern) cannot. Minor given that regex errors are author-time mistakes, but it's a stringly-typed field where a structured one would fit.
- Suggested fix: `regex::Error` does not implement `PartialEq` or `Clone`, so storing it directly requires wrapping (or dropping `Clone`/`Eq` on `SchemaError`). Documenting this tradeoff in the variant's doc comment is the minimal fix.

### WARN — `compile_tag` calls `tag.to_string()` multiple times redundantly

- Location: `schema.rs:288-369`
- What: `compile_tag` receives `tag: &str` and allocates a new `String` via `tag.to_string()` in three separate places: the `InvalidName` error for the tag itself (line 292), the `DuplicateAttr` error (line 297), and the `InvalidRegex` error (line 320). Only one of these can trigger per call, but the allocations are separate — there is no shared clone.
- Why it matters: Minor. For a cold code path (schema build only) this is irrelevant to performance. This is a WARN-level code smell: the `.clone()` reflex scattered across error arms rather than a single `let tag = tag.to_string();` at the top if ownership is genuinely needed in multiple branches.

### INFO — `TagSchema` and `CompiledTagSchema` are `pub(crate)` and lack `PartialEq` on the compiled form

- Location: `schema.rs:58-71`, `schema.rs:139-147`
- What: `TagSchema` derives `Debug, Clone, Default`. `CompiledTagSchema` derives `Debug, Clone` but not `PartialEq`. This is `pub(crate)` so it does not affect the public API, but makes it harder to write unit tests that assert compiled schema state directly. `Regex` does not implement `PartialEq`, so deriving it on `CompiledAttrKind` / `CompiledTagSchema` would require a manual impl or wrapping.

### INFO — `SchemaBuilder::try_build` only surfaces the *first* duplicate tag, not all of them

- Location: `schema.rs:276-285`
- What: `self.duplicates.into_iter().next()` returns the first duplicate and drops the rest. If the caller registered three duplicate tags, they see one error, fix it, rebuild, and see the next one.
- Why it matters: Ergonomics only.

### INFO — `AttrKind::Regex(String)` pattern is anchored at compile time with `\A(?:...)\z` but the doc says "Match the given regular expression"

- Location: `schema.rs:83`, `schema.rs:315-324`
- What: The `AttrKind::Regex(String)` doc says "Match the given regular expression" without mentioning that the pattern is automatically anchored to match the full value. A user who supplies `"todo|done"` expecting it to match substrings will be surprised that `"undone"` is rejected.
- Suggested fix: Update the `AttrKind::Regex` doc comment to say "The pattern is implicitly anchored — it must match the entire attribute value, not a substring."

### INFO — `validate.rs` uses `std::fmt::Write as _` (line 4) — the `_` wildcard import of a trait

- Location: `validate.rs:4`
- What: `use std::fmt::Write as _` imports the `Write` trait into scope unnamed (so only the `write!` macro finds it). This is the correct idiom. However, `write!(msg, "{v:?}")` at line 273 returns a `Result` that is silently ignored with `let _ =`. For writing into a `String`, `fmt::Write` is infallible — but the explicit discard is unnecessary noise.

### Clean — Schema DSL ergonomics

Checked: `schema.rs:42-428`. The builder pattern is idiomatic: `Schema::builder()` returns `SchemaBuilder`; each method takes `self` (consuming) and returns `Self` (fluent); the terminating methods are `build` (panicking, for const/static schemas) and `try_build` (fallible, for config-loaded schemas). This mirrors `Regex::new` / `RegexBuilder::build`. `#[must_use]` is present on all builder methods and on `build`. `From<AttrKind> for AttrConstraint` (line 125-132) satisfies C-CONV. `AttrKind::one_of` is a clean ergonomic wrapper. C-CTOR, C-COMMON-TRAITS, and C-GETTER are all satisfied for public types.

### Clean — Regex compilation caching

Checked: `schema.rs:312-324`, `validate.rs:279-283`. `AttrKind::Regex(String)` is compiled exactly once in `compile_tag` (at `SchemaBuilder::build` or `try_build` time) and stored as `CompiledAttrKind::Regex(Regex)` inside `CompiledTagSchema`. `validate` only calls `re.is_match(value)` on the pre-compiled object. There is no per-call recompilation. This is correct.

### Clean — Validation engine coverage and error aggregation

Checked: `validate.rs:142-257`. The engine walks every node in the tree (`walk` is fully recursive, visiting all children regardless of whether the parent matched the schema, lines 157-162). It checks: required attributes, attribute value constraints (enum + regex), required children, exclusive-children allowlist, and text content presence. All checks accumulate into `errors: &mut Vec<ValidationError>` — validation never short-circuits on first error. The doc comment on `validate` (lines 138-140) correctly describes this behavior. No discrepancy between docs and behavior.

### Clean — Error location richness

Checked: `validate.rs:15-73`. Every `ValidationError` variant carries a 1-based `line: u32` sourced from `node.span.start.line` (a `SourcePosition`). `UnexpectedChild` uses the child's line (`child.span.start.line`, line 239), not the parent's — correct. Byte offsets are available on `SourcePosition` but not surfaced in validation errors; line numbers are the right granularity for schema errors since they're diagnostic messages, not splice targets.

### Clean — `thiserror` usage

Checked: `schema.rs:164-210`, `validate.rs:15-73`. All error enums derive `thiserror::Error`. All variants use structured fields (no `String` catch-all variant like `Other(String)`). The `#[error(...)]` format strings reference named fields. `SchemaError` and `ValidationError` are both `#[non_exhaustive]`, which is correct for a library adding variants over time.

### Clean — `unwrap` / `expect` outside tests

Checked: `schema.rs`, `validate.rs` in full. The only `unwrap_or_else` call is in `SchemaBuilder::build` (line 260), and it deliberately panics — the doc comment explains this is the intentional "I know my schema is valid" variant, with `try_build` for the fallible path. This is the correct pattern (same as `Regex::new` vs `Regex::new(...).unwrap()`).

### Clean — Public API derives (C-COMMON-TRAITS, C-DEBUG)

Checked: all public types across both files. `Schema`: `Debug, Clone, Default`. `SchemaBuilder`: `Debug, Clone, Default`. `AttrKind`: `Debug, Clone, PartialEq, Eq`. `AttrConstraint`: `Debug, Clone, PartialEq, Eq`. `SchemaError`: `Debug, Clone, Error, PartialEq, Eq`. `ValidationError`: `Debug, Clone, Error, PartialEq, Eq`. `ValidationReport`: `Debug, Clone, Default, PartialEq, Eq`. All public types implement `Debug`. Full marks on C-COMMON-TRAITS.

### Clean — Type-state opportunity assessment

The two-step `SchemaBuilder → Schema` is an adequate type-state separation: you cannot call `validate` with a `SchemaBuilder`, only with a `Schema`. A finer type-state (`UnvalidatedDoc` → `ValidatedDoc`) would require wrapping `Markdown`, which would conflict with the two-track API (Rust crate is primary, Node binding is thin). Current design is correct for the scope.

### Clean — Iterator vs loop in validate.rs

Checked: `validate.rs` throughout. Child-tag set construction (line 223) uses `.map().collect()`. Attribute iteration uses `for (attr_name, constraint) in &ts.attrs` which is idiomatic for `BTreeMap`. `TextSegments` is a custom iterator consumed via `.any(...)` (line 249). No gratuitous `for i in 0..vec.len()` loops.

## Checked

| File | Lines read |
|------|-----------|
| `crates/marxml/src/schema.rs` | 1–428 (full) |
| `crates/marxml/src/validate.rs` | 1–288 (full) |
| `crates/marxml/src/error.rs` | 1–207 (full) |
| `crates/marxml/src/document.rs` | 1–220 (full) |
| `crates/marxml/src/types.rs` | 1–271 (full, for `ElementData` / `SourcePosition` field layout) |

**Conclusion:** Schema + validation structurally sound — regex compiled exactly once, validation walks the full tree and aggregates all errors, every error variant carries a structured `line: u32`, public-type derives complete — but the primary wart is `check_kind` returning `Option<String>` and `ValidationError::InvalidAttr` storing `reason: String`, both of which discard machine-representable constraint information.
