#!/usr/bin/env python3
"""
Verify each track has required metadata.json with v2 or v3 schema.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from track_schema import validate_metadata_v2


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def validate_metadata(metadata_file: Path) -> list[str]:
    """Return a list of result lines for one metadata.json file.

    Lines prefixed with ``[ERROR]`` indicate failures; ``[OK]`` indicates success.
    All tracks must use schema_version 2 or 3.
    """
    try:
        with open(metadata_file, encoding="utf-8") as handle:
            data = json.load(handle)
    except (json.JSONDecodeError, OSError, UnicodeDecodeError) as exc:
        return [
            f"  [ERROR] Cannot read metadata.json ({type(exc).__name__}): {metadata_file}"
        ]

    if not isinstance(data, dict):
        return [f"  [ERROR] metadata.json root must be an object: {metadata_file}"]

    sv = data.get("schema_version")
    if sv not in (2, 3):
        return [f"  [ERROR] schema_version must be 2 or 3 (got {sv!r}) in {metadata_file}"]

    track_dir_name = metadata_file.parent.name
    schema_errors = validate_metadata_v2(data, track_dir_name=track_dir_name)
    if schema_errors:
        return [f"  [ERROR] {e}" for e in schema_errors]
    return ["  [OK] v2 schema validation passed"]


def main(argv: list[str] | None = None) -> int:
    _ = argv
    root = project_root()
    print("--- Verify track metadata ---")

    track_root = root / "track" / "items"
    if not track_root.is_dir():
        print("[OK] No track directories found. Skipping metadata checks.")
        return 0

    track_dirs = sorted(p for p in track_root.iterdir() if p.is_dir())
    if not track_dirs:
        print("[OK] No track directories found. Skipping metadata checks.")
        return 0

    failed = False
    for dir_path in track_dirs:
        metadata_file = dir_path / "metadata.json"
        print(f"Checking track metadata: {dir_path.relative_to(root)}")
        if not metadata_file.is_file():
            print(f"  [ERROR] Missing metadata.json: {metadata_file.relative_to(root)}")
            failed = True
            continue
        lines = validate_metadata(metadata_file)
        for line in lines:
            print(line)
        if any("[ERROR]" in line for line in lines):
            failed = True

    if failed:
        print("--- verify_track_metadata FAILED ---")
        return 1
    print("--- verify_track_metadata PASSED ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
