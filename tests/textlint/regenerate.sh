#!/usr/bin/env bash
# Regenerate the reference output that tests/compat.rs checks issen against.
#
#   ./tests/textlint/regenerate.sh
#
# Run it only when you mean to move the compatibility target, and read the
# resulting diff: every changed line is a behaviour change in textlint or a
# behaviour issen must follow.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
fixtures="$here/../fixtures"

npm --prefix "$here" install --no-audit --no-fund

for md in "$fixtures"/*.md; do
  name="$(basename "$md" .md)"
  # textlint records the path it was given; keep it repo-relative so the
  # fixture does not carry anyone's home directory.
  cp "$md" "$here/$name.md"
  ( cd "$here" && npx textlint --format json "$name.md" > "$name.raw.json" ) || true
  node -e '
    const fs = require("fs");
    const [raw, out, name] = process.argv.slice(1);
    const j = JSON.parse(fs.readFileSync(raw, "utf8"));
    for (const f of j) f.filePath = `tests/fixtures/${name}.md`;
    fs.writeFileSync(out, JSON.stringify(j));
  ' "$here/$name.raw.json" "$fixtures/$name.textlint.json" "$name"
  rm -f "$here/$name.md" "$here/$name.raw.json"
done

echo "regenerated $(ls "$fixtures"/*.textlint.json | wc -l | tr -d ' ') fixture(s)"
