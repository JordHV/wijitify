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


def clear_checksum(crate_dir: Path) -> None:
    checksum = crate_dir / ".cargo-checksum.json"
    checksum.write_text(json.dumps({"files": {}, "package": None}) + "\n", encoding="utf-8")
    print(f"cleared checksum: {checksum}")


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
    clear_checksum(winit)

    slint_winit = vendor / "i-slint-backend-winit-1.18.1"
    replace_file(
        slint_winit / "clipboard.rs",
        PATCHES / "i-slint-backend-winit-1.18.1-clipboard.rs",
    )
    clear_checksum(slint_winit)

    print("vendor patches applied")


if __name__ == "__main__":
    main()
