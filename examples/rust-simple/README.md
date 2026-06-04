# rust-simple

The smallest end-to-end marxml example in Rust.

- Parses `samples/notes.md`.
- Compiles a `Selector` once and reuses it.
- Prints the structured payload back as compact XML and JSON.
- Writes nothing to disk.

## Run

```sh
cargo run -p example-simple
```

## API surface exercised

- `marxml::parse`
- `marxml::Selector::parse`
- `Markdown::select`
- `ElementRef::attr` / `ElementRef::text`
- `Markdown::to_xml` (compact)
- `Markdown::to_json`
