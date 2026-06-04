# rust-advanced

End-to-end mutation + validation against a **real markdown document** that
embeds XML for the structured bits. The fixture renders cleanly on GitHub
as a plan with headings and bulleted tasks; marxml parses the same bytes
as a tree of `<phase>` / `<task>` elements.

- Parses `samples/plan.md` — a markdown plan with `## Phase` headings and
  tasks as bullets wrapped in `<task>` tags.
- Compiles selectors once and reuses them across queries.
- `update` marks every `task[status="todo"]` as `done`. Headings, prose,
  and bullets are byte-preserved.
- `replace_text` swaps one task body (safe: caller-supplied bytes are escaped).
- `validate` checks the rewritten document against a `Schema`.
- Writes two outputs:
  - `out/plan.md` — full markdown with surgical edits (still renders on GitHub).
  - `out/plan.xml` — clean structured payload, via `to_xml(SerializeOpts::structured())`. Single `<markdown>` root, indented, markdown noise stripped between siblings. Valid XML document (passes `xmllint`).

## Run

```sh
cargo run -p example-advanced
```

## Reset

From this directory:

```sh
pnpm dlx rimraf out
```

The input fixture is never written to — only files under `out/` are produced — so deleting `out/` is the entire reset.

## API surface exercised

- `marxml::parse`, `Selector::parse`
- `Markdown::select` (compiled-selector reuse)
- `Markdown::update`, `Markdown::replace_text`
- `Schema::builder`, `AttrKind::one_of`, `marxml::validate`
- `SerializeOpts::structured` (pretty + strip_text + wrap_in("markdown"))
- `Markdown::to_xml`
