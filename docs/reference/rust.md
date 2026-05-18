# Rust reference

The `marxml` crate on [crates.io](https://crates.io/crates/marxml). Per-symbol docs on [docs.rs/marxml](https://docs.rs/marxml).

## At a glance

```rust
use marxml::{parse, Selector, SerializeOpts, Schema, AttrKind, validate};

let doc = parse(src)?;

let sel = Selector::parse(r#"task[status="todo"]"#)?;
for el in doc.select(&sel) {
    println!("{}", el.attr("id").unwrap_or(""));
}

let updated = doc.update(&sel, &[("status", "done")]);
let xml     = doc.to_xml(&SerializeOpts::pretty());
let json    = doc.to_json();

let schema = Schema::builder()
    .tag("task", |t| t.attr("id", AttrKind::String.required()))
    .try_build()?;
let report = validate(&doc, &schema);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Install

```sh
cargo add marxml
```

## API

### parse

```rust
fn parse(input: &str) -> Result<Markdown, ParseError>
fn parse_owned(input: String) -> Result<Markdown, ParseError>
fn parse_fragment(input: &str) -> Result<Markdown, ParseError>

const MAX_INPUT_BYTES: usize  // 16 MiB
const MAX_DEPTH: usize        // 1024
```

`parse_owned` accepts an owned `String` to avoid an extra allocation when you already have one. `parse_fragment` is an alias today; reserved for future fragment-specific semantics.

### Markdown

The parsed document. All methods take `&self`; mutators return a new `String` rather than rewriting the tree in place.

**Reading**

```rust
fn raw(&self) -> &str
fn root_count(&self) -> usize
fn root_elements(&self) -> impl Iterator<Item = ElementRef<'_>>
fn select(&self, sel: &Selector) -> impl Iterator<Item = ElementRef<'_>>
```

**Mutating** — return the rewritten document as a new `String`:

```rust
fn update(&self, sel: &Selector, attrs: &[(&str, &str)]) -> String
fn replace_content(&self, sel: &Selector, body: &str) -> String
fn replace_text(&self, sel: &Selector, body: &str) -> String
fn replace_in(&self, sel: &Selector, pattern: &Regex, replacement: &str) -> String
fn replace_text_in(&self, sel: &Selector, pattern: &Regex, replacement: &str) -> String
```

Fallible variants return a [`MutationReport`](#types) (with applied / skipped counts) and surface programmer-error inputs (invalid XML name, duplicate key) as a `MutateError` instead of panicking:

```rust
fn try_update(&self, sel: &Selector, attrs: &[(&str, &str)]) -> Result<MutationReport, MutateError>
fn replace_content_report(&self, sel: &Selector, body: &str) -> MutationReport
fn replace_in_report(&self, sel: &Selector, pattern: &Regex, replacement: &str) -> MutationReport
```

**Serializing**

```rust
fn to_xml(&self, opts: &SerializeOpts) -> String
fn to_json(&self) -> serde_json::Value
```

### Selectors

Compiled once, reused across calls.

```rust
fn Selector::parse(input: &str) -> Result<Selector, SelectorError>
```

See [DSL · Selectors](../dsl/selectors.md) for the grammar.

### Schema

Build a schema with the fluent builder; pass it to `validate`.

```rust
fn Schema::builder() -> SchemaBuilder

impl SchemaBuilder {
    fn tag(self, name: impl Into<String>, f: impl FnOnce(TagBuilder) -> TagBuilder) -> Self
    fn build(self) -> Schema             // panics on invalid input
    fn try_build(self) -> Result<Schema, SchemaError>
}

impl TagBuilder {
    fn attr(self, name: impl Into<String>, constraint: impl Into<AttrConstraint>) -> Self
    fn child_required(self, name: impl Into<String>) -> Self
    fn child_optional(self, name: impl Into<String>) -> Self
    fn exclusive_children(self) -> Self
    fn content_required(self) -> Self
}

enum AttrKind {
    String,
    Enum(Vec<String>),     // also: AttrKind::one_of(["a", "b"])
    Regex(String),
}

impl AttrKind {
    fn required(self) -> AttrConstraint
    fn optional(self) -> AttrConstraint
}
```

See [DSL · Schema](../dsl/schema.md) for the validation semantics.

### validate

```rust
fn validate(doc: &Markdown, schema: &Schema) -> ValidationReport
```

Free function (not a method on `Markdown`) so validation stays decoupled from the document type. Walks the tree once, accumulates every error.

### Types

```rust
struct ElementRef<'a> {
    fn tag(&self) -> &str
    fn attr(&self, name: &str) -> Option<&str>
    fn attrs(&self) -> impl Iterator<Item = (&str, &str)>
    fn content(&self) -> &str
    fn text(&self) -> impl Iterator<Item = &str>  // text segments between children
    fn children(&self) -> impl Iterator<Item = ElementRef<'_>>
    fn location(&self) -> SourceSpan
    fn is_self_closing(&self) -> bool
    fn select(&self, sel: &Selector) -> impl Iterator<Item = ElementRef<'_>>
}

struct SourceSpan { start: SourcePosition, end: SourcePosition }
struct SourcePosition { line: u32, offset: u32 }   // line 1-based, offset 0-based byte

struct SerializeOpts {
    fn pretty() -> Self
    fn default() -> Self
    // .indent(&str) / .self_close_empty(bool) builders
}

struct MutationReport {
    pub output: String,
    pub applied: usize,
    pub skipped_overlaps: usize,
    pub skipped_self_closing: usize,
}

struct ValidationReport {
    fn is_valid(&self) -> bool
    fn errors(&self) -> &[ValidationError]
    fn len(&self) -> usize
    fn iter(&self) -> impl Iterator<Item = &ValidationError>
}

// Escape helpers (rarely needed in app code; used internally by mutators)
fn escape_attr(s: &str) -> String
fn escape_text(s: &str) -> String
fn is_valid_name(s: &str) -> bool
```

## Behavior

- **`ElementRef<'a>` is borrowed.** Its lifetime is tied to the `Markdown` it came from. Hold the `Markdown` alive while you read element refs.
- **Mutators are pure.** They return a new `String`; the original `Markdown` is never modified. To chain mutations, re-`parse` the returned string.
- **Untouched bytes are preserved verbatim.** Mutations splice into the raw source; whitespace, comments, and surrounding prose round-trip exactly.
- **`try_*` for runtime-sourced input.** The plain `update` panics on invalid XML names or duplicate keys (programmer-error inputs). Use `try_update` when the attribute slice comes from runtime data (config, RPC, user input).
- **Regex patterns are not anchored** in `replace_in` / `replace_text_in` — you control anchoring with `^` / `$`. Schema regex constraints, by contrast, are auto-anchored.
- **Selectors are compile-once.** Cache `Selector` instances when you'll use the same selector many times.

## Errors

```rust
enum ParseError { /* … */ }                  // tokenizer + parser failures
enum SelectorError { Empty, UnexpectedEnd, Syntax { reason, at } }
enum MutateError { InvalidAttrName { name }, DuplicateAttrName { name } }
enum SchemaError { InvalidRegex {…}, InvalidName {…}, DuplicateTag {…}, DuplicateAttr {…} }
enum ValidationError {
    MissingAttr { tag, attr, line },
    InvalidAttr { tag, attr, value, reason, line },
    MissingChild { tag, child, line },
    UnexpectedChild { tag, child, line },
    EmptyContent { tag, line },
}
```

Every error type is `#[non_exhaustive]` — match with a wildcard arm. See [docs.rs](https://docs.rs/marxml) for per-variant detail.

## Compatibility

- **MSRV:** Rust 1.75.
- **Platforms:** any target that builds the `regex` and `serde_json` crates (everywhere, in practice).
- **Edition:** 2021.

## See also

- [DSL · Selectors](../dsl/selectors.md) · [DSL · Schema](../dsl/schema.md) · [DSL · Cookbook](../dsl/cookbook.md)
- [Node reference](./node.md)
- [Architecture](../ARCHITECTURE.md)
- [docs.rs/marxml](https://docs.rs/marxml) — per-symbol rustdoc
