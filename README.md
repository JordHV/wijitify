# Wijitify

> ## ⚠️ WARNING / DISCLAIMER
>
> **This entire project was written by clankers**, with direction given by the human overlords.
>
> It may contain bugs, nonsense, or both. **Use at your own risk.** No warranties, express or implied. Use of this could brick your tablet...

Open-source modular Linux touchscreen dashboard. **Wijitify** is the shell; **Zalender** is the first module (calendar), in the spirit of a family wall calendar — offline-friendly, touch-first, and fast.

Status: V0.3 — calendar-first appliance with shared dark/light UI and public ICS feeds.

## Features (V0)

- Full-screen Slint shell with first-run setup (pick modules, configure Zalender)
- Dark / light theme (Settings), shared Button / Field / Row / Banner kit
- Zalender: public ICS/iCal URL feeds, month + day views, offline cache, action feedback
- Module enable/disable, shared config and logging
- Flatpak packaging, GitHub Actions CI, and in-app updates (Settings → System → Update → Restart)

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

## Icons

UI icons use [Font Awesome Free](https://fontawesome.com) 6 Solid (`assets/fonts/fa-solid-900.ttf`, SIL OFL 1.1). The sidebar toggle uses [Akar Icons](https://github.com/artcoholic/akar-icons) `panel-left` (MIT). Both live in `ui/icons.slint`.

## License

[GPL-3.0-or-later](LICENSE)
