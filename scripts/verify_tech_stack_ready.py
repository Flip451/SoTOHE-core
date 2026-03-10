#!/usr/bin/env python3
"""
Verify that tech stack decisions are fully resolved.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

TECH_STACK_FILE = Path("track/tech-stack.md")
TRACK_ROOT = Path("track/items")
TEMPLATE_DEV_MARKER_FILE = Path(".track-template-dev")

# Matches structured placeholder lines containing TODO: markers.
# Structured lines start with -, |, or 理由: (Markdown list items, table rows,
# or rationale annotations).  TODO: may be wrapped in backticks or bare.
# Examples:
#   - **DB**: `TODO: PostgreSQL / SQLite`
#   - **DB**: TODO: PostgreSQL / SQLite
#   | column   | `TODO: value`
#   | column   | TODO: value
#   理由: `TODO: rationale`
#   理由: TODO: rationale
# Does not match instructional prose that merely mentions "TODO:" in running text
# (lines not starting with a Markdown structural prefix are ignored).
_UNRESOLVED_RE = re.compile(r"^\s*(?:-|\||理由:).*TODO:")


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def is_template_dev_mode(root: Path) -> bool:
    if os.environ.get("TRACK_TEMPLATE_DEV", "").strip() == "1":
        return True
    return (root / TEMPLATE_DEV_MARKER_FILE).is_file()


def has_track_dirs(root: Path) -> bool:
    track_root = root / TRACK_ROOT
    if not track_root.is_dir():
        return False
    return any(p for p in track_root.iterdir() if p.is_dir())


def main(argv: list[str] | None = None) -> int:
    _ = argv
    root = project_root()
    print("--- Verify tech stack readiness ---")

    tech_stack = root / TECH_STACK_FILE
    if not tech_stack.is_file():
        print(f"[ERROR] Missing tech stack file: {TECH_STACK_FILE}")
        return 1

    template_dev = is_template_dev_mode(root)
    tracks_present = has_track_dirs(root)

    if template_dev and not tracks_present:
        print(
            "[OK] Template development mode is enabled and no track directories were "
            "found. Skipping tech stack TODO check."
        )
        return 0

    unresolved = [
        line
        for line in tech_stack.read_text(encoding="utf-8").splitlines()
        if _UNRESOLVED_RE.match(line)
    ]
    if unresolved:
        print(f"[ERROR] Unresolved tech stack TODOs found in {TECH_STACK_FILE}:")
        for line in unresolved:
            print(f"  {line}")
        return 1

    print("[OK] Tech stack has no blocking TODO placeholders.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
