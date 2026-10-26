# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-10-25

### Added

- Bundled UI fonts (Nunito, Libre Baskerville, IBM Plex Mono, Playfair Display, Oswald, Caveat, Bungee) so the font choice looks the same on every device
- Separate event font size for calendar titles and times (config schema 8; existing font size is copied onto events)
- Repeating local events: day, week, month, quarter, or year, ending never, on a date, or after a count
- Event location on local events and from ICS feeds

## [0.3.2] - 2026-10-24

### Fixed

- Publish the Surface Flatpak keyboard and clipboard fixes. The v0.3.1 tag failed CI before publish: patched vendored crates nulled their package checksum, so `cargo --offline build --release --locked` rejected `Cargo.lock`
- Vendored Flatpak patches keep that package checksum and only skip per-file checks

## [0.3.1] - 2026-10-23

### Fixed

- Surface Ubuntu Flatpak: GNOME on-screen keyboard Backspace (winit text-input delete_surrounding_text)
- Surface Ubuntu Flatpak: cut/copy/paste via wl-clipboard; touch long-press and right-click text edit menu

## [0.3.0] - 2026-10-22

### Added

- Shared Slint visual kit: Theme tokens, Button / Field / Row / Banner, dark and light palettes
- Settings theme, editable component palette, font family and scale (config schema 7)
- Calendar presets: built-in day, week, month, year, and agenda, plus custom presets
- Agenda view (count is weeks of chronological list) and a year view
- Display options: dim past days and shade weekends (on by default)
- Now marker (dot and line) on today's column in day and week grids
- Second sidebar mini-month; event chips with title-first layout
- Per-event local overrides (color, icon, custom fields) stored beside config, not in the ICS cache
- Font Awesome and sidebar icons

### Fixed

- Setup Add feed never copied the name and URL into Rust (feeds stayed empty); Finish flushes a pending row
- Empty-name feeds default to the URL host; an invalid or missing URL shows a banner
- Public Google-style ICS URLs: `#` in the calendar id is normalized to `%23` before fetch

### Changed

- Dashboard top bar: clock, period title with chevrons, Today, and the preset pill
- Month grid: hairline cells, today ring, overflow count; day modal uses a stroked panel
- Day, single-week, and agenda show title then time; month shows title only
- Feed color drives day dots and agenda bars; status reports how many events loaded

## [0.2.1] - 2026-10-20

### Changed

- Confirm in-app update pipeline end-to-end (Settings shows 0.2.1 after Update + Restart)

## [0.2.0] - 2026-10-20

### Added

- In-app updates via a self-hosted Flatpak OSTree repo on GitHub Pages
- Settings: current version, Check now / Update / Restart, progress and errors
- Startup + daily update feed check (`latest.json`); `WIJITIFY_UPDATE_*` overrides for development
- Signed release pipeline publishes `.flatpakref`, `latest.json`, and a repo-aware Flatpak bundle
- `scripts/flatpak-dev-repo.sh` for fast VM update iteration

## [0.1.0] - 2026-10-19

### Added

- Wijitify shell: full-screen Slint UI, navigation, settings, first-run setup
- Module registry with enable/disable and Zalender as the first built-in module
- Zalender: ICS/iCal URL feeds, offline cache, month and day views
- Shared config (versioned TOML), storage, logging, background Tokio runtime
- Flatpak packaging, CI, and release workflows
- Guiding principles and agent rules for development
