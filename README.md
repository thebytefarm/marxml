<div align="center">
  <img src="https://raw.githubusercontent.com/thebytefarm/marxml/main/.github/assets/banner.png" alt="marxml" width="100%" />
  <p><strong>Fast markdown + XML query and mutation. Rust core, Node bindings. Workers of the markup, unite.</strong></p>

<a href="https://github.com/thebytefarm/marxml/actions/workflows/ci.yml"><img src="https://github.com/thebytefarm/marxml/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
<a href="https://crates.io/crates/marxml"><img src="https://img.shields.io/crates/v/marxml" alt="crates.io" /></a>
<a href="https://www.npmjs.com/package/marxml"><img src="https://img.shields.io/npm/v/marxml" alt="npm version" /></a>
<a href="https://github.com/thebytefarm/marxml/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue" alt="License" /></a>

</div>

> [!WARNING]
> **Pre-1.0 · under active development.** `marxml` is on `0.X.X` and APIs, types, and selector grammar may still shift between minor versions. Not recommended for production yet — pin a specific version and read the changelog before bumping. Licensed MIT/Apache-2.0 (so it's always at your own risk anyway).

`marxml` lets you read and write XML-shaped tags embedded in markdown documents. Find them with CSS-style selectors, change them surgically, validate they're well-formed — without rewriting the prose around them. Same API in Rust and Node.

## Features

- **Find tags with selectors.** `task[id^="4."]`, `phase > task`, `note:not([archived])` — the CSS subset you already know.
- **Edit surgically.** Change an attribute or replace inner content. Every byte you didn't touch comes back identical: prose, whitespace, comments, ordering.
- **Validate the shape.** Required attributes, enum/regex constraints, child rules — declarative schema, structured errors with line numbers.
- **Fast on both sides.** Native speed in Node via prebuilt binaries for macOS and Linux. Windows support is pending — see [release notes](./contributing/release.md#platform-coverage).

## Why?

`marxml` started as plumbing for a workflow agent — plan and phase-planning documents (think GSD-style trackers) stored as markdown with task state inside XML tags. Agents needed to update those tags reliably: flip a status, append a note, mark a child done — without rewriting the surrounding prose or hallucinating new structure.

The general lesson: LLMs drift at the prose level but stay disciplined inside known XML tags. Scope the model's output to a tag, and the read/write boundary becomes deterministic again. `marxml` is the read/write layer for that boundary — selectors to find tags, byte-preserving mutation to change them, schema to verify what came back. Same shape in Rust and Node.

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
  <task id="1.1" status="todo">do this</task>
  <task id="1.2" status="done">finished</task>
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
import { parse } from 'marxml'

const src = `
<phase id="1" status="todo">
  <task id="1.1" status="todo">do this</task>
  <task id="1.2" status="done">finished</task>
</phase>
`

const doc = parse(src)

for (const task of doc.select('task[status="todo"]')) {
  console.log(task.attrs.id) // "1.1"
}

const updated = doc.updateAttrs('task[status="todo"]', [
  { name: 'status', value: 'done' },
])
```

## Docs

- [DSL reference](docs/dsl/) — selectors, validation schema, cookbook recipes, formal grammar.
- [Rust reference](docs/reference/rust.md) — API surface, design notes, lints, MSRV.
- [Node reference](docs/reference/node.md) — `MarkdownDoc` shape, distribution, regex behavior.
- [Architecture](docs/ARCHITECTURE.md) — tokenizer, mutation strategy, two-track API.

## License

Dual-licensed under either [MIT](./LICENSE-MIT) or [Apache 2.0](./LICENSE-APACHE) at your option.
