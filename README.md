<div align="center">
  <img src="https://raw.githubusercontent.com/thebytefarm/marxml/main/.github/assets/banner.png" alt="marxml" width="100%" />
  <p><strong>Fast markdown + XML query and mutation. Rust core, Node bindings. Workers of the markup, unite.</strong></p>

<a href="https://github.com/thebytefarm/marxml/actions/workflows/ci.yml"><img src="https://github.com/thebytefarm/marxml/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
<a href="https://crates.io/crates/marxml"><img src="https://img.shields.io/crates/v/marxml" alt="crates.io" /></a>
<a href="https://www.npmjs.com/package/marxml"><img src="https://img.shields.io/npm/v/marxml" alt="npm version" /></a>
<a href="https://github.com/thebytefarm/marxml/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue" alt="License" /></a>

</div>

> [!WARNING]
> **Pre-release · under active development.** `marxml` is in the `0.0.x` placeholder phase — the names on crates.io and npm are reserved, but the working API lands in `0.1.0`. APIs, types, and selector grammar will change without notice until then. Don't depend on it in production yet.

`marxml` parses markdown documents that embed XML-shaped tags and gives you a typed handle for querying, mutating, and serializing them. Built in Rust, distributed as both a [crates.io](https://crates.io/crates/marxml) crate and an [npm package](https://www.npmjs.com/package/marxml) with prebuilt native bindings via [napi-rs](https://napi.rs/).

The Rust API mirrors [`scraper`](https://github.com/rust-scraper/scraper) — the de facto CSS-selector HTML library — adapted for markdown+XML. The Node API mirrors the same shape with JS-idiomatic ergonomics.

## Features

- **One pass, structured tree.** Hand-rolled state-machine tokenizer + stack-based assembler. No regex parsing — same-tag nesting works out of the box.
- **CSS-subset selectors.** `task[id^="4."]`, `phase > task`, `*:nth-child(2)`, `:not(simple)`. Compiled once, reused across documents.
- **Surgical mutation.** Three string-returning helpers — `update`, `replace_content`, `replace_in`. Untouched bytes are preserved verbatim.
- **Validation.** Declarative schema with required/optional attributes, enum/regex constraints, required children, exclusive allowlists.
- **Native everywhere.** Prebuilt `.node` binaries for macOS (arm64, x64), Linux (x64-gnu, x64-musl, arm64-gnu), and Windows (x64-msvc).

## Install

### Rust

```sh
cargo add marxml
```

### Node / TypeScript

```sh
pnpm add marxml
```

## Rust quickstart

```rust
use marxml::{parse, Selector};

let src = r#"
<phase id="1" status="todo">
  <task id="1.1"><status>todo</status></task>
  <task id="1.2"><status>done</status></task>
</phase>
"#;

let doc = parse(src)?;
let sel = Selector::parse(r#"task[status="todo"]"#)?;

for task in doc.select(&sel) {
    println!("{}", task.attr("id").unwrap_or(""));
}

let updated = doc.update(&sel, &[("status", "done")]);
println!("{updated}");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Node quickstart

```ts
import {
  parse,
  select,
  updateAttrs,
  replaceContent,
  replaceInContent,
  toXml,
  toJson,
  validateSchema,
} from 'marxml'

const src = `<task id="1" status="todo">do thing</task>`

const doc = parse(src)
// { raw: '<task id="1" status="todo">do thing</task>',
//   elements: [{ tag: 'task', attrs: { id: '1', status: 'todo' }, ... }] }

const tasks = select(src, 'task[status="todo"]')
const updated = updateAttrs(src, 'task', [{ name: 'status', value: 'done' }])
```

## Selector cheat sheet

| Pattern              | Meaning                              | Example                |
| -------------------- | ------------------------------------ | ---------------------- |
| `tag`                | tag name                             | `task`                 |
| `*`                  | any element                          | `*`                    |
| `[attr]`             | has attribute                        | `task[id]`             |
| `[attr="val"]`       | attribute equals                     | `task[id="4.1"]`       |
| `[attr^="x"]`        | attribute starts-with                | `task[id^="4."]`       |
| `[attr$="x"]`        | attribute ends-with                  | `task[id$=".final"]`   |
| `[attr*="x"]`        | attribute contains                   | `task[id*="."]`        |
| `a, b`               | union                                | `task, phase`          |
| `a b`                | descendant                           | `phase task`           |
| `a > b`              | direct child                         | `phase > task`         |
| `:first-child`       | first child of its parent            | `task:first-child`     |
| `:nth-child(n)`      | nth child, 1-indexed                 | `task:nth-child(2)`    |
| `:not(simple)`       | negation of a simple selector        | `task:not([status])`   |

Sibling combinators (`~`, `+`), `:has()`, `:contains()`, and case-insensitive flags are not yet supported. Full grammar in [`docs/SELECTOR-GRAMMAR.md`](docs/SELECTOR-GRAMMAR.md).

## Use cases

- LLM tool output: structured XML-tagged blocks embedded in markdown responses.
- Agent state machines stored as markdown (the use case marxml was built for).
- Static-site / docs pipelines with structured callouts.
- Anywhere markdown-as-data meets a hot loop.

## Why?

Markdown is the lingua franca of LLM output. XML is what models reach for when they need structure inside it. The result is a hybrid the existing tooling fits poorly: markdown parsers flatten the tags into HTML, XML parsers choke on the surrounding prose, and ad-hoc regex collapses the moment tags nest.

`marxml` treats the hybrid as the primary format. One tokenizer pass produces a typed tree you can query with CSS-subset selectors and mutate with byte-preserving string splices. Same shape in Rust and Node, same selectors, same semantics.

## How it works

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the tokenizer state machine, the two-track API split, and the mutation strategy (string-splicing rather than AST rewrite).

## Layout

```
marxml/
├── crates/marxml/        # Rust crate → crates.io
│   └── benches/          # criterion benches (also wired to CodSpeed)
└── bindings/node/        # napi-rs wrapper → npm
    └── npm/<target>/     # per-platform binary sub-packages
```

## License

Dual-licensed under either [MIT](./LICENSE-MIT) or [Apache 2.0](./LICENSE-APACHE) at your option.
