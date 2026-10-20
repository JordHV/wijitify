# Update e2e test (0.2.0 → 0.2.1, VM)

Run inside the VM guest. Once v0.2.1 is published, the Pages `.flatpakref` installs 0.2.1, so install 0.2.0 from its **release bundle** (it carries `--repo-url`, so it still updates from Pages).

## 1. Uninstall old bundle if present

```bash
flatpak kill io.github.JordHV.Wijitify 2>/dev/null
flatpak uninstall --user -y io.github.JordHV.Wijitify || true
flatpak remotes --user --columns=name,url | awk '/jordhv.github.io/ {print $1}' | xargs -r -n1 flatpak remote-delete --user
flatpak remotes --user
```

## 2. Install 0.2.0

```bash
curl -L -o wijitify-v0.2.0.flatpak https://github.com/JordHV/wijitify/releases/download/v0.2.0/wijitify-v0.2.0.flatpak
flatpak install --user -y ./wijitify-v0.2.0.flatpak
flatpak run io.github.JordHV.Wijitify
```

Already on 0.2.0 from Pages? Skip the install and just run it. Check what is installed:

```bash
flatpak info --user io.github.JordHV.Wijitify | grep -i -E 'version|commit'
```

## 3. Confirm Settings shows 0.2.0

Open Settings in the app: the version line must read `0.2.0`.

## 4. See 0.2.1

```bash
curl -s https://jordhv.github.io/wijitify/latest.json   # "version": "0.2.1"
```

In Settings tap **Check now** (or wait for the startup check). Expect `Update available: v0.2.1`.

## 5. Update + Restart

Tap **Update**, wait for the progress to finish, then tap **Restart now**. After relaunch Settings must show `0.2.1` and `Up to date`.

```bash
flatpak info --user io.github.JordHV.Wijitify | grep -i version
```
