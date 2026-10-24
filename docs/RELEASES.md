# Releases

## Versioning

Semantic versions in `Cargo.toml`. Git tags: `v0.1.0`, `v0.2.0`, … Never reuse a published tag.

## CI

On push/PR to `main`: fmt, clippy (`-D warnings`), test, release build, Flatpak validation.

## Tag release

1. Bump `Cargo.toml`, the top of `CHANGELOG.md`, and the first `<release>` in `packaging/flatpak/io.github.JordHV.Wijitify.metainfo.xml` to the same new version. Never reuse a tag, including one whose GitHub Release failed (`v0.3.1` did not publish).
2. On Linux, from the repo root, run `python3 scripts/release-precheck.py vX.Y.Z`. It must exit 0. That script checks version agreement, the Flatpak vendor-patch checksums, `cargo-sources.json`, fmt, clippy, tests, and `cargo --offline build --release --locked` against a vendored tree with those patches applied — the command the Flatpak build runs. From Windows, start the `wijitify-vm` guest and run it there. Do not push until it exits 0: pushing `main` starts CI, and pushing the tag starts the release.
3. Commit, push `main`, tag `vX.Y.Z`, and push the tag.
4. `release.yml` verifies tag == crate version, builds a signed OSTree repo onto the `gh-pages` branch (GitHub Pages), writes `latest.json` + `.flatpakref`, and publishes a GitHub Release with a `.flatpak` bundle that includes `--repo-url` so bundle installs can still update.

## Install (preferred — updateable)

```bash
flatpak install --user https://jordhv.github.io/wijitify/io.github.JordHV.Wijitify.flatpakref
flatpak run io.github.JordHV.Wijitify
```

Or from a release bundle (also remembers the remote when built with `--repo-url`):

```bash
flatpak install --user ./wijitify-v0.2.0.flatpak
flatpak run io.github.JordHV.Wijitify
```

Use `--user` installs so in-app updates never ask for an admin password.

## In-app updates

Settings shows current version and, when `latest.json` reports a newer one, an **Update** button. Install goes through the Flatpak portal; **Restart now** relaunches with `SpawnFlags::LatestVersion`.

- Production checks: once at startup, then every 24 hours.
- Feed URL: `https://jordhv.github.io/wijitify/latest.json` (override with `WIJITIFY_UPDATE_FEED`).
- Interval override: `WIJITIFY_UPDATE_INTERVAL_SECS` (dev only).

Do **not** change Flatpak `finish-args` without a manual `flatpak update` path — the portal refuses permission-expanding updates.

## Signing

Releases are signed with the `Wijitify Releases` GPG key. Public key: [`packaging/flatpak/wijitify.gpg`](../packaging/flatpak/wijitify.gpg). Private key lives in the `FLATPAK_GPG_PRIVATE_KEY` GitHub Actions secret.
