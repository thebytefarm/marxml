# marxml examples

Four independent, runnable examples — one per language × difficulty. Each is a self-contained package with its own fixture, README, and dependency manifest. Pick one and read it top-to-bottom; nothing is shared except the dependency on the locally-built `marxml` crate / binding.

## Layout

```
examples/
├── README.md                 # this file
├── reset.sh                  # clear every example's out/ dir
├── rust-simple/              # cargo workspace member: example-simple
│   ├── Cargo.toml
│   ├── samples/notes.md
│   └── src/main.rs
├── rust-advanced/            # cargo workspace member: example-advanced
│   ├── Cargo.toml
│   ├── samples/plan.md
│   └── src/main.rs
├── node-simple/              # standalone pnpm package: example-simple
│   ├── package.json          # marxml: link:../../bindings/node
│   ├── samples/notes.md
│   └── index.ts
└── node-advanced/            # standalone pnpm package: example-advanced
    ├── package.json
    ├── samples/plan.md
    └── index.ts
```

## Rust examples

Each lives at the repo's cargo workspace level. Run them by package name:

```sh
cargo run -p example-simple
cargo run -p example-advanced
```

- **`rust-simple/`** — parse → select → `to_xml` + `to_json`. Read-only.
- **`rust-advanced/`** — compiled `Selector` reuse, `update`, `replace_text`, `Schema` validation, pretty XML to `out/plan.xml`.

## Node examples

Each is a standalone pnpm package that links to the locally-built napi binding at `bindings/node/`. Build it once:

```sh
cd bindings/node
pnpm install && pnpm run build:debug
```

Then from either example directory:

```sh
pnpm install
pnpm start
```

- **`node-simple/`** — parse → select → `toXml` + `toJson`. Read-only.
- **`node-advanced/`** — `updateAttrs`, `replaceText`, `validate`, pretty XML to `out/plan.xml`.

## Reset

```sh
./reset.sh         # clears <example>/out/ for every example
```

Every input fixture (`<example>/samples/*.md`) is read-only by convention — examples never write back to them. The only state any example creates is its own `out/` directory (gitignored).
