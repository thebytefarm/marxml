# marxml

> Fast markdown + XML query and mutation. Rust core, JS bindings.

`marxml` parses markdown documents that contain embedded XML tags and gives you a typed handle to query, mutate, and serialize them. Built in Rust, distributed both as a [Rust crate](https://crates.io/crates/marxml) and a [Node package](https://www.npmjs.com/package/marxml) via prebuilt native bindings.

**Status:** early. Name reserved, real implementation in progress.

## Why

Markdown is a great storage format for human-editable structured state — but parsing the embedded XML tags out reliably is fiddly, and doing it fast at scale (thousands of files, repeated parses) starts to bite. `marxml` exists to make that one job fast and ergonomic from either language.

Use cases:

- LLM tool output (XML-tagged blocks inside markdown responses)
- Static-site / docs generators that embed structured callouts
- Agent state machines stored as markdown
- Anywhere markdown-as-data meets a hot path

## Install

### Node / TypeScript

```sh
pnpm add marxml
```

Ships prebuilt `.node` binaries for macOS (arm64, x64), Linux (x64-gnu, x64-musl, arm64-gnu), and Windows (x64-msvc).

### Rust

```sh
cargo add marxml
```

## Usage

_API docs land with the first non-placeholder release._

## Project layout

```
marxml/
├── crates/marxml/         # core Rust crate → crates.io
└── bindings/node/         # napi-rs wrapper  → npm
```

## License

Dual-licensed under either of [MIT](./LICENSE-MIT) or [Apache 2.0](./LICENSE-APACHE) at your option.
