# Node distribution (napi-rs)

How the `marxml` npm package actually ships — what gets uploaded to the registry, what lands on a user's machine, and what `@napi-rs/cli` mutates at publish time. Background → [contributing/release.md](./release.md). When publishing fails → [troubleshooting.md → Release](./troubleshooting.md#release).

## The split-package model

`marxml` publishes one main package plus one binary-only package per supported platform. A user only downloads two of them — the main package + their platform's binary.

| Package                  | Contents                                                                | Audience               |
| ------------------------ | ----------------------------------------------------------------------- | ---------------------- |
| `marxml`                 | `marxml.mjs` (ESM facade), `marxml.d.ts`, `index.js` / `index.d.ts` (napi-rs platform-dispatch loader), `README.md`. **No `.node` binary.** | Everyone               |
| `marxml-darwin-arm64`    | `marxml.darwin-arm64.node` only                                         | macOS Apple Silicon    |
| `marxml-darwin-x64`      | `marxml.darwin-x64.node` only                                           | macOS Intel            |
| `marxml-linux-arm64-gnu` | `marxml.linux-arm64-gnu.node` only                                      | Linux arm64 glibc      |
| `marxml-linux-x64-gnu`   | `marxml.linux-x64-gnu.node` only                                        | Linux x64 glibc        |
| `marxml-linux-x64-musl`  | `marxml.linux-x64-musl.node` only                                       | Alpine / musl          |

Currently 6 packages (1 main + 5 platforms). Windows (`marxml-win32-x64-msvc`) is temporarily disabled pending an npm spam-detection unblock on the name — see [release.md → Platform coverage](./release.md#platform-coverage) for the re-enable path. The `bindings/node/npm/win32-x64-msvc/` directory remains on disk for that purpose.

**Why split?** The alternative is shipping one fat `marxml` package containing every `.node` binary. Every install drags binaries for platforms the user will never use. The split puts each platform's binary in its own ~70 KB package, and npm's `optionalDependencies` resolver pulls in only the one matching the install machine.

The supported platforms are pinned by `bindings/node/package.json#napi.targets`. Adding a target is an [Ask First](../AGENTS.md#ask-first) action — it requires a new `bindings/node/npm/<target>/package.json` sub-package, a matching CI matrix entry in `.github/workflows/release.yml`, and a trusted publisher config on npmjs.com.

## Anatomy of each package

### Main `marxml`

Source: `bindings/node/`. The `files` field in `package.json` controls exactly what ships:

```json
"files": ["marxml.mjs", "marxml.d.ts", "index.js", "index.d.ts", "README.md"]
```

Deliberately no `*.node` glob — see [Gotchas](#gotchas) for why that matters.

### Platform sub-packages

Source: `bindings/node/npm/<target>/`. Each one is a stub `package.json` that publishes a single `.node` file:

```jsonc
{
  "name": "marxml-darwin-arm64",
  "version": "0.0.0",
  "main": "marxml.darwin-arm64.node",
  "files": ["marxml.darwin-arm64.node"],
  "os": ["darwin"],
  "cpu": ["arm64"],
  // ... license, repo, bugs, author copied from main
}
```

The `.node` binary itself is **not in git**. It's produced by the cross-compile matrix in `release.yml` and moved into the sub-package dir by `napi artifacts` at publish time (see next section).

## What `@napi-rs/cli` does at publish time

The CI publish step runs four `napi` commands in sequence. Each does something specific to the source tree on the runner; combined, they transform the in-tree representation into the published one.

### 1. `napi build --platform --release --target <triple> --js-package-name marxml`

Runs once per platform (parallel matrix jobs). Builds `marxml.<triple>.node` and — only on one of the jobs, since it's identical across platforms — generates two files alongside the binary:

- `index.js` — the platform-dispatch loader. Reads `process.platform`, `process.arch`, and runs musl detection on Linux (three-stage probe: `/usr/bin/ldd` filesystem read → `process.report.getReport().sharedObjects` → `ldd --version` subprocess). Once it picks a triple, calls `require('marxml-<triple>')`.
- `index.d.ts` — the corresponding TypeScript surface (just re-exports from the underlying napi class).

`--js-package-name marxml` tells the generated `index.js` what name to `require()` — that's how `index.js` ends up with `require('marxml-darwin-arm64')` rather than something else. (`packageName` in the `napi` config block controls the published npm name; `--js-package-name` controls the loader's `require()` strings. They're normally identical and you only diverge when the JS facade and the binary ship under different umbrella names — not our case.)

Neither `index.js` nor `package.json` get touched here. The build is a pure artifact producer.

### 2. `napi artifacts`

Runs on the publish job after downloading the matrix outputs into `bindings/node/artifacts/`. Walks the artifacts dir for `*.node` files, parses each filename as `<binaryName>.<triple>.node`, and copies each binary into its matching `bindings/node/npm/<triple>/` dir.

It also writes a copy of each binary alongside the root `package.json` so a local `pnpm pack` of the main package can include them if its `files` glob picks them up. **Our `files` field excludes `.node` deliberately** — otherwise the main `marxml` tarball would balloon by ~420 KB shipping binaries that users get via `optionalDependencies` anyway.

### 3. `napi version`

Reads the current `version` from the root `bindings/node/package.json` and writes it into each `bindings/node/npm/<target>/package.json`. **It does not bump the root.** Knope is responsible for that (during `release-pr.yml`). `napi version` just propagates the already-bumped root version to the six sub-packages so they stay in sync.

After this step, all seven `package.json` files agree on the version. This is what `scripts/check-versions.sh` asserts at the start of `release.yml`.

### 4. `napi pre-publish -t npm --skip-gh-release`

Despite the name, this command does more than "prepare." Two things happen, in this order:

**(a) Injects `optionalDependencies` into the main `package.json`.** Before any publishing, it rewrites `bindings/node/package.json` in place to add:

```json
"optionalDependencies": {
  "marxml-darwin-arm64": "<version>",
  "marxml-darwin-x64": "<version>",
  "marxml-linux-arm64-gnu": "<version>",
  "marxml-linux-x64-gnu": "<version>",
  "marxml-linux-x64-musl": "<version>"
}
```

That block is **not in git** — it's synthesized from `napi.targets` every release. The version it pins is whatever's currently in the root `version` field (set by knope, propagated by `napi version`).

**(b) Publishes the platform sub-packages.** Loops over `napi.targets`, runs `npm publish` from each `npm/<target>/` dir. The main `marxml` package is **not** published by this step — it's published by the subsequent `npm publish --provenance` call in `release.yml`. Ordering matters: sub-packages first, main package second, because the main `package.json` now has `optionalDependencies` pointing at them.

(The `-t` flag here is `--tag-style`, governing how `pre-publish` parses release-commit subjects for the GitHub-release feature. With `--skip-gh-release`, it doesn't matter what you pass. Not to be confused with `--target` from `napi build`.)

## Install flow

```sh
npm install marxml
```

1. npm fetches the main `marxml` package manifest. That manifest (the registry's view, not the source) has the `optionalDependencies` block listing every supported-platform package.
2. For each optional dep, npm reads the platform package's `os` and `cpu` fields from the registry. A package whose constraints don't match the current machine is **silently skipped** — that's the entire point of `optionalDependencies` versus `dependencies`.
3. npm downloads the one matching package. An Apple-Silicon user ends up with:
   ```
   node_modules/
     marxml/                       ← JS + types + loader (~10 KB)
     marxml-darwin-arm64/          ← .node binary (~70 KB)
   ```

   Nothing for the other five platforms.

The same logic runs for `pnpm`, `yarn`, and `bun`.

## Runtime flow

```ts
import { parse } from 'marxml'
```

1. Resolver hits `node_modules/marxml/package.json` → `exports.import` → `./marxml.mjs`.
2. `marxml.mjs` imports `./index.js` (the napi-generated loader).
3. `index.js` runs platform detection:
   - `process.platform` + `process.arch` for the base triple.
   - On Linux, the three-stage musl probe to pick `-gnu` vs `-musl`.
4. With a triple chosen, calls `require('marxml-darwin-arm64')` (or whichever).
5. Node resolves the sibling package via `node_modules/`. Its `main` field points at `marxml.darwin-arm64.node`.
6. Node loads the `.node` file as a native addon. The exports (the napi class with `parse` etc.) come back, and the loader returns them to `marxml.mjs`, which wraps them in the factory facade.

## Inspecting what will actually ship

The published tarball is *not* the in-tree directory — `napi pre-publish` mutates `package.json` before publishing. To see exactly what users will get:

### What the *next* release would publish

```sh
cd bindings/node
pnpm run build:debug                # produce index.js, index.d.ts, and a local .node

# Preview the main package tarball
pnpm pack --dry-run                 # list files only
pnpm pack                           # → marxml-0.0.0.tgz, inspect with `tar tzf`

# Preview the rewritten package.json (with optionalDependencies)
pnpm exec napi pre-publish --dry-run
cat package.json                    # ← now shows the synthesized block
git checkout package.json           # restore — important
```

### What was published last time

```sh
npm view marxml@<version> --json       # full registry manifest
npm view marxml@<version> files        # file list
npm pack marxml@<version>              # download the tarball
tar tzf marxml-<version>.tgz
```

### Per-platform sub-package

```sh
cd bindings/node/npm/darwin-arm64
cp ../../marxml.darwin-arm64.node .    # napi artifacts does this in CI
npm pack --dry-run                     # see what marxml-darwin-arm64 would publish
```

## Environment variables (runtime)

Two env vars affect the runtime loader (`index.js`). Both are off by default.

### `NAPI_RS_NATIVE_LIBRARY_PATH`

If set, the loader skips all platform dispatch and `require()`s exactly the path given. Use it during development against a locally built `.node` you don't want to install through npm:

```sh
NAPI_RS_NATIVE_LIBRARY_PATH=/path/to/marxml.darwin-arm64.node node ./test.mjs
```

Also useful in containers where the auto-detection misidentifies the libc.

### `NAPI_RS_ENFORCE_VERSION_CHECK`

When truthy (and not literally `'0'`), the loader compares the loaded platform sub-package's `package.json` version against the version embedded in the main `index.js` at build time. Throws on mismatch with a "reinstall dependencies" hint.

Off by default because npm's resolver normally handles this — a mismatch only happens if someone manually edits `node_modules/` or pins one of the platform packages directly. Enable it in CI environments where you've seen drift.

## Adding a new platform

Out of scope for routine PRs — this is [Ask First](../AGENTS.md#ask-first) territory because it changes the napi target list and adds a new npm package name that has to be claimed before trusted publishing can be configured.

When approved:

1. **Pick a target triple** from [napi-rs supported targets](https://napi.rs/docs/cli/build#-target--t-triple-).
2. **Add it to `bindings/node/package.json#napi.targets`.**
3. **Create `bindings/node/npm/<platform-arch-abi>/package.json`** modeled on an existing one (`os`, `cpu`, `main`, `files`, version, repo metadata).
4. **Add a matrix entry to `release.yml#jobs.build.strategy.matrix.include`** with the right host runner and `use_cross: true` if the target needs it.
5. **Reserve the npm name** (one-time `npm publish` of a stub from a granular token, or include it in the next regular release with a fallback token — see [release.md → Secrets and permissions](./release.md#secrets-and-permissions-one-time-setup)).
6. **Configure trusted publishing** on the new package's npmjs.com settings page once it exists. Same owner / repo / workflow / environment values as the other six.

## Gotchas

### The `optionalDependencies` block isn't in git

If you `cat bindings/node/package.json` from a fresh checkout, you'll see no `optionalDependencies` field. That's correct — `napi pre-publish` synthesizes it from `napi.targets` at publish time. The source manifest is the seed; the registry manifest is the bloomed version.

### `napi artifacts` copies binaries next to the main `package.json`

`napi artifacts` writes a copy of each `.node` into `bindings/node/` (not just into the sub-package dirs). If the main `files` field included `*.node`, that copy would ship inside the main `marxml` tarball — defeating the entire split-package model. Our `files` field deliberately excludes the glob. **Don't add `*.node` to it.**

### Failed publish leaves a mutated `package.json` on the runner

`napi pre-publish` writes the synthesized `optionalDependencies` block to `package.json` **before** the publish loop. If a sub-package publish fails partway through, the workflow ends with `package.json` already mutated on the runner. This doesn't matter for the next run (the runner is ephemeral; the next workflow starts from a fresh checkout), but it means a manual `pnpm exec napi pre-publish` on your laptop will leave your tree dirty. Run `git checkout bindings/node/package.json` after any local dry-run.

### Provenance scope

`npm publish --provenance` works on any package published from an OIDC-enabled CI. `release.yml` sets `NPM_CONFIG_PROVENANCE: true` as an env var on both publish steps, so provenance covers every package: the main `marxml` (via the direct `npm publish`), and each platform sub-package (via `napi pre-publish`, which inherits `process.env` when shelling out to `npm publish` per target).

### Version mismatch between main and platform packages

`scripts/check-versions.sh` only compares the **crate** version (root `Cargo.toml`) against the **main npm** version (`bindings/node/package.json`). It does not check the six sub-packages — those are kept in sync mechanically by `napi version` at publish time, not asserted by the version-sync gate. If you ever hand-edit a sub-package `version` field (don't), the gate won't catch it; the loader's `NAPI_RS_ENFORCE_VERSION_CHECK` would, if enabled.

### `npm pack --dry-run` doesn't run `napi pre-publish`

A bare `pnpm pack --dry-run` previews the **current source `package.json`**, not the synthesized publish-time one. If you want to preview the actual published manifest, run `napi pre-publish --dry-run` first (which mutates `package.json`), then `pnpm pack --dry-run`, then `git checkout package.json` to restore.
