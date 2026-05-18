---
default: major
---

#### Initial functional release

First publish of the working library. Prior `0.0.0` was a name reservation only.

**Rust API** (`marxml` crate):

```rust
let doc = marxml::parse(src)?;
let sel = marxml::Selector::parse("task[status=\"todo\"]")?;
let matches: Vec<_> = doc.select(&sel).collect();

let updated  = doc.update(&sel, &[("status", "done")]);
let replaced = doc.replace_text(&sel, "new body");
let rewrite  = doc.replace_in(&sel, &re, "$1");

let xml  = doc.to_xml(&marxml::SerializeOpts::pretty());
let json = doc.to_json();

let schema = marxml::Schema::builder()
    .tag("task", |t| t.attr("id", marxml::AttrKind::String.required()))
    .try_build()?;
let report = marxml::validate(&doc, &schema);
```

**Node API** (`marxml` on npm, ESM-only):

```ts
import { parse } from 'marxml';

const doc = parse(src);
doc.select('task[status="todo"]');
doc.updateAttrs('task', [{ name: 'status', value: 'done' }]);
doc.replaceText('task', 'new body');
doc.replaceInContent('task', /draft/i, 'final');
doc.toXml({ pretty: true });
doc.toJson();
doc.validate({ task: { attrs: { id: { kind: 'string', required: true } } } });
```

**What ships:**

- Parser: hand-rolled tokenizer + stack assembler, MAX_DEPTH 1024, MAX_INPUT_BYTES guards.
- Selectors: CSS subset (`*`, `tag`, `[attr]`, `[attr="v"]`, `^=` / `$=` / `*=`, descendant, `>`, `,`, `:first-child`, `:nth-child(n)`, `:not(simple)`).
- Mutation: `update` / `replace_content` / `replace_text` / `replace_in` / `replace_text_in` (+ fallible `try_*` variants returning `MutationReport`).
- Serialization: `to_xml` (tight + pretty) / `to_json`.
- Validation: declarative schema, `AttrKind::{String, Enum, Regex}`, required/optional attrs + children, exclusive-children, content-required.
- Node binding: factory API, no per-call reparse, JS RegExp flags preserved (`imsx`), errors never panic the host.
- 6 napi targets prebuilt: darwin-{arm64,x64}, linux-{x64-gnu,x64-musl,arm64-gnu}, win32-x64-msvc.
- Docs: `README.md`, `docs/ARCHITECTURE.md`, `docs/dsl/` (selectors + schema + grammar + cookbook), `docs/reference/{rust,node}.md`.
- License: MIT OR Apache-2.0.
