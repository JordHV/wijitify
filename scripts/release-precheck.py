#!/usr/bin/env python3
"""Stop a release before it reaches GitHub Actions.

Run this on Linux (the wijitify-vm guest) from the repo root, after the
version bump and before any git push. Pushing main starts CI. Pushing a
v* tag starts the release. Both must see a tree that already passed here.

Steps, in order (first failure stops the run):

  1. Cargo.toml, CHANGELOG.md, and the Flatpak metainfo name the same version.
     Optional argument vX.Y.Z (or X.Y.Z) must match that version.
  2. The vendor-patch script keeps each crate's package checksum. A null
     package checksum is what broke the v0.3.1 Flatpak build.
  3. cargo-sources.json contains every crates.io checksum from Cargo.lock.
  4. cargo fmt, clippy -D warnings, and tests.
  5. The Flatpak cargo command, offline, against a vendored tree with the
     Wijitify patches applied:

       cargo --offline build --release --locked

Exit 0 means it is safe to commit (if needed) and push. Anything else means
do not push.

Usage:
  python3 scripts/release-precheck.py
  python3 scripts/release-precheck.py v0.3.2
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATCHED = (
    "winit-0.30.13",
    "i-slint-backend-winit-1.18.1",
)


def run(cmd: list[str], env: dict[str, str] | None = None) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=ROOT, check=True, env=env)


def crate_version() -> str:
    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("version"):
            return line.split('"')[1]
    sys.exit("Cargo.toml has no version")


def changelog_version() -> str:
    for line in (ROOT / "CHANGELOG.md").read_text(encoding="utf-8").splitlines():
        if line.startswith("## ["):
            return line.split("[", 1)[1].split("]", 1)[0]
    sys.exit("CHANGELOG.md has no version heading")


def metainfo_version() -> str:
    text = (ROOT / "packaging/flatpak/io.github.JordHV.Wijitify.metainfo.xml").read_text(
        encoding="utf-8"
    )
    needle = '<release version="'
    start = text.find(needle)
    if start < 0:
        sys.exit("metainfo has no release")
    start += len(needle)
    return text[start : text.find('"', start)]


def check_versions(expected: str | None) -> str:
    version = crate_version()
    changelog = changelog_version()
    metainfo = metainfo_version()
    if not (version == changelog == metainfo):
        sys.exit(
            "version mismatch: "
            f"Cargo.toml {version}, CHANGELOG {changelog}, metainfo {metainfo}"
        )
    if expected is not None:
        tag = expected[1:] if expected.startswith("v") else expected
        if tag != version:
            sys.exit(f"tag {expected} does not match crate version {version}")
    print(f"version {version} matches changelog and metainfo")
    return version


def check_patch_script() -> None:
    """The v0.3.1 failure: package checksum set to null, then cargo --locked."""
    with tempfile.TemporaryDirectory(prefix="wijitify-patch-") as tmp:
        home = Path(tmp)
        package_hash = "a" * 64
        crates = {
            "winit-0.30.13": home
            / "vendor"
            / "winit-0.30.13"
            / "src"
            / "platform_impl"
            / "linux"
            / "wayland"
            / "seat"
            / "text_input"
            / "mod.rs",
            "i-slint-backend-winit-1.18.1": home
            / "vendor"
            / "i-slint-backend-winit-1.18.1"
            / "clipboard.rs",
        }
        for crate, dest in crates.items():
            dest.parent.mkdir(parents=True)
            dest.write_text("// vendor\n", encoding="utf-8")
            checksum = home / "vendor" / crate / ".cargo-checksum.json"
            checksum.write_text(
                json.dumps({"package": package_hash, "files": {"src/lib.rs": "b" * 64}})
                + "\n",
                encoding="utf-8",
            )
        env = os.environ.copy()
        env["CARGO_HOME"] = str(home)
        run(
            [sys.executable, "packaging/flatpak/apply-vendor-patches.py"],
            env=env,
        )
        for crate in PATCHED:
            data = json.loads(
                (home / "vendor" / crate / ".cargo-checksum.json").read_text(encoding="utf-8")
            )
            if data.get("package") != package_hash or data.get("files") != {}:
                sys.exit(f"patch script broke checksum contract for {crate}: {data}")
    print("vendor patches keep the package checksum")


def vendored_release_build() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="wijitify-precheck-"))
    try:
        vendor = tmp / "vendor"
        print(f"vendoring into {vendor}", flush=True)
        run(["cargo", "vendor", "--locked", "--versioned-dirs", str(vendor)])
        env = os.environ.copy()
        env["CARGO_HOME"] = str(tmp)
        run([sys.executable, "packaging/flatpak/apply-vendor-patches.py"], env=env)
        for crate in PATCHED:
            data = json.loads(
                (vendor / crate / ".cargo-checksum.json").read_text(encoding="utf-8")
            )
            package = data.get("package")
            if not isinstance(package, str) or len(package) != 64 or data.get("files") != {}:
                sys.exit(f"patched {crate} checksum is not releasable: {data}")
        directory = vendor.as_posix()
        # Same cargo line as packaging/flatpak/io.github.JordHV.Wijitify.yml.
        run(
            [
                "cargo",
                "build",
                "--offline",
                "--release",
                "--locked",
                "--config",
                'source.crates-io.replace-with="vendored-sources"',
                "--config",
                f'source.vendored-sources.directory="{directory}"',
            ]
        )
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main() -> None:
    os.chdir(ROOT)
    expected = sys.argv[1] if len(sys.argv) > 1 else None
    check_versions(expected)
    check_patch_script()
    run([sys.executable, "packaging/flatpak/check_cargo_sources.py"])
    if sys.platform != "linux":
        sys.exit(
            "Release precheck must finish on Linux so the Flatpak Wayland patches compile.\n"
            "From Windows, start the wijitify-vm guest and run this script there.\n"
            "Do not push main or the version tag until it exits 0."
        )
    run(["cargo", "fmt", "--all", "--", "--check"])
    run(["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"])
    run(["cargo", "test", "--all-features"])
    vendored_release_build()
    print("release precheck passed")


if __name__ == "__main__":
    main()
