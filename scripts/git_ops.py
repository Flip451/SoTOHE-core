#!/usr/bin/env python3
"""
Git helper wrappers used by exact cargo-make tasks for automated flows.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path, PurePosixPath

TRANSIENT_AUTOMATION_FILES = (
    ".takt/pending-add-paths.txt",
    ".takt/pending-note.md",
    ".takt/pending-commit-message.txt",
    "tmp/track-commit/add-paths.txt",
    "tmp/track-commit/commit-message.txt",
    "tmp/track-commit/note.md",
    "tmp/track-commit/track-dir.txt",
)
TRANSIENT_AUTOMATION_DIRS = (
    ".takt/handoffs",
    "tmp",
)
GLOB_MAGIC_CHARS = {"*", "?", "[", "]"}


def run_git(args: list[str]) -> int:
    result = subprocess.run(["git", *args], check=False)
    return result.returncode


def ensure_existing_nonempty_file(path: Path, *, label: str) -> int:
    if not path.is_file():
        print(f"[ERROR] Missing {label}: {path}", file=sys.stderr)
        return 1

    content = path.read_text(encoding="utf-8").strip()
    if not content:
        print(f"[ERROR] {label} is empty: {path}", file=sys.stderr)
        return 1

    return 0


def load_stage_paths(path: Path) -> tuple[int, list[str] | None]:
    check = ensure_existing_nonempty_file(path, label="stage path list file")
    if check:
        return check, None

    transient_paths = {PurePosixPath(entry) for entry in TRANSIENT_AUTOMATION_FILES}
    transient_dirs = {PurePosixPath(d) for d in TRANSIENT_AUTOMATION_DIRS}
    stage_paths: list[str] = []
    seen: set[str] = set()

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        entry = raw_line.strip()
        if not entry or entry.startswith("#"):
            continue
        if entry in seen:
            continue

        entry_path = PurePosixPath(entry)
        if entry_path.is_absolute():
            print(
                f"[ERROR] Stage path list must use repo-relative paths: {entry}",
                file=sys.stderr,
            )
            return 1, None
        if ".." in entry_path.parts:
            print(
                f"[ERROR] Stage path list cannot escape the repo root: {entry}",
                file=sys.stderr,
            )
            return 1, None
        if entry in {".", "./"}:
            print(
                f"[ERROR] Stage path list cannot use whole-worktree pathspecs: {entry}",
                file=sys.stderr,
            )
            return 1, None
        if entry.startswith(":"):
            print(
                f"[ERROR] Stage path list cannot use git pathspec magic or shorthand: {entry}",
                file=sys.stderr,
            )
            return 1, None
        if any(char in entry for char in GLOB_MAGIC_CHARS):
            print(
                f"[ERROR] Stage path list cannot use glob patterns: {entry}",
                file=sys.stderr,
            )
            return 1, None
        if any(
            transient_path == entry_path or entry_path in transient_path.parents
            for transient_path in transient_paths
        ):
            print(
                "[ERROR] Stage path list cannot include transient automation files or their parent directories: "
                f"{entry}",
                file=sys.stderr,
            )
            return 1, None
        if any(
            entry_path == td or td in entry_path.parents or entry_path in td.parents
            for td in transient_dirs
        ):
            print(
                "[ERROR] Stage path list cannot include transient automation directories or their contents: "
                f"{entry}",
                file=sys.stderr,
            )
            return 1, None

        seen.add(entry)
        stage_paths.append(entry)

    if not stage_paths:
        print(
            f"[ERROR] Stage path list file has no usable entries: {path}",
            file=sys.stderr,
        )
        return 1, None

    return 0, stage_paths


def add_all() -> int:
    args = ["add", "-A", "--", "."]
    args.extend(f":(exclude){path}" for path in TRANSIENT_AUTOMATION_FILES)
    args.extend(f":(exclude){d}" for d in TRANSIENT_AUTOMATION_DIRS)
    return run_git(args)


def add_from_file(path: Path, *, cleanup: bool) -> int:
    check, stage_paths = load_stage_paths(path)
    if check:
        return check

    code = run_git(["add", "--", *stage_paths])
    if code == 0 and cleanup:
        path.unlink(missing_ok=True)
    return code


def commit_from_file(path: Path, *, cleanup: bool, track_dir: Path | None = None) -> int:
    check = ensure_existing_nonempty_file(path, label="commit message file")
    if check:
        return check

    # Branch guard: if track-dir.txt exists (written by /track:commit),
    # validate the current branch matches the track's expected branch.
    track_dir_file = path.parent / "track-dir.txt" if track_dir is None else None
    effective_track_dir = track_dir
    if effective_track_dir is None and track_dir_file is not None and track_dir_file.is_file():
        raw = track_dir_file.read_text(encoding="utf-8").strip()
        if raw:
            effective_track_dir = Path(raw)

    if effective_track_dir is not None:
        code = _verify_commit_branch(effective_track_dir)
        if code != 0:
            return code

    code = run_git(["commit", "-F", str(path)])
    # Cleanup both commit message and track-dir.txt on success or failure.
    if cleanup:
        if code == 0:
            path.unlink(missing_ok=True)
        if track_dir_file is not None:
            track_dir_file.unlink(missing_ok=True)
    return code


def _verify_commit_branch(track_dir: Path) -> int:
    """Validate that the track directory is valid and branch matches."""
    # Validate path: must be under track/items/<id> with metadata.json
    if not track_dir.is_dir():
        print(f"[ERROR] Track directory not found: {track_dir}", file=sys.stderr)
        return 1
    metadata_file = track_dir / "metadata.json"
    if not metadata_file.is_file():
        print(f"[ERROR] metadata.json not found in: {track_dir}", file=sys.stderr)
        return 1

    try:
        from track_branch_guard import BranchGuardError, verify_track_branch
        from track_resolution import current_git_branch

        root = track_dir.parent.parent.parent  # track/items/<id> -> project root
        branch = current_git_branch(root)
        verify_track_branch(track_dir, current_branch=branch)
    except BranchGuardError as e:
        print(f"[ERROR] Branch guard: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"[ERROR] Branch guard check failed: {e}", file=sys.stderr)
        return 1

    return 0


def note_from_file(path: Path, *, cleanup: bool) -> int:
    check = ensure_existing_nonempty_file(path, label="git note file")
    if check:
        return check

    code = run_git(["notes", "add", "-f", "-F", str(path), "HEAD"])
    if code == 0 and cleanup:
        path.unlink(missing_ok=True)
    return code


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Git wrapper helpers for cargo-make exact tasks."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser(
        "add-all",
        help="Stage the whole worktree except transient automation scratch files.",
    )

    add_parser = subparsers.add_parser(
        "add-from-file",
        help="Stage repo-relative paths listed in a file.",
    )
    add_parser.add_argument("path", type=Path)
    add_parser.add_argument("--cleanup", action="store_true")

    commit_parser = subparsers.add_parser(
        "commit-from-file",
        help="Create a commit using the message stored in a file.",
    )
    commit_parser.add_argument("path", type=Path)
    commit_parser.add_argument("--cleanup", action="store_true")
    commit_parser.add_argument(
        "--track-dir",
        type=Path,
        default=None,
        help="Explicit track directory for branch guard validation.",
    )

    note_parser = subparsers.add_parser(
        "note-from-file",
        help="Attach a git note using the contents of a file.",
    )
    note_parser.add_argument("path", type=Path)
    note_parser.add_argument("--cleanup", action="store_true")

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "add-all":
        return add_all()
    if args.command == "add-from-file":
        return add_from_file(args.path, cleanup=args.cleanup)
    if args.command == "commit-from-file":
        return commit_from_file(args.path, cleanup=args.cleanup, track_dir=args.track_dir)
    if args.command == "note-from-file":
        return note_from_file(args.path, cleanup=args.cleanup)

    parser.error(f"Unknown command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
