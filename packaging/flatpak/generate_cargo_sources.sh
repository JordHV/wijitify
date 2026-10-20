#!/usr/bin/env bash
# Regenerate cargo-sources.json from Cargo.lock (run on Linux / WSL / CI).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
if [[ ! -f Cargo.lock ]]; then
  cargo generate-lockfile
fi
GEN_URL="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py"
TMP="$(mktemp -d)"
curl -fsSL "$GEN_URL" -o "$TMP/flatpak-cargo-generator.py"
python3 "$TMP/flatpak-cargo-generator.py" Cargo.lock -o packaging/flatpak/cargo-sources.json
echo "Wrote packaging/flatpak/cargo-sources.json"
