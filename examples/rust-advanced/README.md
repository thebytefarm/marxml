# rust-advanced

End-to-end mutation + validation in Rust.

- Parses `samples/plan.md`.
- Compiles selectors once and reuses them across queries.
- `update` marks every `task[status="todo"]` as `done`.
- `replace_text` swaps one task body (safe: caller-supplied bytes are escaped).
- `validate` checks the rewritten document against a `Schema`.
- Pretty-prints the result to `out/plan.xml`.

## Run

```sh
cargo run -p example-advanced
```

## Reset

```sh
../reset.sh         # clears out/ in every example
```

The input fixture is never written to — only `out/plan.xml` is produced.

## API surface exercised

- `marxml::parse`, `Selector::parse`
- `Markdown::select` (compiled-selector reuse)
- `Markdown::update`, `Markdown::replace_text`
- `Schema::builder`, `AttrKind::one_of`, `marxml::validate`
- `SerializeOpts::pretty`, `Markdown::to_xml`
