"""Atomic file write via sotp CLI, with pure-Python fallback when no compatible binary is found."""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

_SOTP_COMPATIBLE: str | None = None
_SOTP_SEARCHED: bool = False


def _probe_supports_file_write_atomic(binary: str) -> bool:
    """Return True if the binary supports 'file write-atomic'."""
    try:
        probe = subprocess.run(
            [binary, "file", "write-atomic", "--help"],
            capture_output=True,
            timeout=5,
        )
        return probe.returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def _find_sotp() -> str | None:
    """Find the first sotp binary that supports 'file write-atomic'."""
    global _SOTP_COMPATIBLE, _SOTP_SEARCHED
    if _SOTP_SEARCHED:
        return _SOTP_COMPATIBLE
    _SOTP_SEARCHED = True

    # Collect all candidates: PATH first, then workspace build paths.
    candidates: list[str] = []
    path_sotp = shutil.which("sotp")
    if path_sotp:
        candidates.append(path_sotp)
    project_root = Path(__file__).resolve().parent.parent
    for build_dir in ("target/debug/sotp", "target/release/sotp"):
        p = project_root / build_dir
        if p.is_file():
            candidates.append(str(p))

    # Probe each candidate; use the first compatible one.
    for candidate in candidates:
        if _probe_supports_file_write_atomic(candidate):
            _SOTP_COMPATIBLE = candidate
            return _SOTP_COMPATIBLE

    return None


def atomic_write_file(path: Path, content: str) -> None:
    """Write content atomically.

    Uses the first compatible sotp CLI binary found.
    Falls back to direct write when no compatible binary is available.
    If a compatible sotp is found but fails at runtime, raises OSError.
    """
    sotp = _find_sotp()
    if sotp is None:
        path.write_text(content, encoding="utf-8")
        return

    proc = subprocess.run(
        [sotp, "file", "write-atomic", "--path", str(path)],
        input=content.encode("utf-8"),
        capture_output=True,
    )
    if proc.returncode != 0:
        stderr = proc.stderr.decode("utf-8", errors="replace").strip()
        msg = f"sotp file write-atomic failed (exit {proc.returncode}): {stderr}"
        raise OSError(msg)
