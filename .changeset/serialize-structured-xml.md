---
default: minor
---

#### Add `SerializeOpts::strip_text`, `wrap_in`, and `structured()` for valid-XML output

`to_xml` previously preserved every inter-element byte verbatim. For pure XML or HTML-style input this is the right default (mixed-content fidelity). For markdown-host input where sibling tags often have prose, headings, or bullets between them, that text got baked into the "structured" output as character data — defeating the point of asking for a clean structured extract.

Multi-root inputs hit a second problem: the output was a valid XML *fragment* but not a valid XML *document*. Strict XML parsers (and `xmllint`) reject multi-root input.

This release adds two opt-in fields on `SerializeOpts` (`#[non_exhaustive]`, so additive) plus a convenience constructor that combines them:

- `SerializeOpts.strip_text: bool` — when `true`, drop non-whitespace text that appears *between* child elements. Text *inside* leaf elements (`<task>body</task>`) is kept either way.
- `SerializeOpts.wrap_in: Option<String>` — when set, wrap the serialized output in a single synthetic root element with this tag name. Turns a multi-root document into a single-root, well-formed XML document.
- `SerializeOpts::structured()` — convenience constructor equivalent to `pretty().strip_text(true).with_root("markdown")`. Produces a clean, indented, single-`<markdown>`-rooted XML document. Override the wrapper name with `.with_root("plan")` etc.
- Builder methods `.strip_text(bool)` and `.with_root(impl Into<String>)`.

**Node binding mirror**

`MarkdownDoc.toXml()` accepts three new options:

- `stripText?: boolean` — same semantics as the Rust field.
- `wrapIn?: string` — synthetic root wrapper.
- `structured?: boolean` — shortcut for the clean valid-XML-document shape.

**Examples**

The advanced examples (`rust-advanced`, `node-advanced`) switch to `SerializeOpts::structured()` / `toXml({ structured: true })` for their `plan.xml` output. The result passes `xmllint --noout`.

**Compatibility**

All changes are additive. Existing callers of `to_xml(&SerializeOpts::default())` or `to_xml(&SerializeOpts::pretty())` see identical output. No new dependencies.
