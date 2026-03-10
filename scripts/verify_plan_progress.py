#!/usr/bin/env python3
"""
Verify track plan.md is in sync with metadata.json (SSoT).

All tracks must use schema_version 2. plan.md is a read-only view
rendered from metadata.json via render_plan().
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

from track_markdown import render_plan
from track_schema import parse_metadata_v2, validate_metadata_v2

_CONVENTION_SECTION_RE = re.compile(r"^##\s+Related Conventions", re.MULTILINE)
_CONVENTION_PATH_RE = re.compile(
    r"^-\s+`(project-docs/conventions/[^`]+)`", re.MULTILINE
)


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def extract_convention_paths(plan_text: str) -> list[str]:
    """Extract convention file paths from the 'Related Conventions' section of plan.md."""
    match = _CONVENTION_SECTION_RE.search(plan_text)
    if match is None:
        return []
    section_start = match.end()
    # Find next ## heading or end of text
    next_heading = re.search(r"^##\s", plan_text[section_start:], re.MULTILINE)
    section_text = (
        plan_text[section_start : section_start + next_heading.start()]
        if next_heading
        else plan_text[section_start:]
    )
    return _CONVENTION_PATH_RE.findall(section_text)


def validate_convention_paths(plan_text: str, root: Path) -> list[str]:
    """Check that convention paths listed in plan.md exist on disk."""
    paths = extract_convention_paths(plan_text)
    errors: list[str] = []
    for p in paths:
        # Reject path traversal
        if ".." in p:
            errors.append(f"  [ERROR] Convention path contains '..': {p}")
            continue
        if not (root / p).is_file():
            errors.append(
                f"  [ERROR] Convention path listed in plan.md does not exist: {p}"
            )
    return errors


def track_dirs(root: Path | None = None) -> list[Path]:
    repo_root = root or project_root()
    track_root = repo_root / "track" / "items"
    if not track_root.exists():
        return []
    return sorted(path for path in track_root.iterdir() if path.is_dir())


def validate_track(dir_path: Path) -> list[str]:
    """Validate a single track directory.

    Checks that metadata.json exists, is valid v2 schema, and that
    plan.md matches the rendered output from metadata.json.
    """
    metadata_file = dir_path / "metadata.json"
    plan_file = dir_path / "plan.md"

    if not metadata_file.is_file():
        return [f"  [ERROR] Missing metadata.json: {metadata_file.as_posix()}"]

    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError, UnicodeDecodeError) as exc:
        return [
            f"  [ERROR] Cannot read metadata.json ({type(exc).__name__}): {metadata_file.as_posix()}"
        ]

    if not isinstance(data, dict):
        return [
            f"  [ERROR] metadata.json root must be an object: {metadata_file.as_posix()}"
        ]

    sv = data.get("schema_version")
    if sv != 2:
        return [
            f"  [ERROR] schema_version must be 2 (got {sv!r}) in {metadata_file.as_posix()}"
        ]

    # Full v2 schema validation
    track_dir_name = dir_path.name
    schema_errors = validate_metadata_v2(data, track_dir_name=track_dir_name)
    if schema_errors:
        return [f"  [ERROR] {e}" for e in schema_errors]

    if not plan_file.is_file():
        return [f"  [ERROR] Missing plan: {plan_file.as_posix()}"]

    meta = parse_metadata_v2(data)
    expected = render_plan(meta)
    try:
        actual = plan_file.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        return [
            f"  [ERROR] Cannot read plan.md ({type(exc).__name__}): {plan_file.as_posix()}"
        ]

    if actual != expected:
        return [
            f"  [ERROR] plan.md is out of sync with metadata.json (SSoT) in {dir_path.name}",
            "  [ERROR] Read-only violation: plan.md must not be edited directly.",
            "  [ERROR] SSoT guidance: edit metadata.json via transition_task() / add_task(),",
            "          then run sync_rendered_views() to regenerate plan.md.",
        ]

    results = ["  [OK] plan.md is in sync with metadata.json (SSoT)"]

    # Convention path existence check
    repo_root = dir_path.parent.parent.parent  # track/items/<id> -> repo root
    conv_errors = validate_convention_paths(actual, repo_root)
    if conv_errors:
        results.extend(conv_errors)
    return results


def main(argv: list[str] | None = None) -> int:
    _ = argv
    print("--- Verify plan progress consistency ---")

    dirs = track_dirs()
    if not dirs:
        print("[OK] No track directories found. Skipping progress checks.")
        return 0

    failed = False
    for dir_path in dirs:
        print(f"Checking track: {dir_path.as_posix()}")
        for line in validate_track(dir_path):
            print(line)
            if "[ERROR]" in line:
                failed = True

    if failed:
        print("--- verify_plan_progress FAILED ---")
        return 1

    print("--- verify_plan_progress PASSED ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
