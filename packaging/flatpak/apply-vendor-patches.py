#!/usr/bin/env python3
"""Apply Wijitify vendor patches inside the Flatpak cargo vendor tree."""

from __future__ import annotations

import json
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PATCHES = ROOT / "vendor-patches"


def cargo_home() -> Path:
    env = os.environ.get("CARGO_HOME")
    if env:
        return Path(env)
    # flatpak-builder extracts dest=cargo/vendor under the module build dir
    return Path.cwd() / "cargo"


def relax_file_checksums(crate_dir: Path) -> None:
    """Allow patched sources without dropping the lockfile checksum.

    Cargo compares `.cargo-checksum.json`'s `package` field to `Cargo.lock`.
    Setting `package` to null makes `cargo --locked` fail ("checksum could not
    be calculated"). An empty `files` map skips per-file checks so the patch
    applies. See the cargo vendor checksum format.
    """
    checksum = crate_dir / ".cargo-checksum.json"
    if not checksum.is_file():
        sys.exit(f"missing checksum file: {checksum}")
    data = json.loads(checksum.read_text(encoding="utf-8"))
    package = data.get("package")
    if not isinstance(package, str) or len(package) != 64:
        sys.exit(f"vendor checksum has no package hash: {checksum}")
    checksum.write_text(
        json.dumps({"files": {}, "package": package}) + "\n",
        encoding="utf-8",
    )
    print(f"kept package checksum, cleared file checksums: {checksum}")


def replace_file(dest: Path, source: Path) -> None:
    if not source.is_file():
        sys.exit(f"missing patch source: {source}")
    if not dest.is_file():
        sys.exit(f"missing vendor file to patch: {dest}")
    shutil.copyfile(source, dest)
    print(f"patched: {dest}")


def main() -> None:
    vendor = cargo_home() / "vendor"
    if not vendor.is_dir():
        sys.exit(f"cargo vendor dir not found: {vendor}")

    winit = vendor / "winit-0.30.13"
    replace_file(
        winit
        / "src"
        / "platform_impl"
        / "linux"
        / "wayland"
        / "seat"
        / "text_input"
        / "mod.rs",
        PATCHES / "winit-0.30.13-text_input_mod.rs",
    )
    relax_file_checksums(winit)

    slint_winit = vendor / "i-slint-backend-winit-1.18.1"
    replace_file(
        slint_winit / "clipboard.rs",
        PATCHES / "i-slint-backend-winit-1.18.1-clipboard.rs",
    )
    relax_file_checksums(slint_winit)

    print("vendor patches applied")


if __name__ == "__main__":
    main()
