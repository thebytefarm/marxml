# Changelog

All notable changes to this project are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 1 foundation: workspace dependencies, CI workflow, contributor docs, task runner.
- Phase 2 parser: `marxml::parse(&str) -> Result<Markdown, ParseError>` and `parse_fragment`. Hand-written state-machine tokenizer + stack-based tree assembler. Supports nested elements (including same-tag nesting, an improvement over the TS implementation), self-closing tags, hyphenated and underscored names, unicode in content and attribute values. Detects unclosed tags, mismatched/stray closes, malformed attributes, and duplicate sibling `id` attributes within the same tag.
- `Markdown::root_elements()` foundation for the selector API.
- `ElementRef` with `tag()`, `attr()`, `attrs()`, `content()`, `children()`, `location()`, `is_self_closing()`.
- Phase 3 selectors: `Selector::parse(s) -> Result<Selector, SelectorError>`, `Markdown::select(&Selector)` and `ElementRef::select(&Selector)`. CSS3 subset — tag, `*`, `[attr]`, `[attr="val"]`, `^=`, `$=`, `*=`, descendant (` `), child (`>`), union (`,`), `:first-child`, `:nth-child(n)`, `:not(simple)`. Tag-less selectors (`[id]`) match any element with the attribute.
- `ElementRef::text()` yields text segments between child elements; `ElementRef::select()` for sub-tree queries.
- `SelectorError` variants: `Empty`, `UnexpectedEnd`, `Syntax { reason, at }`.
- Phase 4 mutation: `Markdown::update(&Selector, &[(name, value)])`, `Markdown::replace_content(&Selector, body)`, `Markdown::replace_in(&Selector, &Regex, replacement)`. All return a new owned `String`; the original document is never modified. Untouched bytes are preserved verbatim. Mutated documents remain parseable.
- Phase 5 serialization: `Markdown::to_xml(&SerializeOpts)`, `Markdown::to_json() -> serde_json::Value`. `SerializeOpts` configures `indent` (per-level prefix) and `self_close_empty`. `SerializeOpts::pretty()` for indented multi-line output. `Display` impl on `Markdown` returns the original raw; `Display` on `ElementRef` returns the element's outer XML, byte-for-byte from the source.
- Phase 6 validation: `marxml::validate(&Markdown, &Schema) -> ValidationReport`. Declarative `Schema::builder()` API with per-tag rules: required/optional attributes, attribute value constraints (`AttrKind::String`, `Enum`, `Regex`), required children, optional children, exclusive-children allowlist, content-required. `ValidationError` variants: `MissingAttr`, `InvalidAttr`, `MissingChild`, `UnexpectedChild`, `EmptyContent`. Validation continues past the first error so all problems surface at once. Regex patterns are pre-compiled at `Schema::build` time, failing fast on bad patterns.
- Phase 7 Node bindings: napi-rs 3 bindings in `bindings/node/`. JS functions: `parse`, `select`, `updateAttrs`, `replaceContent`, `replaceInContent`, `toXml`, `toJson`, `validateSchema`. `replaceInContent` accepts either a string pattern or a `{ source, flags }` `RegExpShape`. `validateSchema` takes a JS-shaped schema object and returns a `ValidationReport` with typed errors. 23 vitest tests against the built binary.

## [0.0.0] — 2026-05-13

### Added

- Name reservation on [crates.io](https://crates.io/crates/marxml) and [npm](https://www.npmjs.com/package/marxml).
- Workspace skeleton: `crates/marxml` + `bindings/node`.
- MIT + Apache 2.0 dual license.

[Unreleased]: https://github.com/thebytefarm/marxml/compare/v0.0.0...HEAD
[0.0.0]: https://github.com/thebytefarm/marxml/releases/tag/v0.0.0
