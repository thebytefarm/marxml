# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, Cursor, opencode, etc.) working in this repo. Edit this file — `CLAUDE.md` is a symlink to it.

## What this repo is

This is [`thebytefarm/marxml`](https://github.com/thebytefarm/marxml) — a Rust workspace that publishes:

- **`marxml`** on [crates.io](https://crates.io/crates/marxml) — the parser/selector/mutator core.
- **`marxml`** on [npm](https://www.npmjs.com/package/marxml) — the same core compiled to native bindings via [napi-rs](https://napi.rs/), with prebuilt `.node` artifacts per platform.

Both ship as **one product**: the crate and the npm package always share the same version (enforced by `scripts/check-versions.sh` in CI). The library parses markdown documents that embed XML-shaped tags, lets you query them with CSS-subset selectors, and lets you mutate them with byte-preserving string splices. Background: see [`README.md`](./README.md), [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md), and [`docs/SELECTOR-GRAMMAR.md`](./docs/SELECTOR-GRAMMAR.md).

## Persona

You are an idiomatic Rust author writing a small, performance-sensitive parser. You prefer:

- **Pure functions** over methods that mutate `self` — the parser is one pass over `&str`, the mutators return owned `String`s.
- **`thiserror`-derived enums** over `Box<dyn Error>` or string errors.
- **State machines and stack-based assembly** over regex for tokenization — same-tag nesting and byte-offset bookkeeping depend on this.
- **Byte-preserving splices** over AST rebuilding for mutation — round-trip fidelity is a hard requirement.
- **Compiled-once, reused-many selectors** — `Selector::parse` is the hot-path entry, not a per-call helper.

The Rust API mirrors [`scraper`](https://github.com/rust-scraper/scraper) intentionally. The Node API mirrors the Rust API intentionally. Divergence between the two surfaces is a smell.

Enforced by:

- Workspace lints (`Cargo.toml`): `unsafe_code = "forbid"`, `missing_docs = "warn"`, `clippy::all` + `clippy::pedantic` at warn (with a few carve-outs).
- CI (`.github/workflows/ci.yml`): `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --workspace` on Linux + macOS + Windows.

Don't fight the lints — fix the design.

## Boundaries

### Always

- **Read files before modifying them.**
- **Run commands from repo root.** `cargo` operates on the workspace. For the Node side, use `pnpm` inside `bindings/node/` (the npm workspace is local to that dir).
- **Use `just`** for the common task gates — `just check`, `just test`, `just bench`, `just cov`, `just versions`. See [Commands](#commands).
- **Use `thiserror`** for new error variants. Keep public errors in `crates/marxml/src/error.rs`.
- **Preserve untouched bytes verbatim** in any mutation path. Mutators take `&str` and return `String` by splicing, never by re-serializing the tree.
- **Mirror the API** when you change one side. New Rust mutator → corresponding Node export. New Node helper → corresponding crate fn.
- **Add a changeset** (`.changeset/<slug>.md`) for any change to the public Rust API or the Node binding's `marxml.d.ts`. Knope reads these to drive the release PR.
- **Conventional Commits** — `type(scope): subject`. Knope reads these too. See [Git](#git).
- **Run `just check` before pushing.** That's the CI gate locally.

### Never

- **`unsafe` blocks** in crate code. Workspace forbids it (`unsafe_code = "forbid"`).
- **Hand-bump versions.** `Cargo.toml`, `bindings/node/package.json`, and the six `bindings/node/npm/*/package.json` files all change together — via knope's release flow, never by editing them directly. Add a changeset instead.
- **New regex for tokenization.** The tokenizer is a hand-rolled state machine on purpose — regex breaks on same-tag nesting and on the byte-offset bookkeeping the mutators rely on. Regex is fine for selector attr-value matching and schema validation (where it's already used).
- **AST rebuild for mutation.** Mutators splice into the source string. Reconstructing from the typed tree loses whitespace, comments, and markdown fidelity.
- **Mutate function parameters or shared state.** Pass-through, return owned.
- **Clippy warnings.** CI runs `-D warnings`.
- **`--no-verify`, `--no-gpg-sign`,** or any flag that bypasses a hook or check. Fix the underlying failure.
- **Commit directly to `main`** for non-WIP work. PRs target `main`.
- **Commit secrets, `.env`, API keys, or tokens.**
- **Emojis** in code, commits, PRs, or docs unless explicitly asked.
- **Comments that restate the code.** Only write a comment when the *why* is non-obvious.

### Ask First

- **Adding a crate dependency** — workspace `Cargo.toml` or member.
- **Adding an npm dependency** in `bindings/node/package.json`.
- **Changing the public crate API** — `parse`, `Selector`, `Document`, mutator signatures, error variants, anything re-exported from `crates/marxml/src/lib.rs`.
- **Changing the Node binding surface** — anything visible from `marxml.mjs` / `marxml.d.ts`, or any new method on the underlying napi class in `bindings/node/src/lib.rs`.
- **Selector grammar changes** — keep [`docs/SELECTOR-GRAMMAR.md`](docs/SELECTOR-GRAMMAR.md) as the source of truth.
- **Schema DSL changes** — `crates/marxml/src/schema.rs` shapes the public validation API.
- **Tokenizer state-machine changes.** It's load-bearing; propose the change before writing it.
- **Bumping MSRV** (currently `1.75`) or the napi-rs target list.
- **Touching `knope.toml`, `release.yml`, or `scripts/check-versions.sh`.** These run the publish pipeline.
- **Force pushes, branch deletes, `reset --hard`,** or anything rewriting shared history.

## Structure

```
marxml/
├── Cargo.toml                            # Workspace root: members, shared deps, lints
├── justfile                              # Local task runner (mirrors CI gates)
├── knope.toml                            # Release config — DO NOT bump versions manually
├── crates/
│   └── marxml/                           # The published crate → crates.io
│       ├── src/
│       │   ├── lib.rs                    # Public surface
│       │   ├── tokenizer.rs              # State-machine tokenizer (no regex)
│       │   ├── parse.rs                  # Stack-based tree assembler
│       │   ├── document.rs               # Typed tree + select()
│       │   ├── selector/                 # CSS-subset selector parser + matcher
│       │   ├── mutate.rs                 # update / replace_content / replace_in
│       │   ├── serialize.rs              # to_xml / to_json + SerializeOpts
│       │   ├── schema.rs                 # Declarative validation schema
│       │   ├── validate.rs               # Validation engine
│       │   ├── escape.rs                 # XML escaping helpers
│       │   ├── types.rs                  # Shared types
│       │   └── error.rs                  # thiserror error enum
│       ├── tests/                        # Integration + property + snapshot (insta) + fixture_suite
│       └── benches/                      # criterion benches → CodSpeed in CI
├── bindings/
│   └── node/                             # napi-rs wrapper → npm
│       ├── src/lib.rs                    # #[napi] class wrapping marxml::Markdown
│       ├── marxml.mjs                    # ESM factory wrapper (hides the napi class) — public
│       ├── marxml.d.ts                   # Public TS API (MarkdownDoc interface + JSDoc)
│       ├── index.js / index.d.ts         # napi-rs raw output — NOT the public API
│       ├── __test__/                     # vitest binding tests
│       ├── npm/<target>/                 # per-platform binary sub-packages (6 platforms)
│       └── package.json                  # npm metadata + napi target list
├── docs/
│   ├── ARCHITECTURE.md                   # Tokenizer state machine, two-track API, mutation strategy
│   └── SELECTOR-GRAMMAR.md               # Full selector grammar
├── .changeset/                           # Pending release notes (one .md per change, knope-managed)
├── scripts/check-versions.sh             # CI guard — asserts crate vs npm version parity
├── .github/workflows/                    # ci.yml, bench.yml, release-pr.yml, release-tag.yml, release.yml
└── AGENTS.md                             # ← you are here (CLAUDE.md is a symlink)
```

**Two-track API.** The Rust crate is the source of truth. The Node binding is a thin `#[napi]` wrapper. New surface lands in Rust first.

**Mutation contract.** Every mutator takes `&str` and returns `String`. Untouched byte ranges are copied verbatim. If you can't preserve fidelity through a change, raise it before implementing.

## Tech Stack

| Tool                                                                                          | Purpose                                  |
| --------------------------------------------------------------------------------------------- | ---------------------------------------- |
| [Rust](https://www.rust-lang.org/) 2021, MSRV 1.75                                            | Core language                            |
| [thiserror](https://docs.rs/thiserror)                                                        | Error enums                              |
| [regex](https://docs.rs/regex)                                                                | Selector attr matching + schema validation — **not** the tokenizer |
| [serde](https://serde.rs) + [serde_json](https://docs.rs/serde_json)                          | `to_json` serialization                  |
| [napi-rs](https://napi.rs/) (`@napi-rs/cli`)                                                  | Rust → Node bindings + prebuilt binaries |
| [vitest](https://vitest.dev)                                                                  | Node-side binding tests                  |
| [insta](https://insta.rs)                                                                     | Snapshot tests (fixtures)                |
| [proptest](https://proptest-rs.github.io/proptest/)                                           | Property-based parser/mutator tests      |
| [rstest](https://docs.rs/rstest)                                                              | Parameterized tests                      |
| [criterion](https://bheisler.github.io/criterion.rs/book/) + [CodSpeed](https://codspeed.io/) | Benchmarks                               |
| [knope](https://knope.tech/)                                                                  | Release automation (changesets → tag → publish) |
| [just](https://github.com/casey/just)                                                         | Local task runner                        |

Node `>= 18`. pnpm for the Node workspace inside `bindings/node/`.

## Commands

Run from repo root. `just` mirrors the CI gates.

| Command                                            | What it does                                                  |
| -------------------------------------------------- | ------------------------------------------------------------- |
| `just check`                                       | Full pre-push gate: `fmt-check + clippy + test`               |
| `just fmt`                                         | `cargo fmt --all`                                             |
| `just fmt-check`                                   | `cargo fmt --all -- --check`                                  |
| `just clippy`                                      | `cargo clippy --all-targets --all-features --workspace -- -D warnings` |
| `just test`                                        | `cargo test --all-features --workspace`                       |
| `just test-thorough`                               | Same with `PROPTEST_CASES=10000` (slower; run before pushing breaking changes) |
| `just bench`                                       | `cargo bench --workspace` (CodSpeed wraps this in CI)         |
| `just cov` / `just cov-summary`                    | `cargo llvm-cov` (HTML report or summary)                     |
| `just versions`                                    | Verify crate vs npm version parity                            |
| `just clean`                                       | `cargo clean` + remove coverage artifacts                     |

Node binding (run from `bindings/node/`):

```bash
pnpm install
pnpm run build              # release build of the .node + JS/dts
pnpm run build:debug        # debug build (faster; needed before `pnpm test`)
pnpm test                   # vitest against the built binding
```

Knope (verify before pushing changes to `knope.toml`):

```bash
knope --validate                                       # config schema check
knope prepare-release --dry-run                        # preview the release PR contents
knope prepare-release --prerelease-label rc --dry-run  # preview RC bump
```

## Verification

Before marking a task complete:

1. **`just check` passes** locally — `fmt-check + clippy + test`.
2. **For Node-touching changes:** `cd bindings/node && pnpm run build:debug && pnpm test` succeeds.
3. **No boundary violations.** No new `unsafe`, no new regex in the tokenizer, no AST-rebuild mutators, no clippy warnings papered over with `#[allow(...)]` without an explanatory comment.
4. **Public-API changes have a changeset.** `.changeset/<slug>.md` with the right bump (`patch`/`minor`/`major`).
5. **Docs in sync.** Public API changes update doc comments. Selector changes update [`docs/SELECTOR-GRAMMAR.md`](docs/SELECTOR-GRAMMAR.md). Architecture changes update [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
6. **Both sides updated** when the change spans Rust + Node.
7. **Conventional Commits** for every commit.

CI (`.github/workflows/ci.yml`) runs `fmt → clippy → test (Linux/macOS/Windows) → coverage` on every push and PR to `main`. CodSpeed runs the bench suite on PRs via `.github/workflows/bench.yml`.

## Git

**Branches.** Feature branches for non-WIP work; PRs target `main`. Pre-1.0 WIP can land directly on `main` if explicitly authorized.

**Conventional Commits.** Format: `type(scope): subject`. Knope reads these to drive releases.

- **Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `ci`, `deps`, `release`
- **Scopes** (flexible — no commitlint pin): a module name (`parse`, `mutate`, `serialize`, `selector`, `schema`, `validate`), a surface (`node`, `crate`), or a workflow (`release`, `ci`, `docs`)

```bash
git commit -m "feat(mutate): add replace_in for inline content edits"
git commit -m "fix(selector): handle escaped quotes in attr-value matching"
git commit -m "refactor: tighten Rust idioms across the crate"
```

**Changesets.** One `.changeset/<slug>.md` per user-visible change. Knope consumes them during `release-pr.yml`.

**GitHub work** uses `gh` directly — never synthesize URLs.

## Release flow

Three workflows, chained — **read this before touching anything version-related**.

1. **`release-pr.yml`** — runs on push to `main` (or manual dispatch with `prerelease=rc`). Knope aggregates `.changeset/*.md`, bumps `Cargo.toml` + `bindings/node/package.json` + the six `npm/*/package.json` files, prepends a new `CHANGELOG.md` section, deletes the consumed changesets, and opens a `chore: release X.Y.Z` PR.
2. **`release-tag.yml`** — runs when the release PR merges. Knope creates the `vX.Y.Z` git tag.
3. **`release.yml`** — runs on `v*` tag push. Cross-compiles napi bindings for 6 platforms, publishes per-platform npm sub-packages, the main npm package (with provenance), and the crate to crates.io.

**Two secrets needed**: `CARGO_REGISTRY_TOKEN`, `NPM_TOKEN`. Both set at the repo level.

## Adding things

**A new selector pseudo-class or combinator:** Update `crates/marxml/src/selector/` (parser + matcher), add insta/proptest cases under `crates/marxml/tests/`, then update [`docs/SELECTOR-GRAMMAR.md`](docs/SELECTOR-GRAMMAR.md). The Node side passes selectors through as strings — no mirror needed.

**A new mutator:** Land it in `crates/marxml/src/mutate.rs`. Cover with snapshot + property tests (round-trip fidelity is the invariant). Then add a method to the napi class in `bindings/node/src/lib.rs`, expose it through `marxml.mjs`, type it in `marxml.d.ts`, regenerate types (`pnpm run build:debug`), and add a vitest case.

**A new schema constraint:** Extend `crates/marxml/src/schema.rs` + `validate.rs`. Update the Node `validateSchema` surface if the input shape changes.

**A new crate dependency:** **Ask first.** When approved, prefer adding it to `[workspace.dependencies]` and referencing from the member with `{ workspace = true }`.

**A new napi target:** **Ask first.** Targets live in `bindings/node/package.json#napi.targets` and require a matching `bindings/node/npm/<target>/` sub-package.

## Pitfalls specific to this repo

- **The tokenizer is hand-rolled on purpose.** Reaching for `regex` to "simplify" it will break same-tag nesting and the byte-offset bookkeeping the mutators depend on. If a tokenizer change feels regex-shaped, the abstraction is wrong, not the tool.
- **Mutators return `String`, not `Document`.** Chained mutation re-parses. This is intentional — it keeps every public mutator round-trip-safe in isolation. Don't add a "fast path" that operates on a held tree without reading [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) first.
- **`Selector::parse` is the hot-path API.** Compiled once, reused many. The benches assume this — don't introduce a `select_str(src, "css")` convenience that re-parses on every call as the primary API.
- **Selectors require quoted attribute values.** `task[id="t1"]` works; `task[id=t1]` errors. Grammar is in [`docs/SELECTOR-GRAMMAR.md`](docs/SELECTOR-GRAMMAR.md).
- **`bindings/node/marxml.d.ts` is the public Node API, not `index.d.ts`.** `index.d.ts` is napi-rs's raw output (the underlying class). Users consume `marxml.d.ts` (the `MarkdownDoc` interface). Read and write `marxml.d.ts`.
- **napi-rs needs a build before tests.** `pnpm test` in `bindings/node/` runs vitest against the generated `marxml.mjs` and the local `.node` binary. Run `pnpm run build:debug` first or the tests load a stale binary.
- **Don't `cd bindings/node && cargo build`.** The crate lives at workspace root; build from there. `pnpm` commands are rooted at `bindings/node/` because the npm workspace is local to that dir.
- **Snapshot tests via `insta`.** When fixtures legitimately change, review the diff carefully and accept with `cargo insta review` — never bulk-accept.
- **Editing `Cargo.toml` / `package.json` version fields by hand.** Don't. Add a changeset; knope handles the bump and CI guards parity via `scripts/check-versions.sh`.
- **`unsafe_code = "forbid"` is workspace-wide.** napi-rs internals are fine; your code isn't. If you think you need `unsafe`, you don't.
