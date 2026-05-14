<div align="center">
  <img src="https://raw.githubusercontent.com/thebytefarm/marxml/main/.github/assets/banner.png" alt="marxml" width="100%" />
  <p><strong>Fast markdown + XML query and mutation. Rust core, Node bindings. Fully typed.</strong></p>

<a href="https://github.com/thebytefarm/marxml/actions/workflows/ci.yml"><img src="https://github.com/thebytefarm/marxml/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI" /></a>
<a href="https://crates.io/crates/marxml"><img src="https://img.shields.io/crates/v/marxml" alt="crates.io" /></a>
<a href="https://www.npmjs.com/package/marxml"><img src="https://img.shields.io/npm/v/marxml" alt="npm version" /></a>
<a href="https://github.com/thebytefarm/marxml/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue" alt="License" /></a>

</div>

> [!WARNING]
> **Pre-release · under active development.** `marxml` is in the `0.0.x` placeholder phase — the names on crates.io and npm are reserved, but the working API lands in `0.1.0`. APIs, types, and CLI surface will change without notice until then. Don't depend on it in production yet.

## Features

- One pass, two trees: parse markdown once, get both the markdown AST and the embedded XML nodes addressable from the same handle.
- Query like a document: `find`, `first`, attribute filters, and CSS-ish selectors over the XML tags inside your markdown.
- Mutate without rewriting: in-place attribute and content edits, then `toString()` re-emits the original markdown around your changes.
- Rust core: zero-copy where it can, single allocation where it can't. Built on `pulldown-cmark` and a tuned XML tokenizer.
- Node-native: prebuilt `.node` binaries via [napi-rs](https://napi.rs/). No `node-gyp`, no postinstall compile.
- Dual-distributed: same engine, shipped as a [Rust crate](https://crates.io/crates/marxml) and a [Node package](https://www.npmjs.com/package/marxml).

## Install

### Node / TypeScript

```sh
pnpm add marxml
```

Ships prebuilt binaries for macOS (arm64, x64), Linux (x64-gnu, x64-musl, arm64-gnu), and Windows (x64-msvc).

### Rust

```sh
cargo add marxml
```

## Usage

> The snippets below sketch the planned `0.1.0` surface. Treat as design intent until the real release lands.

### TypeScript

```ts
import { Document } from 'marxml'

const doc = Document.parse(source)

// query
const callouts = doc.find('callout')
const frontmatter = doc.first('frontmatter')

// mutate
for (const todo of doc.find('todo')) {
  todo.setAttr('done', 'true')
}

// serialize back to markdown
const output = doc.toString()
```

### Rust

```rust
use marxml::Document;

let mut doc = Document::parse(source)?;

for node in doc.find_mut("todo") {
    node.set_attr("done", "true");
}

let output = doc.to_string();
```

## Why

Markdown is a great storage format for human-editable structured state — but parsing the embedded XML tags out reliably is fiddly, and doing it fast at scale (thousands of files, repeated parses) starts to bite. `marxml` exists to make that one job fast and ergonomic from either language.

Use cases:

- LLM tool output (XML-tagged blocks inside markdown responses)
- Static-site / docs generators with structured callouts
- Agent state machines stored as markdown
- Anywhere markdown-as-data meets a hot path

## Project layout

```
marxml/
├── crates/marxml/         # core Rust crate → crates.io
└── bindings/node/         # napi-rs wrapper  → npm
```

## License

Dual-licensed under either of [MIT](./LICENSE-MIT) or [Apache 2.0](./LICENSE-APACHE) at your option.
