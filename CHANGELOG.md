# Changelog

All notable changes to this project are documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 1 foundation: workspace dependencies, CI workflow, contributor docs, task runner.
- Phase 2 parser: `marxml::parse(&str) -> Result<Markdown, ParseError>` and `parse_fragment`. Hand-written state-machine tokenizer + stack-based tree assembler. Supports nested elements (including same-tag nesting, an improvement over the TS implementation), self-closing tags, hyphenated and underscored names, unicode in content and attribute values. Detects unclosed tags, mismatched/stray closes, malformed attributes, and duplicate sibling `id` attributes within the same tag.
- `Markdown::root_elements()` foundation for the selector API (Phase 3).
- `ElementRef` with `tag()`, `attr()`, `attrs()`, `content()`, `children()`, `location()`, `is_self_closing()`.

## [0.0.0] — 2026-05-13

### Added

- Name reservation on [crates.io](https://crates.io/crates/marxml) and [npm](https://www.npmjs.com/package/marxml).
- Workspace skeleton: `crates/marxml` + `bindings/node`.
- MIT + Apache 2.0 dual license.

[Unreleased]: https://github.com/thebytefarm/marxml/compare/v0.0.0...HEAD
[0.0.0]: https://github.com/thebytefarm/marxml/releases/tag/v0.0.0
