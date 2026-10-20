# Development

## Machines

| Role | Environment |
| --- | --- |
| Day-to-day UI | Windows — `cargo run -- --windowed` |
| Linux / Flatpak | VMware VM `Ubuntu 64-bit 26.04.1 Surfacetesting` (SSH host `wijitify-vm`) |
| Device truth | Surface Book 3 (Ubuntu 26.04 + linux-surface) |

## Windows

```powershell
rustup component add rustfmt clippy
cargo run -- --windowed
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Requires Visual Studio Build Tools (MSVC) for the `x86_64-pc-windows-msvc` target.

Performance logging: `$env:WIJITIFY_PERF = "1"`.

## VM (Linux)

1. Provision once: `scripts/vm-provision.sh` (inside the guest) or `scripts/vm-provision.ps1` from Windows after SSH works.
2. Clone the repo **inside** the guest (not a VMware shared folder).
3. Install Slint deps, flatpak-builder; see script.
4. Snapshot name: `clean`.

SSH config snippet (Windows `~/.ssh/config`):

```
Host wijitify-vm
  HostName 192.168.254.133
  User surfacetest
  IdentityFile ~/.ssh/id_ed25519
```

`vmrun` (VMware Workstation): start/stop, snapshots, `captureScreen`. Prefer SSH for commands.

## Surface

Confirm `uname -r` contains `surface` and `systemctl status iptsd` is active. Install Flatpak + Flathub. Measure performance only on this device.

## Official docs

- [Rust Book](https://doc.rust-lang.org/book/)
- [std](https://doc.rust-lang.org/std/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Clippy](https://rust-lang.github.io/rust-clippy/)
- [docs.rs](https://docs.rs) (match `Cargo.lock`)
- [Slint](https://docs.slint.dev)
- [Tokio](https://tokio.rs/tokio/tutorial)
- [Flatpak](https://docs.flatpak.org)

Offline crate docs: `cargo doc --open`.

## App ID

Flatpak / desktop id: `io.github.JordHV.Wijitify`. The XDG data/config path stays `Wijitify` (`ProjectDirs::from("io", "github", "Wijitify")`).

## In-app updates — fast VM loop

Production checks `latest.json` at startup and every 24h. For local Flatpak iteration:

```bash
# inside the VM, in a git clone (not a shared folder)
./scripts/flatpak-dev-repo.sh
flatpak run io.github.JordHV.Wijitify
```

Each cycle: bump `version` in `Cargo.toml` (no need to commit), rerun the script, then tap **Check now** in Settings (or wait ~15s). Tap **Update**, then **Restart now**.

The script sets:

- `WIJITIFY_UPDATE_FEED=http://127.0.0.1:8000/latest.json`
- `WIJITIFY_UPDATE_INTERVAL_SECS=15`

via `flatpak override`. Clear with:

```bash
flatpak override --user --reset io.github.JordHV.Wijitify
```

### Banner UI on Windows (no Flatpak install/restart)

```powershell
# terminal 1: serve a hand-edited latest.json
python -m http.server 8000

# terminal 2
$env:WIJITIFY_UPDATE_FEED = "http://127.0.0.1:8000/latest.json"
$env:WIJITIFY_UPDATE_INTERVAL_SECS = "15"
cargo run -- --windowed
```

**Update** / **Restart** report “not running as a Flatpak” outside a Flatpak runtime.
