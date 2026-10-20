# Deploy (VM rehearsal → Surface)

## 1. VM prerequisites

Guest user from autoinstall: `surfacetest`. Password is whatever you set at install time.

From Windows (with the guest password):

```powershell
.\scripts\vm-provision.ps1 -GuestPassword 'YOUR_PASSWORD'
ssh wijitify-vm
```

Then inside the guest, clone the repo (not a VMware shared folder), build, and take a VMware snapshot named `clean`:

```powershell
& "C:\Program Files (x86)\VMware\VMware Workstation\vmrun.exe" snapshot "C:\Users\jordh\Documents\Virtual Machines\Ubuntu 64-bit 26.04.1 Surfacetesting\Ubuntu 64-bit 26.04.1 Surfacetesting.vmx" clean
```

## 2. Install from a Flatpak bundle

```bash
chmod +x scripts/deploy-kiosk.sh
./scripts/deploy-kiosk.sh ./wijitify-v0.1.0.flatpak
```

Enable GNOME auto-login in Settings → Users. Keep a normal admin account for maintenance.

Quit path: Settings → **Quit to desktop**, or tap the top-left corner then Quit.

## 3. Surface checks

```bash
./scripts/surface-verify.sh
```

Expect `uname -r` to contain `surface` and `iptsd` active. Touch and performance numbers only count on the Surface, not the VM.
