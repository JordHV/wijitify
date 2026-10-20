#!/usr/bin/env bash
# Local Flatpak repo for fast update iteration (no CI / no GPG).
# Usage: ./scripts/flatpak-dev-repo.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SITE="${WIJITIFY_DEV_SITE:-$HOME/wj-site}"
REPO="$SITE/repo"
PORT="${WIJITIFY_DEV_PORT:-8000}"
APP_ID="io.github.JordHV.Wijitify"
REMOTE="wijitify-dev"

mkdir -p "$REPO"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

echo "Building $APP_ID $VERSION into $REPO …"
flatpak-builder --user --force-clean \
  --default-branch=stable \
  --repo="$REPO" \
  build-dir packaging/flatpak/io.github.JordHV.Wijitify.yml

flatpak build-update-repo --generate-static-deltas "$REPO"

mkdir -p "$SITE"
python3 - <<PY
import json, datetime
from pathlib import Path
Path("$SITE/latest.json").write_text(
    json.dumps({
        "version": "$VERSION",
        "notes": "dev build",
        "published": datetime.date.today().isoformat(),
    }, indent=2) + "\n",
    encoding="utf-8",
)
PY

if ! curl -fsS "http://127.0.0.1:${PORT}/latest.json" >/dev/null 2>&1; then
  echo "Starting http.server on :${PORT} …"
  nohup python3 -m http.server "$PORT" -d "$SITE" >/tmp/wj-site-http.log 2>&1 &
  sleep 1
fi

if ! flatpak remotes --user | grep -q "^${REMOTE}"; then
  flatpak remote-add --user --no-gpg-verify "$REMOTE" "http://127.0.0.1:${PORT}/repo"
fi

if ! flatpak info --user "$APP_ID" >/dev/null 2>&1; then
  flatpak install -y --user "$REMOTE" "$APP_ID"
fi

flatpak override --user \
  --env=WIJITIFY_UPDATE_FEED="http://127.0.0.1:${PORT}/latest.json" \
  --env=WIJITIFY_UPDATE_INTERVAL_SECS=15 \
  "$APP_ID"

echo
echo "Ready. Run:  flatpak run $APP_ID"
echo "Feed:        http://127.0.0.1:${PORT}/latest.json  (version $VERSION)"
echo "Reset env:   flatpak override --user --reset $APP_ID"
