#!/bin/sh
# Install the git hooks this repository expects.
# Run once per clone: sh scripts/install-hooks.sh
set -e

root=$(git rev-parse --show-toplevel)
hooks=$(git rev-parse --git-path hooks)
mkdir -p "$hooks"

for name in commit-msg; do
    cp "$root/scripts/hooks/$name" "$hooks/$name"
    chmod +x "$hooks/$name"
    echo "installed $name"
done

echo
echo "Commit messages are now checked before every commit."
echo "Bypass with --no-verify only to rewrite history you already pushed."
