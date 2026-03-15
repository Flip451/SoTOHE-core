#!/usr/bin/env python3
"""
Verify that the latest track has complete, non-placeholder artifacts.
"""

from __future__ import annotations

import json
import re
import sys
from datetime import UTC, datetime, time
from pathlib import Path

try:
    from scripts.track_schema import (
        v3_branch_field_missing,
        v3_branchless_track_invalid,
        v3_non_null_branch_invalid,
    )
except ImportError:  # pragma: no cover - script execution path
    from track_schema import (
        v3_branch_field_missing,
        v3_branchless_track_invalid,
        v3_non_null_branch_invalid,
    )

PLACEHOLDER_LINE_RE = re.compile(r"TODO:|TEMPLATE STUB", re.IGNORECASE)
TASK_LINE_RE = re.compile(r"^\s*(?:[-*]|\d+\.)\s+\[(?:[^\]])\]\s+.+")
LIST_MARKER_RE = re.compile(r"^\s*(?:[-*]|\d+\.)\s+")
VERIFICATION_SCAFFOLD_LINES = {
    "scope verified",
    "manual verification steps",
    "result / open issues",
    "verified_at",
    # Japanese equivalents
    "検証範囲",
    "手動検証手順",
    "結果 / 未解決事項",
    "検証日",
}


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def track_dirs(root: Path | None = None) -> list[Path]:
    try:
        from scripts.track_schema import all_track_directories
    except ImportError:  # pragma: no cover - script execution path
        from track_schema import all_track_directories

    repo_root = root or project_root()
    return all_track_directories(repo_root)


def display_path(path: Path, root: Path | None = None) -> str:
    repo_root = root or project_root()
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()


def parse_updated_at(raw_value: str) -> datetime:
    value = raw_value.strip()
    if not value:
        raise ValueError("updated_at must be a non-empty string")
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError:
        parsed = datetime.combine(
            datetime.fromisoformat(value + "T00:00:00").date(), time.min
        )
    if parsed.tzinfo is None:
        return parsed.replace(tzinfo=UTC)
    return parsed.astimezone(UTC)


# Statuses that should be excluded from "latest track" selection.
# Archived tracks are complete and no longer need active verification.
_SKIP_STATUSES = {"archived"}


def selection_priority(status: str, branch: object, schema_version: int) -> int:
    branch_name = branch.strip() if isinstance(branch, str) else ""
    has_branch = bool(branch_name)
    if has_branch and status != "done":
        return 2
    if not has_branch and schema_version != 3 and status not in {"done", "archived"}:
        return 2
    if not has_branch and status == "planned":
        return 1
    return 0


def load_track_metadata(
    track_dir: Path, root: Path | None = None
) -> tuple[datetime, str, object, list[str]]:
    """Load track metadata and return (updated_at, status, branch, errors)."""
    metadata_file = track_dir / "metadata.json"
    if not metadata_file.is_file():
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because metadata.json is missing: {display_path(metadata_file, root)}"
            ],
        )

    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except json.JSONDecodeError as err:
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because metadata.json is invalid: {display_path(metadata_file, root)} ({err})"
            ],
        )
    if not isinstance(data, dict):
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because metadata.json is invalid: {display_path(metadata_file, root)} (metadata.json must be a JSON object)"
            ],
        )

    if v3_branch_field_missing(data):
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because branch is missing: {display_path(metadata_file, root)}"
            ],
        )
    if v3_branchless_track_invalid(data):
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                "[ERROR] Cannot determine latest track because branchless v3 metadata is only valid "
                f"for planning-only tracks: {display_path(metadata_file, root)}"
            ],
        )
    if v3_non_null_branch_invalid(data):
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because branch is invalid: {display_path(metadata_file, root)}"
            ],
        )

    updated_at = data.get("updated_at")
    if not isinstance(updated_at, str):
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because updated_at is missing or invalid: {display_path(metadata_file, root)}"
            ],
        )

    try:
        parsed = parse_updated_at(updated_at)
    except ValueError as err:
        return (
            datetime.min.replace(tzinfo=UTC),
            "",
            None,
            [
                f"[ERROR] Cannot determine latest track because updated_at is invalid: {display_path(metadata_file, root)} ({err})"
            ],
        )
    raw_status = data.get("status", "")
    status = raw_status if isinstance(raw_status, str) else ""
    schema_version = data.get("schema_version", 2)
    if not isinstance(schema_version, int):
        schema_version = 2
    return parsed, status, (data.get("branch"), schema_version), []


def latest_track_dir(root: Path | None = None) -> tuple[Path | None, list[str]]:
    dirs = track_dirs(root)
    if not dirs:
        return None, []

    repo_root = root or project_root()
    archive_root = (repo_root / "track" / "archive").resolve()
    latest_dir: Path | None = None
    latest_rank = (-1, datetime.min.replace(tzinfo=UTC), "")
    errors: list[str] = []
    for dir_path in dirs:
        # Skip tracks in track/archive/ by path regardless of metadata content.
        try:
            dir_path.resolve().relative_to(archive_root)
            continue
        except ValueError:
            pass
        updated_at, status, branch_info, track_errors = load_track_metadata(dir_path, root)
        if track_errors:
            errors.extend(track_errors)
            continue
        if status in _SKIP_STATUSES:
            continue
        branch, schema_version = branch_info
        rank = (selection_priority(status, branch, schema_version), updated_at, dir_path.name)
        if rank > latest_rank:
            latest_dir = dir_path
            latest_rank = rank

    if errors:
        return None, errors
    return latest_dir, []


def placeholder_lines(text: str) -> list[tuple[int, str]]:
    found: list[tuple[int, str]] = []
    in_fence = False
    for line_number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if PLACEHOLDER_LINE_RE.search(line):
            found.append((line_number, line))
    return found


def meaningful_non_heading_lines(text: str) -> list[str]:
    meaningful: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("#"):
            continue
        if stripped.startswith(">"):
            continue
        if re.fullmatch(r"[-*_]{3,}", stripped):
            continue
        meaningful.append(stripped)
    return meaningful


def has_task_items(text: str) -> bool:
    return any(TASK_LINE_RE.match(line) for line in text.splitlines())


def normalize_scaffold_line(line: str) -> str:
    stripped = line.strip()
    stripped = LIST_MARKER_RE.sub("", stripped)
    return stripped.rstrip(":").strip().lower()


def scaffold_placeholder_lines(text: str) -> list[tuple[int, str]]:
    found: list[tuple[int, str]] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("#"):
            continue
        if normalize_scaffold_line(line) in VERIFICATION_SCAFFOLD_LINES:
            found.append((line_number, line))
    return found


def validate_spec_file(path: Path, root: Path | None = None) -> list[str]:
    text = path.read_text(encoding="utf-8")
    errors: list[str] = []
    if not text.strip():
        return [f"[ERROR] Latest track spec.md is empty: {display_path(path, root)}"]
    placeholders = placeholder_lines(text)
    if placeholders:
        errors.append(
            f"[ERROR] Latest track spec.md still contains placeholders: {display_path(path, root)}"
        )
        errors.extend(f"  {line_number}:{line}" for line_number, line in placeholders)
    if not meaningful_non_heading_lines(text):
        errors.append(
            f"[ERROR] Latest track spec.md lacks substantive content beyond headings: {display_path(path, root)}"
        )
    return errors


def validate_plan_file(path: Path, root: Path | None = None) -> list[str]:
    text = path.read_text(encoding="utf-8")
    errors: list[str] = []
    if not text.strip():
        return [f"[ERROR] Latest track plan.md is empty: {display_path(path, root)}"]
    placeholders = placeholder_lines(text)
    if placeholders:
        errors.append(
            f"[ERROR] Latest track plan.md still contains placeholders: {display_path(path, root)}"
        )
        errors.extend(f"  {line_number}:{line}" for line_number, line in placeholders)
    # Task state validation is handled by verify_plan_progress (metadata.json SSoT sync)
    if not has_task_items(text):
        errors.append(
            f"[ERROR] Latest track plan.md does not contain any task items: {display_path(path, root)}"
        )
    return errors


def validate_verification_file(path: Path, root: Path | None = None) -> list[str]:
    text = path.read_text(encoding="utf-8")
    errors: list[str] = []
    if not text.strip():
        return [
            f"[ERROR] Latest track verification.md is empty: {display_path(path, root)}"
        ]
    placeholders = placeholder_lines(text)
    if placeholders:
        errors.append(
            f"[ERROR] Latest track verification.md still contains placeholders: {display_path(path, root)}"
        )
        errors.extend(f"  {line_number}:{line}" for line_number, line in placeholders)
    if not meaningful_non_heading_lines(text):
        errors.append(
            f"[ERROR] Latest track verification.md lacks substantive content beyond headings: {display_path(path, root)}"
        )
    scaffold_lines = scaffold_placeholder_lines(text)
    if scaffold_lines:
        errors.append(
            f"[ERROR] Latest track verification.md still contains scaffold placeholders: {display_path(path, root)}"
        )
        errors.extend(f"  {line_number}:{line}" for line_number, line in scaffold_lines)
    return errors


def main(argv: list[str] | None = None) -> int:
    _ = argv
    print("--- Verify latest track files ---")

    latest_dir, latest_errors = latest_track_dir()
    if latest_errors:
        for error in latest_errors:
            print(error)
        print("--- verify_latest_track_files FAILED ---")
        return 1

    if latest_dir is None:
        print("[OK] No tracks yet. Skipping latest-track file check.")
        print("--- verify_latest_track_files PASSED ---")
        return 0

    required_files = {
        "spec.md": validate_spec_file,
        "plan.md": validate_plan_file,
        "verification.md": validate_verification_file,
    }

    failed = False
    for filename, validator in required_files.items():
        path = latest_dir / filename
        if not path.is_file():
            print(f"[ERROR] Latest track is missing {filename}: {display_path(path)}")
            failed = True
            continue
        for error in validator(path):
            print(error)
            failed = True

    if failed:
        print("--- verify_latest_track_files FAILED ---")
        return 1

    print(
        "[OK] Latest track has complete spec.md, plan.md, and verification.md: "
        + display_path(latest_dir)
    )
    print("--- verify_latest_track_files PASSED ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
