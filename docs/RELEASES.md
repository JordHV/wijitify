# Releases

## Versioning

Semantic versions in `Cargo.toml`. Git tags: `v0.1.0`, `v0.2.0`, … Never reuse a published tag.

## CI

On push/PR to `main`: fmt, clippy (`-D warnings`), test, release build, Flatpak validation.

## Tag release

1. Update `CHANGELOG.md` and `Cargo.toml` version.
2. Tag `vX.Y.Z` and push the tag.
3. `release.yml` verifies tag == crate version, builds `.flatpak` + SHA256, publishes a GitHub Release.

## Flatpak

V0 ships a downloadable `.flatpak` **bundle** for manual install. Continuous updates need Flathub or a self-hosted OSTree remote (post-V0).

```bash
flatpak install --user ./wijitify-v0.1.0.flatpak
flatpak run io.github.JordHV.Wijitify
```

## Update UX (post-V0)

App may notify via GitHub Releases API. Actual install uses the Flatpak remote / OS updater — the sandboxed app does not replace itself.
