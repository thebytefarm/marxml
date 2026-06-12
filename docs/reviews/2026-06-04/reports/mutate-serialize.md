# Reviewer C — mutate + serialize + escape

Model: Sonnet. Scope: `crates/marxml/src/mutate.rs`, `serialize.rs`, `escape.rs`, with `docs/ARCHITECTURE.md`, `types.rs`, `error.rs`, `lib.rs`, `document.rs` cross-checked.

## mutate.rs

### ERROR — Architecture doc claims descending sort; code applies ascending

- Location: `mutate.rs:329`, `docs/ARCHITECTURE.md:92`
- What: `apply_splices` sorts splices **ascending** by start offset (`a.0.start.cmp(&b.0.start)`) and then applies them left-to-right with a cursor. `ARCHITECTURE.md` step 3 states: "Sort splices by *descending* start offset so earlier mutations don't shift later positions." The doc and the code contradict each other on the ordering rationale. The ascending approach is actually the correct one for a cursor-based forward pass (descending is the right strategy for in-place `String::replace_range` edits, not for the copy-into-fresh-buffer approach the code uses), but the architectural doc is wrong — which is a documentation correctness violation per the rubric.
- Why it matters: Any engineer following the architecture doc to implement a new mutation variant will sort descending, break the cursor logic, and produce double-counted or dropped content.
- Suggested fix: Update `ARCHITECTURE.md` step 3 to read: "Sort splices ascending by start offset; a monotonic cursor then copies untouched bytes between splices into the output buffer."

### WARN — `rewrite_open_tag` uses `String::new()` with no capacity hint

- Location: `mutate.rs:264`
- What: `let mut out = String::new()` allocates zero bytes, then immediately pushes `<`, the tag name, each attribute, etc. The tag name length plus attribute data is entirely knowable at call time (sum of `el.tag().len()` + sum of attribute key/value lengths + per-attr overhead).
- Why it matters: Every `update` call on a matched element causes one reallocation chain inside `rewrite_open_tag`. On documents with many matches this is the hot path — `apply_splices` has `String::with_capacity(raw.len())` but the per-tag rewrite buffers do not.
- Suggested fix: Pre-size: `let mut out = String::with_capacity(el.tag().len() + 2 + el.attrs().map(|(k,v)| k.len() + v.len() + 4).sum::<usize>());`

### WARN — `splice_regex` is a one-line wrapper that adds no value

- Location: `mutate.rs:177-184`
- What: `fn splice_regex(doc, sel, pattern, replacement) -> MutationReport` simply delegates to `splice_regex_with(doc, sel, pattern, replacement)` unchanged. There is no semantic difference, no additional logic, and no documentation explaining why the indirection exists.
- Why it matters: Dead indirection confuses readers ("why does `replace_in` go through `splice_regex` rather than `splice_regex_with` directly?") and one of them is dead code from a refactor that didn't finish. Clippy pedantic may flag this as a trivial wrapper.
- Suggested fix: Remove `splice_regex`; have both `replace_in` (line 104) and `try_replace_in` (line 156) call `splice_regex_with` directly.

### WARN — `try_replace_content` / `try_replace_in` naming misleads: neither is fallible

- Location: `mutate.rs:142-157`
- What: Both `try_replace_content` and `try_replace_in` are named `try_*` (a Rust convention for fallible operations returning `Result`) but they return `MutationReport`, not `Result<MutationReport, E>`. The doc comment correctly says "Never fails", but the `try_` prefix is a lie to any reader who hasn't read the doc. The public API in `document.rs` exposes them as `replace_content_report` and `replace_in_report` (the correct names), so this is an internal naming problem that leaks through `pub(crate)` visibility.
- Why it matters: A newcomer writing a new calling site inside the crate will see `try_replace_content` and assume it returns `Result`, leading to incorrect error-handling boilerplate.
- Suggested fix: Rename `try_replace_content` → `splice_content_report` and `try_replace_in` → `splice_regex_report` (or drop the `try_` prefix entirely) to match the actual return type contract.

### WARN — `to_xml` initializes with `String::new()` regardless of expected size

- Location: `serialize.rs:96`
- What: `to_xml` allocates an empty `String` then fills it by recursing through potentially the entire element tree. Unlike `apply_splices` (which correctly uses `String::with_capacity(raw.len())`), there is no capacity hint here.
- Why it matters: `to_xml` walks all roots. `raw.len()` is a reasonable upper-bound hint since re-serialized XML can only be shorter (whitespace normalization) or marginally longer (entity escaping of stray characters).
- Suggested fix: `let mut out = String::with_capacity(doc.raw().len());`

### WARN — `element_json` uses `.clone()` on both `k` and `v` for every attribute

- Location: `serialize.rs:279`
- What: `el.attrs.iter().map(|(k, v)| (k.clone(), Value::String(v.clone())))` clones the owned `String` key and value out of `ElementData`. Since `ElementData` owns the `attrs: Vec<(String, String)>` and the function only borrows it, `k.clone()` is necessary for the `Map<String, Value>` key. The value clone is required too. This is not incorrect — it's the minimal thing — but it is worth noting that `attrs` in the JSON output forces O(attr_count) allocations even for read-only callers who only need `text` or `children`.
- Suggested fix: No action required now. If profiling surfaces this, the fix is an explicit borrow API or materializing only requested fields.

### INFO — `apply_splices` handles zero-element `splices` with an early return that copies the whole `raw`

- Location: `mutate.rs:321-328`
- What: The zero-splice fast path does `raw.to_string()` which is a full copy. This is correct and expected but worth noting: the caller (`replace_content`, `replace_in`) will perform an unconditional full `raw.to_string()` every time no elements match the selector.

## serialize.rs

### ERROR — `to_xml` is a re-serializing mutator path: attribute ordering is not byte-preserved

- Location: `serialize.rs:126-131`, `ARCHITECTURE.md:95-99`
- What: `emit_element` reconstructs the opening tag from `el.attrs` (a `Vec<(String, String)>` in `ElementData`). The architecture doc explicitly excludes `to_xml` from the byte-preserving splice contract (it is a serializer, not a mutator), and the module doc says "concatenate every root element as XML, ignoring the surrounding markdown text." So re-serialization here is intentional, not a contract violation. However: if a caller uses `to_xml` output as the input to a subsequent parse-and-mutate cycle, the byte offsets from the first parse no longer correspond. This is a round-trip fidelity gap that is **not documented** anywhere in the public API.
- Why it matters: A caller who does `parse(src).to_xml(opts)` and then passes the output back to `parse()` will get a document that works, but its byte offsets are different from the original; any stored `SourcePosition` values from the first parse are now stale. The gap is real but arguably acceptable for a serializer — the ERROR is the missing documentation.
- Suggested fix: Add a doc comment on `Markdown::to_xml` (in `document.rs`) warning that the output is a freshly re-serialized XML string (not the original raw source), and that stored `SourcePosition` values from the parsed document do not apply to the serializer's output.

### WARN — `emit_element` re-escapes attribute values from `ElementData` but the `update` mutator also re-escapes them

- Location: `serialize.rs:129-131`, `mutate.rs:278-281`
- What: Both `emit_element` (serializer) and `rewrite_open_tag` (mutator) call `push_escaped_attr` on the attribute values from the parsed `ElementData`. This is correct — the stored values are the raw decoded strings from the tokenizer, and re-escaping is needed on output. However, the two sites arrive at this re-escaping independently: `mutate.rs:278` has a comment explaining why ("the existing value already passed the tokenizer's validation, but re-escape on output"), while `serialize.rs` has no such comment.
- Suggested fix: Add a one-line comment to `emit_element` at the attribute loop.

### WARN — `SerializeOpts::self_close_empty` method shadows the field name

- Location: `serialize.rs:80-84`
- What: The builder method `pub fn self_close_empty(mut self) -> Self` has the same name as the public field `pub self_close_empty: bool`. In Rust, methods take priority over field access in method-call syntax, but this creates a confusing name collision: `opts.self_close_empty` (field access → `bool`) vs `opts.self_close_empty()` (builder → `Self`). The API guidelines (C-GETTER) recommend getter names match the field for read-only getters, but builder methods that shadow a field are a smell.
- Why it matters: Downstream code calling `opts.self_close_empty` to read the field will work, but calling `opts.self_close_empty()` returns `Self` (the builder), not `bool` — silent semantic drift if a caller expects a getter.
- Suggested fix: Rename the builder method to `with_self_close_empty(mut self) -> Self` or `collapse_empty(mut self) -> Self` to avoid the name collision with the public field.

### INFO — `emit_pretty_children` double-iterates `text_with_trivia` for `has_inline_text` check

- Location: `serialize.rs:244-246`
- What: `let has_inline_text = text_with_trivia(raw, el, trivia).any(|s| !is_xml_whitespace_only(s));` constructs and iterates the segment iterator once just to check a boolean, then (on the `true` branch) calls `emit_tight_children` which constructs a fresh cursor-based loop over the same data. For documents with mixed content this is two passes over the child list.

### INFO — `to_xml` root separator emits `\n` only when `opts.indent.is_some()`

- Location: `serialize.rs:98-100`
- What: `if i > 0 && opts.indent.is_some() { out.push('\n'); }` means that in tight (compact) mode, multiple roots are concatenated without any separator.

## escape.rs

### Clean — escape table

Checked: `escape_text` covers `&`, `<`, `>` (text context) at lines 33-35. `push_escaped_attr` covers `&`, `<`, `>`, `"` (attribute context) at lines 64-67. `&apos;` is not emitted by either escape path (single-quote inside double-quoted attributes needs no escaping — correct). All five named XML entities are handled correctly in `decode_one_entity` (lines 219-223: `amp`, `lt`, `gt`, `apos`, `quot`). Numeric character references (`&#NNN;` and `&#xHH;`) are decoded in `decode_one_entity` (lines 225-231). `&amp;` is listed first in all match arms so a literal `&` is never double-escaped.

No findings.

### Clean — `escape.rs` unescape path for numeric references

Checked `decode_entities` (lines 181-215): the `&#NNN;` and `&#xHH;` paths are both handled in `decode_one_entity` (lines 225-231) via `u32::from_str_radix` / `.parse::<u32>()`. References that decode to XML-illegal code points (`&#0;` etc.) are refused and passed through verbatim per line 237-240. Behavior is documented in the module doc comment. No findings.

### WARN — `decode_entities` uses `expect` on `chars().next()` in non-test code

- Location: `escape.rs:210`
- What: `let ch = value[i..].chars().next().expect("non-empty tail");` The comment says "non-empty tail" — the intent is that `i < bytes.len()` guarantees `value[i..]` is non-empty. The invariant is maintained by the loop condition (`while i < bytes.len()`), so the `expect` cannot fire on well-formed UTF-8. However, `value` is `&str` (guaranteed UTF-8 by the type system), so `value[i..]` is always a valid `&str` slice at a character boundary (only reached via `i += ch.len_utf8()` increments). The `expect` is a runtime assertion for an invariant that is actually a type-system guarantee — it should be an `unwrap_or_else(|| unreachable!(...))` or, better, restructured to use `value[i..].chars().next()` with explicit handling.
- Why it matters: `expect` in library code outside tests violates the "no `unwrap`/`expect` outside tests" rule from the profile and CLAUDE.md.
- Suggested fix: Replace with `value[i..].chars().next().unwrap_or_else(|| unreachable!("loop guarantees i < len"))` or restructure to iterate `value.char_indices()` and remove the manual byte-index tracking entirely.

### INFO — `is_name_start` / `is_name_char` accept `:` for namespace prefixes nowhere

- Location: `escape.rs:140-151`
- What: XML namespaced names (e.g. `xsi:type`) contain `:`. The current `is_name_char` does not include `:` as a valid character, meaning `update` would refuse namespaced attribute names. This is presumably intentional ("marxml accepts only ASCII XML names" per the module doc), but is not called out anywhere in the public-facing documentation for `is_valid_name`.
- Why it matters: A downstream caller who attempts `doc.update(&sel, &[("xsi:type", "foo")])` will get `MutateError::InvalidAttrName` with no explanation. Worth adding a note to the `is_valid_name` doc comment.

## Checked

| File | Lines read |
|------|-----------|
| `crates/marxml/src/mutate.rs` | 1–362 (full file) |
| `crates/marxml/src/serialize.rs` | 1–326 (full file) |
| `crates/marxml/src/escape.rs` | 1–308 (full file) |
| `docs/ARCHITECTURE.md` | 1–147 (full file) |
| `crates/marxml/src/types.rs` | 1–272 (full file) — for `ElementRef`/`ElementData` field layout and `content_range` semantics |
| `crates/marxml/src/error.rs` | 1–207 (full file) — to cross-check `MutateError` against the crate's central error enum |
| `crates/marxml/src/lib.rs` | 1–50 (full file) — to verify public re-exports of `MutateError`, `MutationReport`, `SerializeOpts` |
| `crates/marxml/src/document.rs` | 105–220 (mutation + serialization public methods) — for `#[must_use]` coverage and naming |

**Conclusion:** Two ERRORs — the architecture doc contradicts the implementation's splice sort order (ascending forward-pass vs documented descending), and `to_xml` lacks documentation that it re-serializes rather than byte-preserving (making stored `SourcePosition` values stale after round-tripping through it); five WARNs covering a dead wrapper function, misleading `try_*` names on infallible functions, missing `String::with_capacity` on the hot `rewrite_open_tag` and `to_xml` paths, a builder method that shadows a public field name, and an `expect` in library code; four INFOs on micro-opt opportunities and a namespace-name documentation gap.
