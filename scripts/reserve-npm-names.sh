#!/usr/bin/env bash
# One-time: publish 0.0.0 placeholders for each marxml-<target> sub-package.
#
# Why: trusted publishing on npm requires the package to already exist before
# you can configure a trusted publisher on its settings page. This script
# reserves the six platform sub-package names so the next step
# (configuring trusted publishing) is unblocked.
#
# Prerequisites:
#   - A short-lived granular npm token in `npm login` state with publish access
#     to `marxml-darwin-arm64`, `marxml-darwin-x64`, `marxml-linux-arm64-gnu`,
#     `marxml-linux-x64-gnu`, `marxml-linux-x64-musl`, `marxml-win32-x64-msvc`.
#     Generate at https://www.npmjs.com/settings/<user>/tokens (Granular Access,
#     publish+read on those six names, 24h expiration). Burn the token after.
#
# What it does, per sub-package:
#   1. Touches a 0-byte `.node` placeholder so the `files` array resolves.
#   2. Runs `npm publish --access public` from the sub-package dir.
#   3. Removes the placeholder.
#
# What it doesn't do:
#   - Reserve the main `marxml` name (already at 0.0.0 on npm).
#   - Configure trusted publishing — you do that in the npmjs.com UI afterward.
#     The script prints the settings URLs you'll need.
#
# Idempotent: if a name is already at 0.0.0, the `npm publish` returns a
# `cannot publish over the previously published versions` error and the script
# treats it as a skip. Any other error (auth, spam detection, network) is
# reported and the script continues to the next package so you can see the
# full picture — exit code is non-zero if any non-skip error happened.

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
npm_dir="$repo_root/bindings/node/npm"

if [[ ! -d "$npm_dir" ]]; then
  echo "✗ $npm_dir not found" >&2
  exit 1
fi

if ! npm whoami >/dev/null 2>&1; then
  echo "✗ Not logged in to npm. Run \`npm login\` first." >&2
  exit 1
fi

echo "Reserving placeholder packages on npm as $(npm whoami)..."
echo

# Anti-spam-detection knobs:
#   - PUBLISH_DELAY (seconds, default 8): pause between publishes so the
#     account doesn't trip the "rapid-fire similar names" heuristic.
#   - Order is shuffled each run via `sort -R`, so the same package isn't
#     always last (the position the spam check tends to flag). On retries,
#     already-published packages skip instantly, so a previously failing
#     package gets a fresh shot from a different position.
delay="${PUBLISH_DELAY:-8}"

failed=()
subs=()
while IFS= read -r line; do
  subs+=("$line")
done < <(find "$npm_dir" -mindepth 1 -maxdepth 1 -type d | sort -R)

for i in "${!subs[@]}"; do
  sub="${subs[$i]}"
  target="$(basename "$sub")"
  pkg_name="marxml-${target}"
  binary="marxml.${target}.node"

  echo "── $pkg_name ──"

  pushd "$sub" >/dev/null
  touch "$binary"

  if output=$(npm publish --access public 2>&1); then
    echo "  ✓ published"
  elif echo "$output" | grep -qiE "cannot publish over the previously published versions|cannot publish over previously published|EPUBLISHCONFLICT"; then
    echo "  · already exists (skipping)"
  else
    echo "  ✗ failed:"
    echo "$output" | sed 's/^/      /'
    failed+=("$pkg_name")
  fi

  rm -f "$binary"
  popd >/dev/null

  # Don't sleep after the last one
  if [[ $i -lt $((${#subs[@]} - 1)) ]]; then
    sleep "$delay"
  fi
  echo
done

if [[ ${#failed[@]} -gt 0 ]]; then
  echo "✗ ${#failed[@]} package(s) failed to publish:"
  for pkg in "${failed[@]}"; do
    echo "    - $pkg"
  done
  echo
  echo "Re-run this script after addressing the errors above. The packages"
  echo "that already published will be skipped automatically."
  echo
  echo "Common causes:"
  echo "  - npm spam detection on rapid-fire publishes from a new account."
  echo "    Wait 5-10 minutes and retry. If it persists, contact npm support."
  echo "  - Granular token missing publish scope on this specific name."
  echo "  - Network or registry hiccup. Just retry."
  exit 1
fi

echo "Done. Next steps:"
echo
echo "  1. Configure trusted publishing on each package:"
for sub in "$npm_dir"/*/; do
  target="$(basename "$sub")"
  echo "       https://www.npmjs.com/package/marxml-${target}/access"
done
echo "       https://www.npmjs.com/package/marxml/access"
echo
echo "     For each, add a trusted publisher with:"
echo "       Owner:    thebytefarm"
echo "       Repo:     marxml"
echo "       Workflow: release.yml"
echo "       Env:      release   (optional but recommended)"
echo
echo "  2. Revoke the granular token you used here:"
echo "       https://www.npmjs.com/settings/$(npm whoami)/tokens"
