# Troubleshooting

Symptoms and fixes for things that go wrong while contributing. Grouped by area. Each symptom is its own heading so you can deep-link to it (right-click → copy link). Answers are collapsed by default (click to expand).

## Release

Things that go wrong in the release pipeline. Background → [contributing/release.md](./release.md).

### `release-pr.yml` fails with `Permission denied to github-actions`

<details>
<summary>Show fix</summary>

**Cause:** Actions can't push to the repo.

**Fix:** Set workflow perms to "Read and write" (Settings → Actions → General → Workflow permissions).

</details>

### `release-pr.yml` succeeds but no PR appears

<details>
<summary>Show fix</summary>

**Cause:** "Allow GitHub Actions to create and approve pull requests" is off.

**Fix:** Toggle it on in the UI (same settings page as above).

</details>

### Knope says "missing front matter" on a `.changeset/*.md` file

<details>
<summary>Show fix</summary>

**Cause:** A `.md` file in `.changeset/` lacks the frontmatter knope expects.

**Fix:** Either add valid frontmatter or remove the file. Knope errors on every `.md` it can't parse.

</details>

### `cargo publish` fails with `crate version is already uploaded`

<details>
<summary>Show fix</summary>

**Cause:** The tag pointed at a version that's already on crates.io.

**Fix:** Bump the version. crates.io is append-only. You can't republish a version, even if yanked.

</details>

### `npm publish` fails with `403 Forbidden — provenance requires OIDC`

<details>
<summary>Show fix</summary>

**Cause:** The workflow lacks `id-token: write`.

**Fix:** Confirmed present in `release.yml`. If you're editing that workflow, don't drop the permission.

</details>

### Per-platform npm package fails to publish

<details>
<summary>Show fix</summary>

**Cause:** `napi version` didn't update its `package.json` before publish.

**Fix:** Inspect `bindings/node/npm/<target>/package.json`. The version must match the main package.

</details>

### `release.yml` failed after some artifacts published

<details>
<summary>Show fix</summary>

**Cause:** `release.yml` publishes per-platform npm → main npm → crate sequentially. If a later step fails, earlier registries already have the version. The tag has not been created yet (tag-last design), so the git side is clean.

**Fix:** Inspect which steps succeeded. Options, ordered by how disruptive they are:

1. **Re-run the workflow.** From the Actions UI, re-run the failed run. Already-published per-platform packages will 4xx on re-publish. That's fine: the napi pre-publish step continues past `EPUBLISHCONFLICT`. The main npm package and crate are gated on the build job succeeding; if either previously succeeded, they'll fail with "version already exists" and the run will halt before tagging. In that case, fall through to the next option.
2. **Tag manually as the receipt.** If every registry actually has the version but the tag step never ran, tag and push manually: `git tag v0.0.1 <merge-sha> && git push origin v0.0.1`. The tag has no triggers attached anymore. Purely a marker.
3. **Burn the version and fix forward.** If something is wrong with what shipped (e.g. main npm published but crate failed because of a Cargo.toml issue), don't try to recover. Add a `patch` changeset describing the fix, let `release-pr.yml` open the next PR, and ship `0.0.2`. crates.io is append-only and npm doesn't reward fighting the registry.

</details>

### Multiple release PRs open at once

<details>
<summary>Show fix</summary>

**Cause:** Two pushes landed close together; knope opened a second PR.

**Fix:** Close the older PR; knope updates the newer one on subsequent pushes.

</details>
