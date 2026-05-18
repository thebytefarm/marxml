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
# moves on.

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

for sub in "$npm_dir"/*/; do
  target="$(basename "$sub")"
  pkg_name="marxml-${target}"
  binary="marxml.${target}.node"

  echo "── $pkg_name ──"
  (
    cd "$sub"
    touch "$binary"
    if npm publish --access public 2>&1; then
      echo "  ✓ published"
    else
      echo "  · already exists (skipping)"
    fi
    rm -f "$binary"
  )
  echo
done

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
