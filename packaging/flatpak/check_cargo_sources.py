#!/usr/bin/env python3
"""Fail if cargo-sources.json is missing a crates.io checksum from Cargo.lock."""
from pathlib import Path
import json
import sys

root = Path(__file__).resolve().parents[2]
lock = root / "Cargo.lock"
sources_path = Path(__file__).resolve().parent / "cargo-sources.json"


def lock_checksums(text: str) -> set[str]:
    found: set[str] = set()
    for line in text.splitlines():
        if line.startswith("checksum = "):
            found.add(line.split('"', 2)[1])
    return found


def main() -> None:
    if not lock.is_file():
        print("Cargo.lock missing — run cargo generate-lockfile", file=sys.stderr)
        sys.exit(1)
    if not sources_path.is_file():
        print(
            "cargo-sources.json missing — run packaging/flatpak/generate_cargo_sources.sh",
            file=sys.stderr,
        )
        sys.exit(1)
    needed = lock_checksums(lock.read_text(encoding="utf-8"))
    entries = json.loads(sources_path.read_text(encoding="utf-8"))
    have = {e["sha256"] for e in entries if e.get("type") == "archive" and "sha256" in e}
    missing = needed - have
    if missing:
        print(
            f"cargo-sources.json is missing {len(missing)} crate checksum(s) — "
            "run packaging/flatpak/generate_cargo_sources.sh",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"cargo-sources.json covers {len(needed)} lockfile checksums")


if __name__ == "__main__":
    main()
