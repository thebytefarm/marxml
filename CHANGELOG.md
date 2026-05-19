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
- Phase 8 release pipeline: `.github/workflows/release.yml` tag-triggered cross-compile matrix for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`. Version-sync gate via `scripts/check-versions.sh`. Per-platform npm sub-packages under `bindings/node/npm/`. CI extended with a `node-binding` job that builds the napi binding and runs vitest on Ubuntu + macOS. `.github/workflows/bench.yml` runs `cargo codspeed` on every PR and `main` push (gated on `CODSPEED_ENABLED=true` repo variable).
- Phase 9 docs + fixture suite:
  - README rewritten around the actual `0.1.0` API (parse / select / mutate / serialize / validate). Selector cheat sheet, install instructions, links.
  - `docs/ARCHITECTURE.md` documenting tokenizer state machine, parser stack, mutation strategy (string-splicing), and the two-track API split between Rust and Node.
  - `docs/SELECTOR-GRAMMAR.md` with the formal grammar, examples for every form, and a list of what's deliberately not supported in 0.1.0.
  - `crates/marxml/tests/fixtures/` — 38 fixture files across six categories (parse, parse_fail, select, mutate, serialize, validate). `tests/fixture_suite.rs` walks them with `insta::glob!`, producing 46 reviewable snapshots.

## [0.0.0] — 2026-05-13

### Added

- Name reservation on [crates.io](https://crates.io/crates/marxml) and [npm](https://www.npmjs.com/package/marxml).
- Workspace skeleton: `crates/marxml` + `bindings/node`.
- MIT + Apache 2.0 dual license.

[Unreleased]: https://github.com/thebytefarm/marxml/compare/v0.0.0...HEAD
[0.0.0]: https://github.com/thebytefarm/marxml/releases/tag/v0.0.0
## 0.1.4 (2026-05-19)

### Fixes

- build napi loader as ESM + add publish-time smoke test

## 0.1.3 (2026-05-19)

### Fixes

- build napi loader as ESM + add publish-time smoke test

#### Fix: build the published `.node` loader as ESM (`--esm`) so `import { parse }` actually works

`marxml@0.1.1` and `0.1.2` shipped with `index.js` and `index.d.ts` in the tarball (the 0.1.1 fix), but the loader was emitted as CJS by `napi build`. Meanwhile `marxml.mjs` does `import { parse as nativeParse } from './index.js'` — an ESM named-import against a CJS module. Node rejects that with:

```
SyntaxError: The requested module './index.js' does not provide an export named 'parse'
```

Root cause: the local `pnpm build` script passes `--esm`, but the matrix `napi build` invocations in `release.yml` didn't. The published loader was CJS while the wrapper expected ESM.

Fix: add `--esm` to every matrix entry in `release.yml#jobs.build.strategy.matrix.include[].build`.

Also adds a smoke-test step to the publish job that does a real `import { parse } from 'marxml'` against the assembled package on the runner (with `NAPI_RS_NATIVE_LIBRARY_PATH` pointing at the linux-x64-gnu binary). If parse can't be loaded or doesn't return the expected shape, the publish job aborts before anything reaches a registry. Prevents this exact class of "tarball ships, package broken" failure from recurring.

## 0.1.2 (2026-05-19)

### Fixes

- include index.js and index.d.ts in the main npm tarball

## 0.1.1 (2026-05-19)

### Fixes

- include index.js and index.d.ts in the main npm tarball

#### Fix: include `index.js` and `index.d.ts` in the published npm tarball

`marxml@0.1.0` shipped without `index.js` and `index.d.ts` — the napi-rs-generated platform-dispatch loader. The two files were in the package's `files` array, but they're gitignored and were only generated in the per-platform build matrix jobs. The publish job checked out fresh and ran `npm publish` against a workspace that didn't have them, so npm silently tarballed only 4 files (`README.md`, `marxml.mjs`, `marxml.d.ts`, `package.json`).

Effect: any consumer running `import { parse } from 'marxml'` got `UNRESOLVED_IMPORT` because `marxml.mjs` re-exports from the missing `./index.js`. The package was effectively non-functional.

Fix: upload `index.js` + `index.d.ts` as a separate `js-bindings` artifact from one build matrix entry, download it into `bindings/node/` in the publish job before `npm publish` runs. No code changes — pure CI pipeline fix.

Also tightens the publish job's existing `Collect artifacts` step to filter `pattern: bindings-*` so the new `js-bindings` artifact doesn't get mixed into the per-platform binary dirs.

## 0.1.0 (2026-05-18)

### Breaking Changes

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
- 5 napi targets prebuilt: darwin-{arm64,x64}, linux-{x64-gnu,x64-musl,arm64-gnu}. Windows (`win32-x64-msvc`) pending an npm spam-detection unblock — see contributing/release.md.
- Docs: `README.md`, `docs/ARCHITECTURE.md`, `docs/dsl/` (selectors + schema + grammar + cookbook), `docs/reference/{rust,node}.md`.
- License: MIT OR Apache-2.0.

### Features

- tokenizer + stack-based assembler (#2)
- CSS-subset selectors + Markdown::select (#3)
- update / replace_content / replace_in (#4)
- to_xml, to_json, SerializeOpts + Display impls (#5)
- declarative schema + validate() (#6)
- napi-rs bindings (Phase 7) (#7)
- cross-compile + CodSpeed (Phase 8) (#8)

### Fixes

- address Codex review findings (parser/mutator/serializer hardening) (#10)
