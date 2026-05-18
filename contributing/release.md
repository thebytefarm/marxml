# Release

How `marxml` gets from a merged PR to published artifacts on crates.io and npm. Versioning + publishing run through [knope](https://knope.tech); the Rust crate and the npm package always release in lockstep on the same semver number.

Pre-release crash course → [README warning](../README.md#warning). High-level contributor entry → [CONTRIBUTING.md](../CONTRIBUTING.md). When something fails → [troubleshooting.md → Release](./troubleshooting.md#release).

## Adding a changeset

Every user-facing PR adds a `.changeset/<slug>.md`:

```markdown
---
default: minor    # or `patch` / `major`
---

#### Short summary line

Optional details. Becomes a bullet under the next CHANGELOG section.
```

**Bump-level guide** (knope follows strict pre-1.0 semver: `0.x.y` treats minor changes as patch-equivalent):

| Changeset says | Effect at `0.x.y` | Effect at `≥ 1.0.0` |
| -------------- | ----------------- | ------------------- |
| `patch`        | `0.0.1 → 0.0.2`   | `1.0.0 → 1.0.1`     |
| `minor`        | `0.0.1 → 0.0.2`   | `1.0.0 → 1.1.0`     |
| `major`        | `0.0.1 → 0.1.0`   | `1.0.0 → 2.0.0`     |

Use `major` for breaking changes (pre-1.0 too; that's how you cross `0.x` boundaries). Skip the changeset entirely when your PR is purely internal: refactors with no surface change, tests, docs typos, CI tweaks, dependency bumps that don't change behavior.

## The release flow

Two workflows, chained:

1. **`release-pr.yml`** runs on push to `main` when any `.changeset/`, `Cargo.toml`, or `bindings/node/package.json` change lands. Knope aggregates pending changesets, bumps versions, prepends a new `CHANGELOG.md` section, deletes the consumed changesets, and opens a `chore: release X.Y.Z` PR on a `release/X.Y.Z` branch.
2. **`release.yml`** runs when the release PR merges (detected by the `chore: release ` commit subject). Pipeline: version-sync gate → cross-compile napi bindings for 6 platforms → publish per-platform npm sub-packages → publish main npm package (with provenance) → publish crate to crates.io → tag `vX.Y.Z`.

The tag is the receipt. If `vX.Y.Z` exists on the repo, every artifact for that version shipped. If `release.yml` fails partway (build matrix flake, registry hiccup, etc.) no tag is created. See [troubleshooting → "publish failed partway"](./troubleshooting.md#release-yml-failed-after-some-artifacts-published) for how to recover.

## Cutting a release candidate

Manually dispatch `release-pr.yml` with `prerelease: rc`. From `0.0.0` with a `minor` changeset:

| Stable          | RC                   |
| --------------- | -------------------- |
| `0.0.0 → 0.0.1` | `0.0.0 → 0.0.1-rc.0` |

Subsequent RC dispatches increment the suffix (`rc.0` → `rc.1` → …) until a stable run (no prerelease input) consumes the changesets and produces the final.

To force a specific version, knope supports `--override-version 0.1.0-rc.0` (only useful for the first release, before changesets compute the version naturally).

## Secrets and permissions (one-time setup)

The release pipeline needs two repo secrets and one repo permission:

| Secret                 | Where to get it                                                                     |
| ---------------------- | ----------------------------------------------------------------------------------- |
| `CARGO_REGISTRY_TOKEN` | crates.io → Account Settings → API Tokens → "New Token" (publish scope on `marxml`) |
| `NPM_TOKEN`            | npmjs.com → Access Tokens → "Granular Access Token" (read+publish on `marxml`)      |

Set both with:

```sh
gh secret set CARGO_REGISTRY_TOKEN --repo thebytefarm/marxml
gh secret set NPM_TOKEN            --repo thebytefarm/marxml
```

**GitHub Actions permissions** (Settings → Actions → General → Workflow permissions):

- ☑ Read and write permissions
- ☑ Allow GitHub Actions to create and approve pull requests

The first is set via API (already configured). The second isn't reliably exposed by the API. Check it manually in the UI before the first release.

## First-release walkthrough

1. Confirm secrets + permissions per the table above.
2. Push the `knope.toml` + `.changeset/` + workflow YAMLs to `main`. The push triggers `release-pr.yml`.
3. Within ~30 seconds you should see a PR titled `chore: release 0.0.1` (or whatever the changesets compute) on a `release/0.0.1` branch. Inspect:
   - `Cargo.toml` and `bindings/node/package.json` both bumped to the new version.
   - `CHANGELOG.md` has a new section at the top with your changeset content.
   - The consumed `.changeset/*.md` files are deleted.
4. If the diff looks right, merge the PR. The squash commit on `main` (`chore: release 0.0.1`) triggers `release.yml`.
5. Watch the pipeline: version-sync gate → cross-compile matrix (~5–10 min) → per-platform npm publishes → main npm publish → `cargo publish` → tag `v0.0.1`. Each publish step takes 1–3 min.
6. Verify on the registries:
   - <https://crates.io/crates/marxml>
   - <https://www.npmjs.com/package/marxml>

If anything fails mid-flow, see [troubleshooting.md → Release](./troubleshooting.md#release). Because the tag is created last, a failure leaves no tag to clean up. Fix the issue and re-run the workflow from the Actions UI (the merge commit on `main` still matches `chore: release `). If publish partially succeeded (e.g. npm shipped but crates.io failed), see [troubleshooting → "publish failed partway"](./troubleshooting.md#release-yml-failed-after-some-artifacts-published).

## Local verification

```sh
# Install knope (one-time; or download a release tarball from
# https://github.com/knope-dev/knope/releases)
cargo install knope

knope --validate                                       # config schema check
knope prepare-release --dry-run                        # preview the release PR contents
knope prepare-release --prerelease-label rc --dry-run  # preview RC bump
```

`knope --validate` returns nothing on success. `--dry-run` prints every file write + git command knope would run, without running them.

## What knope does NOT do

- Run tests (CI does; `ci.yml` runs on the release PR like any other PR).
- Push or sign the actual git tag from your machine (the GH Action does it from the workflow runner with `GITHUB_TOKEN`).
- Publish to non-crates.io / non-npm registries.
- Update the 6 per-platform npm sub-packages. `napi version` does that during `release.yml`.
- Update `bindings/node/index.js` / `index.d.ts` (those are regenerated by `napi build` in `release.yml`).
