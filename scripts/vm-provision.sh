#!/usr/bin/env bash
# Run inside the Surfacetesting Ubuntu 26.04 guest (as surfacetest with sudo).
set -euo pipefail

sudo apt-get update
sudo apt-get install -y \
  open-vm-tools open-vm-tools-desktop \
  openssh-server git curl build-essential pkg-config \
  libfontconfig1-dev libxkbcommon-dev libwayland-dev \
  libx11-dev libxcursor-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-x11-dev \
  flatpak flatpak-builder

sudo systemctl enable --now ssh

if ! command -v rustup >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
rustup component add rustfmt clippy

flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo || true

mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"
if [[ -f /tmp/wijitify_id_ed25519.pub ]]; then
  touch "$HOME/.ssh/authorized_keys"
  grep -qf /tmp/wijitify_id_ed25519.pub "$HOME/.ssh/authorized_keys" \
    || cat /tmp/wijitify_id_ed25519.pub >> "$HOME/.ssh/authorized_keys"
  chmod 600 "$HOME/.ssh/authorized_keys"
fi

echo "Provision complete. Kernel: $(uname -r)"
echo "Install openssh done. Add Host wijitify-vm in ~/.ssh/config on Windows."
