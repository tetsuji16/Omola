#!/bin/sh
# Create GitHub issues from docs/issues/gui/G*.md (one issue per file).
# Requires an authenticated GitHub CLI: `gh auth login`.
# Safe to re-run: files whose title already exists as an open or closed issue are skipped.
#
# Usage: scripts/create-gui-issues.sh [--dry-run]
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
repo="${OMOLA_REPO:-tetsuji16/Omola}"
dry=""
[ "${1:-}" = "--dry-run" ] && dry=1

if [ -z "$dry" ]; then
  gh label create gui --repo "$repo" --color 1D76DB \
    --description "Native egui GUI (crates/omola-gui)" 2>/dev/null || true
  existing="$(gh issue list --repo "$repo" --state all --limit 500 --json title --jq '.[].title')"
else
  existing=""
fi

for file in "$root"/docs/issues/gui/G*.md; do
  # First line: "# G01: title" -> "GUI G01: title"
  title="GUI $(head -n 1 "$file" | sed 's/^# //')"
  if printf '%s\n' "$existing" | grep -Fxq "$title"; then
    echo "skip (exists): $title"
    continue
  fi
  rel="docs/issues/gui/$(basename "$file")"
  body="$(cat "$file")

---
Source: \`$rel\` (keep this file and the issue in sync). Shared rules: \`docs/issues/gui/README.md\`."
  if [ -n "$dry" ]; then
    echo "would create: $title"
  else
    printf '%s' "$body" | gh issue create --repo "$repo" --title "$title" --label gui --body-file -
  fi
done
