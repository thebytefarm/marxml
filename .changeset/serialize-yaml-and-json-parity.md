---
default: minor
---

#### Add YAML output and structured-shape parity across XML / JSON / YAML

Building on the structured-XML work in the previous changeset, this release brings the same shape across JSON and YAML so callers can pick the encoding without re-learning the schema.

**New methods**

- `Markdown::to_yaml() -> String` — YAML-encoded canonical tree. Same shape as `to_json` (top-level array of root elements with `tag` / `attrs` / `text` / `children` / `selfClosing` / `location`), just emitted as YAML. Backed by the maintained, pure-Rust, serde-native `serde-saphyr` crate (see notes on the dependency below).
- `Markdown::to_json_with(&SerializeOpts) -> serde_json::Value` and `Markdown::to_yaml_with(&SerializeOpts) -> String` — apply the same `strip_text` / `wrap_in` semantics from the XML side to the canonical shape:
  - `strip_text` empties the `text` field on every non-leaf element (drops the markdown noise that sits between sibling tags).
  - `wrap_in` wraps the top-level array under that key: `[{...}]` becomes `{"<name>": [{...}]}` (JSON) and `- ...` becomes `name:\n  - ...` (YAML).
- `SerializeOpts::structured()` now produces matching output across **all three** formats:
  - XML: `<markdown>...</markdown>`
  - JSON: `{ "markdown": [...] }`
  - YAML: `markdown:\n  - ...`

**Node binding mirror**

- `MarkdownDoc.toYaml(opts?: ToXmlOpts): string` — new method.
- `MarkdownDoc.toJson(opts?: ToXmlOpts)` — now accepts opts; return type widened to `MarkdownNode[] | Record<string, MarkdownNode[]>`. Arg-less call keeps the existing array shape.
- `MarkdownNode` — new TypeScript type for the canonical tree node (distinct from `Element`, which is the materialized view used by `select()` / `doc.elements`).

**New dependency: `serde-saphyr`**

Pure-Rust, MIT/Apache-2.0, `serde::Serialize`-native, no C FFI (matches the workspace's `unsafe_code = "forbid"` posture). The deprecated `serde_yaml` (archived March 2024) and its fork `serde_yml` (security advisory RUSTSEC-2025-0068, plus quality concerns) were rejected. `serde-saphyr` is the actively-maintained replacement with the smallest surface for our use case (we only need `serde_saphyr::to_string(&value)`).

**Examples**

The advanced examples (`rust-advanced`, `node-advanced`) now write four outputs each:

- `out/plan.md` — full markdown with surgical edits.
- `out/plan.xml` — structured payload (from the previous changeset).
- `out/plan.json` — structured payload, JSON-encoded.
- `out/plan.yaml` — structured payload, YAML-encoded.

All three structured outputs share the `markdown` wrapper key for shape parity.

**Compatibility**

All changes are additive. Arg-less `to_json()` and `to_yaml()` produce the bare-array/sequence shape. Existing `to_json` consumers see no change. `to_xml` behavior is unchanged from the previous release.
