#!/usr/bin/env python3
"""
Git helper wrappers used by exact cargo-make tasks for automated flows.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path, PurePosixPath

TRANSIENT_AUTOMATION_FILES = (
    "tmp/track-commit/add-paths.txt",
    "tmp/track-commit/commit-message.txt",
    "tmp/track-commit/note.md",
    "tmp/track-commit/track-dir.txt",
)
TRANSIENT_AUTOMATION_DIRS = ("tmp",)
GLOB_MAGIC_CHARS = {"*", "?", "[", "]"}


def run_git(args: list[str]) -> int:
    result = subprocess.run(["git", *args], check=False)
    return result.returncode


def run_git_capture(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        check=False,
        text=True,
        capture_output=True,
    )


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
            effective_track_dir = _resolve_repo_relative_path(Path(raw))

    if effective_track_dir is not None:
        code = _verify_commit_branch(effective_track_dir)
        if code != 0:
            # Clean up track-dir.txt even on guard failure to prevent stale context.
            if cleanup and track_dir_file is not None:
                track_dir_file.unlink(missing_ok=True)
            return code
    else:
        code = _require_explicit_track_selector_on_non_track_branch()
        if code != 0:
            if cleanup and track_dir_file is not None:
                track_dir_file.unlink(missing_ok=True)
            return code
        # Fallback: auto-detect track from current branch when no explicit
        # track directory is provided.
        code = _verify_branch_by_auto_detection()
        if code != 0:
            return code

    if effective_track_dir is not None:
        code = _validate_planning_only_commit_paths(effective_track_dir)
        if code != 0:
            if cleanup and track_dir_file is not None:
                track_dir_file.unlink(missing_ok=True)
            return code

    code = run_git(["commit", "-F", str(path)])
    # Cleanup both commit message and track-dir.txt on success or failure.
    if cleanup:
        if code == 0:
            path.unlink(missing_ok=True)
        if track_dir_file is not None:
            track_dir_file.unlink(missing_ok=True)
    return code


def _repo_root() -> Path:
    """Return the repository root (directory containing this script's parent)."""
    return Path(__file__).resolve().parent.parent


def _resolve_repo_relative_path(path: Path) -> Path:
    """Resolve repo-relative selectors from tmp scratch files against the repo root."""
    return path if path.is_absolute() else _repo_root() / path


def _safe_repo_items_dir() -> tuple[Path, Path] | None:
    """Return (resolved_repo_root, resolved_items_dir) if track/items is canonical.

    Returns None if track/items resolves to anything other than the literal
    ``<repo_root>/track/items`` path (rejects symlinks that redirect the tree).
    """
    repo_root = _repo_root().resolve()
    canonical = repo_root / "track" / "items"
    items_dir = (_repo_root() / "track" / "items").resolve()
    if items_dir != canonical:
        return None
    return repo_root, items_dir


def _ensure_branch_guard_imports() -> tuple[int, object, object, object]:
    """Import branch guard dependencies, returning (code, BranchGuardError, verify, current_branch).

    Returns (0, ...) on success, (1, None, None, None) on import failure.
    """
    try:
        _scripts_dir = str(Path(__file__).resolve().parent)
        if _scripts_dir not in sys.path:
            sys.path.insert(0, _scripts_dir)
        from track_branch_guard import BranchGuardError, verify_track_branch
        from track_resolution import current_git_branch
    except ImportError as e:
        print(f"[ERROR] Branch guard import failed: {e}", file=sys.stderr)
        return 1, None, None, None
    return 0, BranchGuardError, verify_track_branch, current_git_branch


def _verify_commit_branch(track_dir: Path) -> int:
    """Validate that the track directory is valid and branch matches."""
    # Validate path: must be under track/items/<id> with metadata.json
    if not track_dir.is_dir():
        print(f"[ERROR] Track directory not found: {track_dir}", file=sys.stderr)
        return 1

    # Enforce that track_dir resolves to a path under the repo's own
    # track/items/ directory.  This prevents bypass via external directories
    # (e.g. /tmp/.../track/items/fake) or symlinked track/items/ itself.
    safe = _safe_repo_items_dir()
    if safe is None:
        print(
            "[ERROR] track/items/ resolves outside the repository root",
            file=sys.stderr,
        )
        return 1
    _, repo_items_dir = safe
    resolved = track_dir.resolve()
    # Must be exactly track/items/<id> — one level deep, no nesting.
    if resolved.parent != repo_items_dir:
        print(
            f"[ERROR] Track directory must be exactly track/items/<id>: {track_dir}",
            file=sys.stderr,
        )
        return 1

    metadata_file = track_dir / "metadata.json"
    if not metadata_file.is_file():
        print(f"[ERROR] metadata.json not found in: {track_dir}", file=sys.stderr)
        return 1
    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError, UnicodeDecodeError) as exc:
        print(f"[ERROR] Cannot read metadata.json in {track_dir}: {exc}", file=sys.stderr)
        return 1
    if not isinstance(data, dict):
        print(f"[ERROR] metadata.json is not an object in: {track_dir}", file=sys.stderr)
        return 1
    try:
        from track_schema import v3_branch_field_missing, v3_branchless_track_invalid
    except ImportError:  # pragma: no cover - script execution path
        from scripts.track_schema import (
            v3_branch_field_missing,
            v3_branchless_track_invalid,
        )

    if v3_branch_field_missing(data) or v3_branchless_track_invalid(data):
        print(
            f"[ERROR] track '{track_dir.relative_to(_repo_root()).as_posix()}' is not activated yet; run /track:activate {track_dir.name}",
            file=sys.stderr,
        )
        return 1

    code, BranchGuardError, verify_track_branch, current_git_branch = (
        _ensure_branch_guard_imports()
    )
    if code:
        return code

    try:
        root = _repo_root()
        branch = current_git_branch(root)
        verify_track_branch(track_dir, current_branch=branch)
    except BranchGuardError as e:
        print(f"[ERROR] Branch guard: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"[ERROR] Branch guard check failed: {e}", file=sys.stderr)
        return 1

    return 0


def _require_explicit_track_selector_on_non_track_branch() -> int:
    """Reject non-track-branch commits unless an explicit selector is present."""
    code, _BranchGuardError, _verify_track_branch, current_git_branch = (
        _ensure_branch_guard_imports()
    )
    if code:
        return code

    branch = current_git_branch(_repo_root())
    if branch is not None and branch.startswith("track/"):
        return 0

    if branch == "HEAD":
        print(
            "[ERROR] detached HEAD requires an explicit track-id selector in tmp/track-commit/track-dir.txt",
            file=sys.stderr,
        )
        return 1

    if branch is None:
        print(
            "[ERROR] cannot determine current git branch; provide an explicit track-id selector in tmp/track-commit/track-dir.txt",
            file=sys.stderr,
        )
        return 1

    print(
        "[ERROR] non-track branch commits require an explicit track-id selector in tmp/track-commit/track-dir.txt",
        file=sys.stderr,
    )
    return 1


def _validate_planning_only_commit_paths(track_dir: Path) -> int:
    """Allow only planning artifacts for explicit branchless v3 planning-only commits."""
    metadata_file = track_dir / "metadata.json"
    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        print(f"[ERROR] Cannot read or parse metadata.json in {track_dir}: {e}", file=sys.stderr)
        return 1

    if (
        data.get("schema_version", 2) != 3
        or data.get("branch") is not None
        or data.get("status") != "planned"
    ):
        return 0

    result = run_git_capture(["diff", "--cached", "--name-only", "--diff-filter=ACMRD"])
    if result.returncode != 0:
        print("[ERROR] git diff --cached --name-only failed", file=sys.stderr)
        return 1

    repo_root = _repo_root()
    try:
        display_path = track_dir.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError:
        display_path = track_dir.as_posix()
    track_prefix = f"{display_path}/"
    allowed = {"track/registry.md", "track/tech-stack.md", ".claude/docs/DESIGN.md"}
    for raw in result.stdout.splitlines():
        path = raw.strip()
        if not path:
            continue
        if path == display_path or path.startswith(track_prefix) or path in allowed:
            continue
        print(
            f"[ERROR] planning-only commit for '{display_path}' may not stage '{path}'; run /track:activate <track-id> before committing implementation files",
            file=sys.stderr,
        )
        return 1

    return 0


def _verify_branch_by_auto_detection() -> int:
    """Fallback branch guard: auto-detect track from current branch and verify.

    Used when no explicit track_dir or track-dir.txt is provided.
    If the current branch matches track/<id>, resolve the track directory and
    run the branch guard.  If not on a track branch, skip silently (no guard).
    Returns 0 on pass/skip, 1 on guard failure.
    """
    code, BranchGuardError, verify_track_branch, current_git_branch = (
        _ensure_branch_guard_imports()
    )
    if code:
        return code

    root = _repo_root()
    branch = current_git_branch(root)
    if branch is None:
        # Cannot determine branch (not a git repo or git unavailable).
        # Fail closed: reject rather than silently skip.
        print("[ERROR] Branch guard: cannot determine current git branch", file=sys.stderr)
        return 1
    if branch == "HEAD":
        # Detached HEAD — fail closed per branch guard policy.
        print("[ERROR] Branch guard: detached HEAD — cannot verify track branch", file=sys.stderr)
        return 1
    if not branch.startswith("track/"):
        return 0  # not on a track branch — nothing to guard

    # Scan all tracks for branch ownership: reject duplicates and
    # directory-name-only matches (branch=null resolved by fallback).
    import json as _json

    safe = _safe_repo_items_dir()
    if safe is None:
        print(
            "[ERROR] track/items/ resolves outside the repository root",
            file=sys.stderr,
        )
        return 1
    _, resolved_items_dir = safe

    track_items_dir = root / "track" / "items"
    matches: list[str] = []
    if track_items_dir.is_dir():
        for candidate in sorted(track_items_dir.iterdir()):
            if not candidate.is_dir():
                continue
            # Must resolve to exactly track/items/<id> (direct child, no nesting).
            resolved_candidate = candidate.resolve()
            if resolved_candidate.parent != resolved_items_dir:
                continue
            meta = candidate / "metadata.json"
            if not meta.is_file():
                continue
            try:
                data = _json.loads(meta.read_text(encoding="utf-8"))
            except (ValueError, OSError):
                continue
            if data.get("branch") == branch:
                matches.append(candidate.name)

    if len(matches) == 0:
        # No track explicitly claims this branch in track/items/.
        # Check track/archive/ — an archived track on its own branch is
        # allowed (e.g. the archive commit itself).
        track_archive_dir = root / "track" / "archive"
        if track_archive_dir.is_dir():
            for candidate in sorted(track_archive_dir.iterdir()):
                if not candidate.is_dir():
                    continue
                meta = candidate / "metadata.json"
                if not meta.is_file():
                    continue
                try:
                    data = _json.loads(meta.read_text(encoding="utf-8"))
                except (ValueError, OSError):
                    continue
                if data.get("branch") == branch and data.get("status") == "archived":
                    return 0  # archived track on its branch — allow

        # Fallback: directory-name match with branch=null (legacy/planning).
        if branch.startswith("track/"):
            slug = branch[len("track/"):]
            null_candidate = track_items_dir / slug
            if null_candidate.is_dir():
                meta = null_candidate / "metadata.json"
                if meta.is_file():
                    try:
                        d = _json.loads(meta.read_text(encoding="utf-8"))
                    except (ValueError, OSError) as e:
                        # Fail closed: corrupt metadata cannot be trusted.
                        print(
                            f"[ERROR] Branch guard: cannot read metadata.json "
                            f"in {null_candidate}: {e}",
                            file=sys.stderr,
                        )
                        return 1
                    if (
                        d.get("branch") is None
                        and d.get("schema_version", 2) != 3
                        and d.get("status") != "archived"
                    ):
                        # Legacy branchless tracks may still resolve by directory name.
                        return 0
        print(
            f"[ERROR] Branch guard: on branch '{branch}' but no track claims "
            f"this branch in metadata.json",
            file=sys.stderr,
        )
        return 1
    if len(matches) > 1:
        print(
            f"[ERROR] Branch guard: multiple tracks claim branch '{branch}': "
            f"{', '.join(matches)}",
            file=sys.stderr,
        )
        return 1

    # Exactly one track claims this branch — resolve and verify.
    track_dir = track_items_dir / matches[0]

    try:
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
