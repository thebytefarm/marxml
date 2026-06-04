#!/usr/bin/env sh
# Reset every example by clearing each gitignored out/ directory.
# Input fixtures live under each example's samples/ and are never written to.
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
for out in "$DIR"/*/out; do
  if [ -d "$out" ]; then
    rm -rf "$out"
    echo "cleared $out"
  fi
done
