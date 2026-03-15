#!/usr/bin/env python3
"""
Verify workspace architecture docs are in sync.

Replaces verify_architecture_docs.sh. Imports architecture_rules and
convention_docs directly to avoid subprocess overhead.
"""

from __future__ import annotations

import sys
from pathlib import Path

# Allow importing sibling scripts without packaging.
_SCRIPTS_DIR = Path(__file__).resolve().parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

import architecture_rules
import convention_docs


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _require_file(root: Path, rel_path: str, label: str) -> list[str]:
    if not (root / rel_path).is_file():
        return [f"[ERROR] Missing file: {rel_path} ({label})"]
    return [f"[OK] {rel_path} exists: {label}"]


def _require_line(root: Path, rel_path: str, pattern: str, label: str) -> list[str]:
    path = root / rel_path
    if not path.is_file():
        return [f"[ERROR] {rel_path} not found (checking for: {label})"]
    if pattern not in path.read_text(encoding="utf-8", errors="replace"):
        return [f"[ERROR] Missing in {rel_path}: {label}"]
    return [f"[OK] {rel_path}: {label}"]


def main(argv: list[str] | None = None) -> int:
    _ = argv
    root = project_root()
    failed = False

    print("--- Verify architecture docs sync ---")

    def emit(lines: list[str]) -> None:
        nonlocal failed
        for line in lines:
            print(line)
            if "[ERROR]" in line:
                failed = True

    emit(
        _require_file(
            root, "docs/architecture-rules.json", "architecture rules source of truth"
        )
    )
    emit(
        _require_file(
            root, "scripts/architecture_rules.py", "architecture rules helper"
        )
    )

    # Verify architecture-rules.json matches Cargo.toml and deny.toml.
    if architecture_rules.run_verify_sync() != 0:
        failed = True

    # Verify each workspace member is mentioned in Cargo.toml and tech-stack.md.
    try:
        rules = architecture_rules.load_rules()
        members = architecture_rules.workspace_members(rules)
    except Exception as err:
        print(f"[ERROR] Could not load workspace members: {err}", file=sys.stderr)
        members = []
        failed = True

    for member in members:
        emit(
            _require_line(
                root, "Cargo.toml", f'"{member}"', f"workspace member {member}"
            )
        )
        emit(
            _require_line(
                root,
                "track/tech-stack.md",
                member,
                f"tech-stack workspace map {member}",
            )
        )

    # Conventions docs alignment.
    conventions_dir = root / "project-docs" / "conventions"
    conventions_readme = conventions_dir / "README.md"
    has_convention_docs = conventions_dir.is_dir() and any(
        p for p in conventions_dir.glob("*.md") if p.name != "README.md"
    )

    if conventions_readme.is_file():
        print("[INFO] project conventions detected; validating conventions docs")
        rc = convention_docs.verify_index()
        rel_readme = conventions_readme.relative_to(root)
        if rc != 0:
            print(f"[ERROR] {rel_readme} index is out of sync")
            failed = True
        else:
            print(f"[OK] {rel_readme} index is in sync")

        emit(
            _require_line(
                root,
                "CLAUDE.md",
                "project-docs/conventions/",
                "CLAUDE project conventions reference",
            )
        )
        emit(
            _require_line(
                root,
                ".codex/instructions.md",
                "project-docs/conventions/",
                "Codex project conventions reference",
            )
        )
        emit(
            _require_line(
                root,
                "DEVELOPER_AI_WORKFLOW.md",
                "project-docs/conventions/",
                "developer workflow project conventions reference",
            )
        )
        emit(
            _require_line(
                root,
                "docs/README.md",
                "project-docs/conventions/",
                "docs README project conventions reference",
            )
        )
        emit(
            _require_file(
                root, ".claude/commands/conventions/add.md", "conventions add command"
            )
        )
    elif has_convention_docs:
        rel_dir = conventions_dir.relative_to(root)
        rel_readme = conventions_readme.relative_to(root)
        print(
            f"[ERROR] {rel_dir} contains convention documents but is missing {rel_readme}"
        )
        failed = True
    else:
        print(
            "[INFO] project conventions not bootstrapped; skipping conventions-specific checks"
        )

    # Workflow and docs alignment checks.
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make check-layers` passes",
            "workflow quality gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make verify-plan-progress` passes",
            "workflow track gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make verify-track-metadata` passes",
            "workflow metadata gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make verify-tech-stack` passes",
            "workflow tech-stack gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make scripts-selftest` passes",
            "workflow scripts selftest gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make hooks-selftest` passes",
            "workflow hooks selftest gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make verify-orchestra` passes",
            "workflow orchestra gate",
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "`cargo make verify-latest-track` passes",
            "workflow latest-track gate",
        )
    )
    emit(
        _require_line(
            root, "track/workflow.md", "/track:revert", "workflow revert command"
        )
    )
    emit(
        _require_line(
            root,
            "track/workflow.md",
            "D[Infra Layer] --> C",
            "workflow mermaid dependency direction",
        )
    )
    emit(
        _require_line(
            root,
            "TRACK_TRACEABILITY.md",
            "Responsibility Split (Fixed)",
            "traceability role section",
        )
    )
    emit(
        _require_line(
            root,
            "TRACK_TRACEABILITY.md",
            "scripts-selftest-local",
            "traceability scripts selftest gate",
        )
    )
    emit(
        _require_line(
            root,
            "TRACK_TRACEABILITY.md",
            "hooks-selftest-local",
            "traceability hooks selftest gate",
        )
    )
    emit(
        _require_line(
            root,
            "TRACK_TRACEABILITY.md",
            "verify-latest-track-local",
            "traceability latest-track gate",
        )
    )
    emit(
        _require_line(
            root,
            "TRACK_TRACEABILITY.md",
            "cargo make ci",
            "traceability ci overview",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "cargo make verify-orchestra",
            "workflow orchestra guardrail",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "cargo make verify-track-metadata",
            "workflow metadata guardrail",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "cargo make verify-tech-stack",
            "workflow tech-stack guardrail",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "cargo make verify-latest-track",
            "workflow latest-track guardrail",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "/track:revert",
            "developer workflow revert command",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "cargo make scripts-selftest",
            "developer workflow scripts selftest gate",
        )
    )
    emit(
        _require_line(
            root,
            "DEVELOPER_AI_WORKFLOW.md",
            "cargo make hooks-selftest",
            "developer workflow hooks selftest gate",
        )
    )

    if failed:
        print("--- verify_architecture_docs FAILED ---")
        return 1
    print("--- verify_architecture_docs PASSED ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
