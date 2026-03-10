#!/usr/bin/env python3
"""
Verify track/registry.md is in sync with metadata.json (SSoT).

registry.md is a read-only view rendered from all track metadata.json files
via render_registry().
"""

from __future__ import annotations

import sys
from pathlib import Path

from track_registry import collect_track_metadata, render_registry


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def verify_registry(root: Path | None = None) -> list[str]:
    """Verify registry.md matches rendered output. Returns result strings ([OK] or [ERROR])."""
    repo_root = root or project_root()
    registry_path = repo_root / "track" / "registry.md"

    if not registry_path.is_file():
        return [f"[ERROR] Missing registry.md: {registry_path.as_posix()}"]

    try:
        actual = registry_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        return [
            f"[ERROR] Cannot read registry.md ({type(exc).__name__}): {registry_path.as_posix()}"
        ]

    tracks = collect_track_metadata(repo_root)
    expected = render_registry(tracks)

    if actual != expected:
        return [
            "[ERROR] registry.md is out of sync with metadata.json (SSoT)",
            "  Run sync_rendered_views() or write_registry() to update.",
        ]

    return ["[OK] registry.md is in sync with metadata.json (SSoT)"]


def main(argv: list[str] | None = None) -> int:
    _ = argv
    print("--- Verify track registry consistency ---")

    results = verify_registry()
    for line in results:
        print(line)

    if any("[ERROR]" in line for line in results):
        print("--- verify_track_registry FAILED ---")
        return 1

    print("--- verify_track_registry PASSED ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
