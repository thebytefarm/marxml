# Schema

Declarative validation rules for parsed `marxml` documents. Authored as a per-tag map of constraints; compiled once into a `Schema` you reuse across documents.

> JSON Schema-*inspired* in spirit but **not** a JSON Schema implementation. Models XML-shaped tags (attrs + children + content), not arbitrary JSON values.

## TL;DR

```rust
// Rust
use marxml::{Schema, schema::AttrKind};

let schema = Schema::builder()
    .tag("task", |t| {
        t.attr("id",     AttrKind::String.required())
         .attr("status", AttrKind::one_of(["todo", "in_progress", "done"]).required())
         .attr("score",  AttrKind::Regex(r"\d+".into()))
         .child_required("title")
         .child_optional("note")
         .exclusive_children()
    })
    .tag("title", |t| t.content_required())
    .try_build()?;

let report = marxml::validate(&doc, &schema);
for err in report.errors() {
    eprintln!("{err}");
}
```

```ts
// Node — same shape, plain JS object
const report = doc.validate({
  task: {
    attrs: {
      id:     { kind: 'string', required: true },
      status: { kind: 'enum', values: ['todo', 'in_progress', 'done'], required: true },
      score:  { kind: 'regex', pattern: '\\d+' },
    },
    childrenRequired:  ['title'],
    childrenOptional:  ['note'],
    childrenExclusive: true,
  },
  title: { contentRequired: true },
})
```

## Concept

- **Per-tag rules.** A schema is a map `tag-name -> rules`. Tags not in the map are not validated — `validate()` walks the tree and only applies rules to elements whose tag appears in the schema.
- **Compile once, reuse many.** `Schema::builder()…try_build()` compiles regex patterns, deduplicates child lists, and converts enums to sets. On Node, `doc.validate(obj)` compiles per call (microseconds for typical schemas).
- **Errors accumulate.** Validation runs to completion and reports every problem; it does not short-circuit on the first failure.
- **Constraints are five-dimensional.** A tag carries (1) attribute constraints, (2) required-child tags, (3) optional-child tags, (4) exclusive-children flag, (5) content-required flag. Nothing else.

## Reference

### Per-tag fields

| Rust builder method        | Node field           | Meaning                                                                          |
| -------------------------- | -------------------- | -------------------------------------------------------------------------------- |
| `.attr(name, constraint)`  | `attrs: {name: …}`   | Attribute constraint (see below).                                                |
| `.child_required(name)`    | `childrenRequired`   | Child tag that must appear at least once.                                        |
| `.child_optional(name)`    | `childrenOptional`   | Child tag allowed alongside required ones (without being required itself).       |
| `.exclusive_children()`    | `childrenExclusive`  | When true, any child tag not in required ∪ optional triggers `UnexpectedChild`. |
| `.content_required()`      | `contentRequired`    | Element must contain at least one non-whitespace text character (text only — nested element markup does not count). |

### Attribute constraints

Every attribute constraint is `(kind, required)`. `required` defaults to `false`.

| Kind             | Rust                                          | Node                                            | Validates                                     |
| ---------------- | --------------------------------------------- | ----------------------------------------------- | --------------------------------------------- |
| String (any)     | `AttrKind::String`                            | `{ kind: 'string' }`                            | Attribute is present (no value constraint).   |
| Enum             | `AttrKind::Enum(vec![...])` or `::one_of([...])` | `{ kind: 'enum', values: [...] }`            | Value is one of the listed strings.           |
| Regex            | `AttrKind::Regex("…".into())`                 | `{ kind: 'regex', pattern: '…' }`               | Value matches the (whole) pattern.            |
| Required wrapper | `.required()` / `.optional()`                 | `required: true / false`                        | Whether absence of the attribute is an error. |

**Regex notes:**
- Patterns are auto-anchored to `\A(?:pattern)\z` — the regex must match the *whole* attribute value, not just a substring. `Regex("todo|done")` rejects `"undone"`.
- Patterns are compiled when the schema is built (`try_build`), not on first validation — bad patterns fail fast.
- Engine: Rust's [`regex`](https://docs.rs/regex) crate. Standard PCRE-ish syntax, no lookaround / backreferences.

### Child rules

- **`children_required`** — every name in the list must appear at least once as a direct child of the element. Duplicates in the list are deduplicated; the same required child being absent only produces one `MissingChild` error.
- **`children_optional`** — names allowed alongside required ones. Only meaningful when `children_exclusive` is set.
- **`children_exclusive`** — when `false` (default), unknown child tags are silently allowed. When `true`, the union of required + optional is the **allowlist**; anything else produces `UnexpectedChild`.

### Content rule

- **`content_required`** — element must contain at least one non-whitespace text character *directly* inside it. Child elements, comments, and CDATA do not satisfy this. `<task><title/></task>` does NOT satisfy `content_required` on `<task>` even though there's a child inside.

## Examples

### Required attribute

```ts
{ task: { attrs: { id: { kind: 'string', required: true } } } }
```
Every `<task>` must have an `id` attribute (any value).

### Enum status

```ts
{ task: { attrs: { status: { kind: 'enum', values: ['todo', 'done'], required: true } } } }
```
`<task status="todo">` ✓, `<task status="bogus">` → `invalid_attr`, `<task/>` → `missing_attr`.

### Regex-validated id

```ts
{ task: { attrs: { id: { kind: 'regex', pattern: '^[0-9]+(\\.[0-9]+)*$', required: true } } } }
```
`<task id="4.1.2">` ✓, `<task id="oops">` → `invalid_attr`.

### Exclusive children

```ts
{
  phase: {
    childrenRequired:  ['task'],
    childrenOptional:  ['note'],
    childrenExclusive: true,
  },
}
```
`<phase>` must have at least one `<task>` child, may have `<note>` children, anything else → `unexpected_child`.

### Required text content

```ts
{ title: { contentRequired: true } }
```
`<title>hello</title>` ✓, `<title/>` → `empty_content`, `<title><b>x</b></title>` → `empty_content` (the `<b>` doesn't count as text).

### Multi-tag schema (everything together)

```ts
{
  task: {
    attrs: {
      id:     { kind: 'string', required: true },
      status: { kind: 'enum', values: ['todo', 'in_progress', 'done'], required: true },
    },
    childrenRequired:  ['title'],
    childrenOptional:  ['note'],
    childrenExclusive: true,
  },
  title: { contentRequired: true },
  note:  { attrs: { author: { kind: 'string', required: true } } },
}
```

## Not supported

These exist in JSON Schema but are not part of the marxml DSL. Each is a deliberate omission, not a roadmap item.

| JSON Schema feature                | Why we don't have it                                                                                    |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `$ref` / shared definitions        | Schemas are small and per-tag; the indirection isn't worth the cost.                                    |
| `oneOf` / `anyOf` / `allOf`        | The five built-in dimensions cover the real cases. Composition is a future-feature, not a 0.x feature.  |
| Number / boolean / null types      | Attribute values are XML strings. Cast at the application layer if you need them typed.                 |
| `properties` (vs `attrs`)          | Element children are tag-named, not key-named — a flat `properties` map doesn't capture order or repetition. `children_*` does. |
| Nested type definitions            | Each tag is its own top-level entry. Recursive structure is implicit in `children_*` referring to other top-level tag names. |
| Conditional schemas (`if`/`then`)  | Out of scope for a structural validator.                                                                |

## Errors

### `SchemaError` — at build time

`SchemaBuilder::try_build()` returns these (the panicking `build()` wrapper turns them into a panic):

| Variant                                     | When                                                                                                |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `SchemaError::InvalidRegex { tag, attr, reason }` | A `AttrKind::Regex(...)` pattern failed to compile.                                            |
| `SchemaError::InvalidName { scope, name }`  | A tag, attribute, or child name in the schema isn't a valid XML name (`scope` is `"tag"`/`"attr"`/`"child"`). |
| `SchemaError::DuplicateTag { tag }`         | The same tag name was registered more than once on the builder.                                     |
| `SchemaError::DuplicateAttr { tag, attr }`  | The same attribute was registered more than once on a single tag.                                   |

Duplicate registrations are rejected (not silently last-wins) because a later-registered constraint silently disabling an earlier one is a hidden weakening of the schema.

### `ValidationError` — at validate time

`validate(&doc, &schema)` collects these into a `ValidationReport`. Every variant carries a 1-based source `line` for diagnostic output.

| Variant                                                          | When                                                                                                  |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `ValidationError::MissingAttr { tag, attr, line }`               | A required attribute was missing on a matched element.                                                |
| `ValidationError::InvalidAttr { tag, attr, value, reason, line }`| An attribute was present but its value failed an enum or regex constraint.                            |
| `ValidationError::MissingChild { tag, child, line }`             | A required child was absent from a matched parent.                                                    |
| `ValidationError::UnexpectedChild { tag, child, line }`          | A child appeared that wasn't in the allowlist (only when `children_exclusive` is true).               |
| `ValidationError::EmptyContent { tag, line }`                    | `content_required` was set but the element had no non-whitespace text directly inside it.             |

On the Node side, each error becomes a `{ kind, tag, line, message }` object. `kind` is the snake-case variant name (`"missing_attr"` / `"invalid_attr"` / `"missing_child"` / `"unexpected_child"` / `"empty_content"`).

## See also

- [Selectors](./selectors.md) — find elements (the other half of the DSL).
- [Cookbook](./cookbook.md) — schema recipes with side-by-side Rust + TS.
- [Grammar](./grammar.md) — schema shape as JSON Schema (for meta-validation).
