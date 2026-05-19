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
2. **`release.yml`** runs when the release PR merges (detected by the `chore: release ` commit subject). Pipeline: version-sync gate → cross-compile napi bindings for each supported platform → publish per-platform npm sub-packages → publish main npm package (with provenance) → publish crate to crates.io → tag `vX.Y.Z`.

The tag is the receipt. If `vX.Y.Z` exists on the repo, every artifact for that version shipped. If `release.yml` fails partway (build matrix flake, registry hiccup, etc.) no tag is created. See [troubleshooting → "publish failed partway"](./troubleshooting.md#release-yml-failed-after-some-artifacts-published) for how to recover.

The npm side of the publish step is non-trivial — seven packages, a `package.json` that gets mutated mid-flight, platform dispatch at runtime. The full mechanics live in [`node-distribution.md`](./node-distribution.md).

## Cutting a release candidate

Manually dispatch `release-pr.yml` with `prerelease: rc`. From `0.0.0` with a `minor` changeset:

| Stable          | RC                   |
| --------------- | -------------------- |
| `0.0.0 → 0.0.1` | `0.0.0 → 0.0.1-rc.0` |

Subsequent RC dispatches increment the suffix (`rc.0` → `rc.1` → …) until a stable run (no prerelease input) consumes the changesets and produces the final.

To force a specific version, knope supports `--override-version 0.1.0-rc.0` (only useful for the first release, before changesets compute the version naturally).

## Secrets and permissions (one-time setup)

Both registries use **trusted publishing** (OIDC). No long-lived tokens in repo secrets. See [npm trusted publishing](#npm-trusted-publishing-one-time) and [crates.io trusted publishing](#trusted-publishing-on-cratesio) below.

### GitHub Actions permissions

Two settings need to be on, both at the **org level** (`thebytefarm`) for the repo-level toggles to stick:

- ☑ Read and write permissions for workflows (org → Settings → Actions → General → Workflow permissions)
- ☑ Allow GitHub Actions to create and approve pull requests (same page)

Once those are on at the org, the repo-level page (Settings → Actions → General → Workflow permissions) inherits. Verify with:

```sh
gh api repos/thebytefarm/marxml/actions/permissions/workflow
# Expect: { "default_workflow_permissions": "write", "can_approve_pull_request_reviews": true }
```

`release-pr.yml` needs both — without write perms knope can't push the release branch; without PR-approval it can't open the PR.

### npm trusted publishing (one-time)

Six npm packages (`marxml` + five platform sub-packages) use trusted publishing. Setup is per-package and one-time. The Windows sub-package is temporarily out of scope — see [Platform coverage](#platform-coverage).

**Step 1 — reserve the platform sub-package names.** The main `marxml` already exists at `0.0.0` on npm. The five platform sub-packages don't yet, and trusted publishing can't be configured on a package that doesn't exist. Use the bundled script:

```sh
# Get a 24h granular token at https://www.npmjs.com/settings/<user>/tokens
# Scope: read+publish on each of the five marxml-<target> names.
npm login

./scripts/reserve-npm-names.sh

# Revoke the token immediately after. You won't need it again.
```

The script publishes a 0-byte placeholder to each of the five names. Idempotent. Re-running on a name that's already at `0.0.0` is a no-op.

**Step 2 — configure trusted publishing on each of the 6 packages.** For each settings page (the script prints the URLs):

- <https://www.npmjs.com/package/marxml/access>
- <https://www.npmjs.com/package/marxml-darwin-arm64/access>
- <https://www.npmjs.com/package/marxml-darwin-x64/access>
- <https://www.npmjs.com/package/marxml-linux-arm64-gnu/access>
- <https://www.npmjs.com/package/marxml-linux-x64-gnu/access>
- <https://www.npmjs.com/package/marxml-linux-x64-musl/access>

Add a trusted publisher with:

- **Publisher:** GitHub Actions
- **Owner:** `thebytefarm`
- **Repository:** `marxml`
- **Workflow filename:** `release.yml`
- **Environment:** leave blank (or set `release` only if you also create a GitHub Actions environment by that name. See [Manual approval gate](#manual-approval-gate-optional))

`release.yml` has `id-token: write` and runs `npm install -g npm@latest` before publishing, so the npm CLI auto-detects OIDC. `NPM_CONFIG_PROVENANCE: true` is set on both publish steps so all 6 packages ship with provenance.

### Platform coverage

Currently five platform sub-packages — macOS (arm64, x64) and Linux (x64-gnu, x64-musl, arm64-gnu). Windows (`marxml-win32-x64-msvc`) is **temporarily disabled** because the name reservation was blocked by npm's spam-detection heuristic during the initial burst publish. The block is name-shape based (the canonical napi-rs `<pkg>-<platform>-<arch>-<abi>` pattern from a new account) — not anything we can fix in code.

Path to re-enable:

1. Open a support ticket at <https://www.npmjs.com/support> asking npm to publish `marxml-win32-x64-msvc@0.0.0` and transfer write access to your account. There's clear precedent ([Node-RED forum case](https://discourse.nodered.org/t/problems-with-npm-publish-why-is-my-node-spam/40229)) — same-week turnaround. Current ticket: **#4396187**.
2. Once unblocked, restore `x86_64-pc-windows-msvc` to `bindings/node/package.json#napi.targets` and the matching matrix entry in `.github/workflows/release.yml#jobs.build.strategy.matrix.include`.
3. Configure trusted publishing on the new package's settings page (<https://www.npmjs.com/package/marxml-win32-x64-msvc/access>) with the same owner/repo/workflow values as the other five.

The `bindings/node/npm/win32-x64-msvc/` directory is intentionally kept on disk — re-enabling Windows is a 2-line config change once the name is unblocked.

### Trusted publishing on crates.io

Enabled. Same OIDC model as npm. Configured at <https://crates.io/crates/marxml/settings> → Trusted Publishers (owner `thebytefarm`, repo `marxml`, workflow `release.yml`). `release.yml` uses `rust-lang/crates-io-auth-action@v1.0.4` to mint a short-lived token via OIDC, then runs `cargo publish` with that token in `CARGO_REGISTRY_TOKEN`. No long-lived registry secret in the repo.

### Manual approval gate (optional)

For extra safety, gate the `publish` job behind a GitHub Actions environment with required reviewers:

1. Settings → Environments → New environment → `release`. Add yourself as required reviewer; restrict deployments to `main`.
2. Add `environment: release` to the `publish` job in `release.yml`.
3. Add `release` as the **Environment** value in each of the 6 npm trusted publisher configs and the crates.io one.

Every release will pause for your one-click approval before any registry call. Recommended once the repo has more than one publisher.

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
