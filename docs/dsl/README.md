# DSL reference

`marxml` exposes two small DSLs:

| DSL | Used by | Answers |
|-----|---------|---------|
| **[Selectors](./selectors.md)** | `doc.select(...)`, `doc.updateAttrs(...)`, every mutator | *Which elements?* |
| **[Schema](./schema.md)**       | `doc.validate(...)` / `marxml::validate(...)` | *What's a valid element?* |

Both operate on the same underlying tree shape (see [Element model](#element-model) below). Both are designed to be parsed once and reused — selectors compile to a `Selector` you pass to every query; schemas compile to a `Schema` you pass to every validation.

## Pages

- [**selectors.md**](./selectors.md) — full selector grammar, every form with an example, supported pseudo-classes, what's deliberately not supported, error reference.
- [**schema.md**](./schema.md) — every schema dimension (attrs, children, content), constraint kinds, Rust builder API + Node object shape, error reference.
- [**grammar.md**](./grammar.md) — formal EBNF for the selector grammar. For tooling authors (highlighters, alternative parsers). The schema has no grammar — its spec is the TS interface.
- [**cookbook.md**](./cookbook.md) — cross-cutting recipes. Side-by-side Rust + TypeScript for the common tasks.

## Element model

Both DSLs target the same parsed-tree shape. An element has:

| Field          | Meaning                                                                                  |
| -------------- | ---------------------------------------------------------------------------------------- |
| **tag**        | `name-start name-char*` — letters, digits, `_`, `-`, `.`. ASCII only.                    |
| **attrs**      | Ordered list of `(name, value)` pairs. Names are XML names; values are decoded strings.  |
| **children**   | Direct child elements, in source order.                                                  |
| **content**    | The raw byte range *between* the open and close tags. Text + nested-element markup.     |
| **text**       | The text *between* child elements. Excludes nested markup, comments, CDATA.              |
| **location**   | `{ start, end }` source span — both `(line, offset)` pairs, line 1-based, offset 0-based |
| **selfClosing**| `true` for `<tag/>`, `false` for `<tag>…</tag>`. Self-closing elements have empty content. |

The same model surfaces as `marxml::ElementRef<'a>` (borrowed) on the Rust side and `Element` (POJO) on the Node side. Schema rules and selectors reference the same fields — when [schema.md](./schema.md) says "child tag", it means an entry in `children`; when [selectors.md](./selectors.md) says `[attr]`, it means a key in `attrs`.

## Where each DSL is parsed

| DSL       | Rust entry                          | Node entry                              |
| --------- | ----------------------------------- | --------------------------------------- |
| Selectors | `Selector::parse(s) -> Result<…>`   | `doc.select(s)` (compiles per call)     |
| Schema    | `Schema::builder()…try_build()`     | `doc.validate(obj)` (compiles per call) |

The Rust API exposes the compiled handles directly so you can reuse them across many calls. The Node binding compiles on every call — the cost is small (microseconds for typical selectors / schemas) and the API stays one-line.

## What's NOT in scope

These are explicitly NOT part of either DSL — don't look for them here:

- **Mutation syntax** — there's no separate DSL for "update this attr". Mutations are method calls (`updateAttrs`, `replaceContent`, etc.) that take a selector to identify targets.
- **Querying inside text content** — selectors target structure, not prose. For text matching inside an element, use `replaceInContent` with a regex.
- **JSON Schema** — the schema DSL is JSON-Schema-*inspired* in field names, but the validator only understands the marxml shape. You cannot paste a `$ref` / `oneOf` / `allOf` and have it work.
