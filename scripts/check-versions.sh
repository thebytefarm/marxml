#!/usr/bin/env bash
# Verify that the crate, npm package, and (optionally) git tag are all on the
# same version. Run from CI as the first job of any release workflow.
#
# Usage:
#   ./scripts/check-versions.sh                # crate vs npm
#   ./scripts/check-versions.sh v0.1.0         # also assert the tag matches

set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

crate_version=$(awk '
  /^\[workspace\.package\]/ { in_block = 1; next }
  /^\[/                     { in_block = 0 }
  in_block && /^version[[:space:]]*=/ {
    gsub(/[",]/, "")
    print $3
    exit
  }
' "$repo_root/Cargo.toml")

npm_version=$(node -p "require('$repo_root/bindings/node/package.json').version")

echo "crate:  $crate_version"
echo "npm:    $npm_version"

if [[ "$crate_version" != "$npm_version" ]]; then
  echo "✗ crate and npm versions disagree" >&2
  exit 1
fi

if [[ $# -ge 1 ]]; then
  tag="${1#v}"
  echo "tag:    $tag"
  if [[ "$tag" != "$crate_version" ]]; then
    echo "✗ tag does not match crate/npm version" >&2
    exit 1
  fi
fi

echo "✓ versions agree"
