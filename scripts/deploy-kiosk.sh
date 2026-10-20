#!/usr/bin/env bash
# Rehearse on the VM from the `clean` snapshot, then run on the Surface.
# Installs Flatpak bundle (path as $1), enables autologin hints, autostart, and power settings.
set -euo pipefail

BUNDLE="${1:-}"
if [[ -z "$BUNDLE" || ! -f "$BUNDLE" ]]; then
  echo "Usage: $0 /path/to/wijitify-vX.Y.Z.flatpak" >&2
  exit 1
fi

flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo || true
flatpak install -y --user "$BUNDLE"

APP_ID="io.github.wijitify.Wijitify"
mkdir -p "$HOME/.config/autostart"
cat > "$HOME/.config/autostart/wijitify.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Wijitify
Exec=flatpak run ${APP_ID}
X-GNOME-Autostart-enabled=true
EOF

# Disable screen blank / suspend for a plugged-in wall display (user session).
gsettings set org.gnome.desktop.session idle-delay 0 || true
gsettings set org.gnome.settings-daemon.plugins.power sleep-inactive-ac-type 'nothing' || true
gsettings set org.gnome.settings-daemon.plugins.power sleep-inactive-battery-type 'nothing' || true

echo "Installed. Enable GNOME auto-login in Settings → Users (keep an admin account)."
echo "Maintenance exit: Settings → Quit to desktop (or tap the top-left corner then Quit)."
echo "Verify kernel: uname -r  (expect 'surface')"
echo "Verify touch: systemctl status iptsd"
