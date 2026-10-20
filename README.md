# Wijitify

Open-source modular Linux touchscreen dashboard. **Wijitify** is the shell; **Zalender** is the first module (calendar), in the spirit of a family wall calendar — offline-friendly, touch-first, and fast.

Status: V0 — calendar-first appliance with in-app Flatpak updates.

## Features (V0)

- Full-screen Slint shell with first-run setup (pick modules, configure Zalender)
- Zalender: ICS/iCal URL feeds, month + day views, offline cache
- Module enable/disable, shared config and logging
- Flatpak packaging, GitHub Actions CI, and in-app updates (Settings → Update → Restart)

### Install (Linux)

```bash
flatpak install --user https://jordhv.github.io/wijitify/io.github.JordHV.Wijitify.flatpakref
flatpak run io.github.JordHV.Wijitify
```

## Principles

See [docs/PRINCIPLES.md](docs/PRINCIPLES.md): simple code, low perceived latency, offline-first cache, speed, and grounding in official docs.

## Quick start (Windows)

```powershell
cargo run -- --windowed
```

## Docs

- [Architecture](docs/ARCHITECTURE.md)
- [Development](docs/DEVELOPMENT.md)
- [Releases](docs/RELEASES.md)
- [Project brief](docs/BRIEF.md)

## License

[GPL-3.0-or-later](LICENSE)
