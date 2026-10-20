#!/usr/bin/env bash
set -euo pipefail
echo "=== kernel ==="
uname -r
echo "=== iptsd ==="
systemctl is-active iptsd || systemctl status iptsd --no-pager || true
echo "=== flatpak ==="
command -v flatpak && flatpak --version
echo "=== display ==="
echo "WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset} XDG_SESSION_TYPE=${XDG_SESSION_TYPE:-unset}"
