#!/usr/bin/env python3
"""Fail if cargo-sources.json is missing or older than Cargo.lock."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[2]
lock = root / "Cargo.lock"
sources = Path(__file__).resolve().parent / "cargo-sources.json"

if not lock.is_file():
    print("Cargo.lock missing — run cargo generate-lockfile", file=sys.stderr)
    sys.exit(1)
if not sources.is_file():
    print(
        "cargo-sources.json missing — run packaging/flatpak/generate_cargo_sources.sh",
        file=sys.stderr,
    )
    sys.exit(1)
if sources.stat().st_mtime < lock.stat().st_mtime:
    print(
        "cargo-sources.json is older than Cargo.lock — regenerate it",
        file=sys.stderr,
    )
    sys.exit(1)
print("cargo-sources.json looks current")
