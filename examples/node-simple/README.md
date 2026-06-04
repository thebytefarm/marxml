# node-simple

The smallest end-to-end marxml example in TypeScript.

- Parses `samples/notes.md`.
- Selects with `note[tag="idea"]`.
- Prints the structured payload back as compact XML and JSON.
- Writes nothing to disk.

## Run

Build the napi binding once (from `bindings/node/`):

```sh
cd ../../bindings/node
pnpm install && pnpm run build:debug
```

Then from this directory:

```sh
pnpm install
pnpm start
```

## API surface exercised

- `parse` (from `marxml`)
- `MarkdownDoc.select`
- `Element.attrs` / `Element.content`
- `MarkdownDoc.toXml` (compact)
- `MarkdownDoc.toJson`
