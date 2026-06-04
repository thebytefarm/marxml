# node-advanced

End-to-end mutation + validation against a **real markdown document** that
embeds XML for the structured bits. The fixture renders cleanly on GitHub
as a plan with headings and bulleted tasks; marxml parses the same bytes
as a tree of `<phase>` / `<task>` elements.

- Parses `samples/plan.md` — a markdown plan with `## Phase` headings and
  tasks as bullets wrapped in `<task>` tags.
- `updateAttrs` marks every `task[status="todo"]` as `done`. Headings,
  prose, and bullets are byte-preserved.
- `replaceText` swaps one task body (safe: caller-supplied bytes are escaped).
- `validate` checks the rewritten document against a schema.
- Writes two outputs:
  - `out/plan.md` — full markdown with surgical edits (still renders on GitHub).
  - `out/plan.xml` — clean structured payload, via `toXml({ structured: true })`. Single `<markdown>` root, indented, markdown noise stripped between siblings. Valid XML document (passes `xmllint`).

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

From this directory:

```sh
pnpm reset          # rimraf out
```

The input fixture is never written to — only files under `out/` are produced — so deleting `out/` is the entire reset.

## API surface exercised

- `parse` (from `marxml`)
- `MarkdownDoc.select`
- `MarkdownDoc.updateAttrs`, `MarkdownDoc.replaceText`
- `MarkdownDoc.validate` with `TagSchemaShape`
- `MarkdownDoc.toXml({ structured: true })` — pretty + stripText + wrapIn:"markdown"
