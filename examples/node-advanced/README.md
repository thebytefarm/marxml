# node-advanced

End-to-end mutation + validation in TypeScript.

- Parses `samples/plan.md`.
- `updateAttrs` marks every `task[status="todo"]` as `done`.
- `replaceText` swaps one task body (safe: caller-supplied bytes are escaped).
- `validate` checks the rewritten document against a schema.
- Pretty-prints the result to `out/plan.xml`.

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

## Reset

```sh
../reset.sh         # clears out/ in every example
```

The input fixture is never written to — only `out/plan.xml` is produced.

## API surface exercised

- `parse` (from `marxml`)
- `MarkdownDoc.select`
- `MarkdownDoc.updateAttrs`, `MarkdownDoc.replaceText`
- `MarkdownDoc.validate` with `TagSchemaShape`
- `MarkdownDoc.toXml({ pretty: true })`
