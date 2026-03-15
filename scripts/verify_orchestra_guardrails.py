#!/usr/bin/env python3
"""
Verify Claude orchestra hooks, permissions, and agent definitions structurally.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

SETTINGS_PATH = Path(".claude/settings.json")
SETTINGS_LOCAL_PATH = Path(".claude/settings.local.json")
PERMISSION_EXTENSIONS_PATH = Path(".claude/permission-extensions.json")
BLOCK_HOOK_PATH = Path(".claude/hooks/block-direct-git-ops.py")
AGENTS_DIR = Path(".claude/agents")

REQUIRED_AGENT_FILES = {"orchestrator.md", "rust-implementation-lead.md"}

EXTRA_CARGO_MAKE_ALLOW_RE = re.compile(
    r"^Bash\(cargo make (?P<task>[A-Za-z0-9][A-Za-z0-9_-]*)(?::\*)?\)$"
)
EXTRA_GIT_ALLOW_RE = re.compile(r"^Bash\(git (?P<subcommand>[a-z][a-z-]*)(?::\*)?\)$")
ALLOWED_EXTRA_GIT_SUBCOMMANDS = {
    "cat-file",
    "describe",
    "for-each-ref",
    "merge-base",
    "name-rev",
    "rev-list",
    "show-ref",
}

EXPECTED_HOOK_PATHS = {
    ".claude/hooks/agent-router.py": "agent-router hook",
    ".claude/hooks/check-codex-before-write.py": "codex-before-write hook",
    ".claude/hooks/suggest-gemini-research.py": "gemini-research hook",
    ".claude/hooks/error-to-codex.py": "error-to-codex hook",
    ".claude/hooks/post-test-analysis.py": "post-test-analysis hook",
    ".claude/hooks/check-codex-after-plan.py": "codex-after-plan hook",
    ".claude/hooks/log-cli-tools.py": "log-cli-tools hook",
    ".claude/hooks/lint-on-save.py": "lint-on-save hook",
    ".claude/hooks/python-lint-on-save.py": "python-lint-on-save hook",
    ".claude/hooks/post-implementation-review.py": "post-implementation-review hook",
}

EXPECTED_HOOK_COMMANDS = {
    "direct git ops block hook": [
        "SOTP_CLI_BINARY:-sotp",
        "hook dispatch block-direct-git-ops",
        "|| exit 2",
    ],
    "file-lock-acquire hook": [
        "SOTP_LOCK_ENABLED:-",
        "SOTP_CLI_BINARY:-sotp",
        "hook dispatch file-lock-acquire",
        "SOTP_AGENT_ID:-pid-$PPID",
        "--pid \"$PPID\"",
        "|| exit 2",
    ],
    "file-lock-release hook": [
        "SOTP_LOCK_ENABLED:-",
        "SOTP_CLI_BINARY:-sotp",
        "hook dispatch file-lock-release",
        "SOTP_AGENT_ID:-pid-$PPID",
        "warning: file-lock-release launcher failed",
        "exit 0",
    ],
}

EXPECTED_OTHER_ALLOW = {
    "Read(./**)": "repo read permission",
    "Edit(./**)": "repo edit permission",
    "Write(./**)": "repo write permission",
    "Glob(./**)": "repo glob permission",
    "Grep(./**)": "repo grep permission",
    "Task(*)": "task permission",
    "Skill(*)": "skill permission",
    "TodoWrite(*)": "todo write permission",
    "Bash(codex:*)": "codex permission",
    "Bash(gemini:*)": "gemini permission",
    "Bash(tree:*)": "tree permission",
    "Bash(which:*)": "which permission",
    "Bash(true)": "true permission",
}

EXPECTED_GIT_ALLOW = {
    "Bash(git status:*)": "git status permission",
    "Bash(git diff:*)": "git diff permission",
    "Bash(git log:*)": "git log permission",
    "Bash(git show:*)": "git show permission",
    "Bash(git branch --list:*)": "git branch --list permission",
    "Bash(git rev-parse:*)": "git rev-parse permission",
    "Bash(git ls-files:*)": "git ls-files permission",
    "Bash(git notes show:*)": "git notes show permission",
    "Bash(git notes list:*)": "git notes list permission",
}

EXPECTED_CARGO_MAKE_ALLOW = {
    "Bash(cargo make help)": "cargo make help permission",
    "Bash(cargo make bootstrap)": "cargo make bootstrap permission",
    "Bash(cargo make build-tools)": "cargo make build-tools permission",
    "Bash(cargo make build-dev)": "cargo make build-dev permission",
    "Bash(cargo make up)": "cargo make up permission",
    "Bash(cargo make down)": "cargo make down permission",
    "Bash(cargo make logs)": "cargo make logs permission",
    "Bash(cargo make ps)": "cargo make ps permission",
    "Bash(cargo make shell)": "cargo make shell permission",
    "Bash(cargo make tools-up)": "cargo make tools-up permission",
    "Bash(cargo make tools-down)": "cargo make tools-down permission",
    "Bash(cargo make fmt-exec)": "cargo make fmt-exec permission",
    "Bash(cargo make clippy-exec)": "cargo make clippy-exec permission",
    "Bash(cargo make test-exec)": "cargo make test-exec permission",
    "Bash(cargo make test-one-exec:*)": "cargo make test-one-exec permission",
    "Bash(cargo make check-exec)": "cargo make check-exec permission",
    "Bash(cargo make machete-exec)": "cargo make machete-exec permission",
    "Bash(cargo make deny-exec)": "cargo make deny-exec permission",
    "Bash(cargo make llvm-cov-exec)": "cargo make llvm-cov-exec permission",
    "Bash(cargo make fmt)": "cargo make fmt permission",
    "Bash(cargo make fmt-check)": "cargo make fmt-check permission",
    "Bash(cargo make clippy)": "cargo make clippy permission",
    "Bash(cargo make test)": "cargo make test permission",
    "Bash(cargo make test-doc)": "cargo make test-doc permission",
    "Bash(cargo make test-nocapture)": "cargo make test-nocapture permission",
    "Bash(cargo make bacon)": "cargo make bacon permission",
    "Bash(cargo make bacon-test)": "cargo make bacon-test permission",
    "Bash(cargo make check)": "cargo make check permission",
    "Bash(cargo make deny)": "cargo make deny permission",
    "Bash(cargo make machete)": "cargo make machete permission",
    "Bash(cargo make clippy-tests)": "cargo make clippy-tests permission",
    "Bash(cargo make llvm-cov)": "cargo make llvm-cov permission",
    "Bash(cargo make check-layers)": "cargo make check-layers permission",
    "Bash(cargo make architecture-rules-verify-sync)": "architecture rules wrapper permission",
    "Bash(cargo make python-lint)": "cargo make python-lint permission",
    "Bash(cargo make ci-rust)": "cargo make ci-rust permission",
    "Bash(cargo make ci)": "cargo make ci permission",
    "Bash(cargo make verify-arch-docs)": "cargo make verify-arch-docs permission",
    "Bash(cargo make verify-plan-progress)": "cargo make verify-plan-progress permission",
    "Bash(cargo make verify-track-metadata)": "cargo make verify-track-metadata permission",
    "Bash(cargo make verify-tech-stack)": "cargo make verify-tech-stack permission",
    "Bash(cargo make verify-orchestra)": "cargo make verify-orchestra permission",
    "Bash(cargo make verify-latest-track)": "cargo make verify-latest-track permission",
    "Bash(cargo make verify-track-registry)": "cargo make verify-track-registry permission",
    "Bash(cargo make scripts-selftest)": "cargo make scripts-selftest permission",
    "Bash(cargo make hooks-selftest)": "cargo make hooks-selftest permission",
    "Bash(cargo make guides-selftest)": "cargo make guides-selftest permission",
    "Bash(cargo make add-all)": "cargo make add-all permission",
    "Bash(cargo make add-pending-paths)": "cargo make add-pending-paths permission",
    "Bash(cargo make track-add-paths)": "cargo make track-add-paths permission",
    "Bash(cargo make commit-pending-message)": "cargo make commit-pending-message permission",
    "Bash(cargo make note-pending)": "cargo make note-pending permission",
    "Bash(cargo make track-commit-message)": "cargo make track-commit-message permission",
    "Bash(cargo make track-note)": "cargo make track-note permission",
    "Bash(cargo make track-transition:*)": "cargo make track-transition permission",
    "Bash(cargo make track-sync-views:*)": "cargo make track-sync-views permission",
    "Bash(cargo make track-branch-create:*)": "cargo make track-branch-create permission",
    "Bash(cargo make track-branch-switch:*)": "cargo make track-branch-switch permission",
    "Bash(cargo make track-activate:*)": "cargo make track-activate permission",
    "Bash(cargo make track-plan-branch:*)": "cargo make track-plan-branch permission",
    "Bash(cargo make track-resolve:*)": "cargo make track-resolve permission",
    "Bash(cargo make architecture-rules-workspace-members)": "architecture rules workspace-members permission",
    "Bash(cargo make workspace-tree)": "architecture rules workspace-tree permission",
    "Bash(cargo make workspace-tree-full)": "architecture rules workspace-tree-full permission",
    "Bash(cargo make architecture-rules-direct-checks)": "architecture rules direct-checks permission",
    "Bash(cargo make conventions-add:*)": "conventions add permission",
    "Bash(cargo make conventions-update-index)": "conventions update-index permission",
    "Bash(cargo make conventions-verify-index)": "conventions verify-index permission",
    "Bash(cargo make guides-add:*)": "guides add permission",
    "Bash(cargo make guides-list)": "cargo make guides-list permission",
    "Bash(cargo make guides-fetch:*)": "guides fetch permission",
    "Bash(cargo make guides-clean:*)": "guides clean permission",
    "Bash(cargo make guides-usage)": "cargo make guides-usage permission",
    "Bash(cargo make guides-setup)": "cargo make guides-setup permission",
    "Bash(cargo make track-pr-push)": "cargo make track-pr-push permission",
    "Bash(cargo make track-pr-ensure)": "cargo make track-pr-ensure permission",
    "Bash(cargo make track-pr-review)": "cargo make track-pr-review permission",
    "Bash(cargo make track-pr-merge:*)": "cargo make track-pr-merge permission",
    "Bash(cargo make track-pr-status:*)": "cargo make track-pr-status permission",
    "Bash(cargo make track-local-review:*)": "cargo make track-local-review permission",
    "Bash(cargo make track-switch-main)": "cargo make track-switch-main permission",
}

EXPECTED_ALLOW = {
    **EXPECTED_OTHER_ALLOW,
    **EXPECTED_GIT_ALLOW,
    **EXPECTED_CARGO_MAKE_ALLOW,
}

FORBIDDEN_ALLOW = {
    "Bash(git:*)",
    "Bash(git add:*)",
    "Bash(git commit:*)",
    "Bash(git fetch:*)",
    "Bash(git remote:*)",
    "Bash(git tag:*)",
    "Bash(cat:*)",
    "Bash(ls:*)",
    "Bash(find:*)",
    "Bash(grep:*)",
    "Bash(head:*)",
    "Bash(tail:*)",
    "Bash(wc:*)",
    "Bash(sort:*)",
    "Bash(uniq:*)",
    "Bash(diff:*)",
    "Bash(echo:*)",
    "Bash(pwd:*)",
    "Bash(cd:*)",
    "Bash(mkdir:*)",
    "Bash(touch:*)",
    "Bash(cp:*)",
    "Bash(mv:*)",
    "Bash(chmod:*)",
    "Bash(cargo:*)",
    "Bash(takt:*)",
    "Bash(docker:*)",
    "Bash(docker-compose:*)",
    "Bash(rustup:*)",
    "Bash(rustfmt:*)",
    "Bash(jq:*)",
    "Bash(sed:*)",
    "Bash(awk:*)",
    "Bash(env:*)",
    "Bash(xargs:*)",
    "Bash(python3:*)",
    "Bash(pytest:*)",
    "Bash(python3 scripts/architecture_rules.py:*)",
    "Bash(python3 scripts/convention_docs.py:*)",
    "Bash(python3 scripts/external_guides.py:*)",
    "Bash(python3 scripts/takt_failure_report.py:*)",
    "Bash(cargo make add:*)",
    "Bash(cargo make commit:*)",
    "Bash(cargo make note:*)",
    "Bash(cargo make clean)",
    "Bash(cargo make architecture-rules-verify-sync:*)",
}


def cargo_make_task_name(entry: str) -> str | None:
    match = EXTRA_CARGO_MAKE_ALLOW_RE.fullmatch(entry)
    if not match:
        return None
    return match.group("task")


def git_subcommand_name(entry: str) -> str | None:
    match = EXTRA_GIT_ALLOW_RE.fullmatch(entry)
    if not match:
        return None
    return match.group("subcommand")


# Baseline allow entries and forbidden entries both reserve task names so
# extra_allow cannot re-approve them via a wider wildcard form.
KNOWN_CARGO_MAKE_TASKS = {
    task_name
    for entry in set(EXPECTED_CARGO_MAKE_ALLOW) | FORBIDDEN_ALLOW
    for task_name in [cargo_make_task_name(entry)]
    if task_name is not None
}

# Apply the same reservation rule to git subcommands so extensions cannot widen
# already-reviewed baseline/forbidden git permissions.
KNOWN_GIT_SUBCOMMANDS = {
    subcommand
    for entry in set(EXPECTED_GIT_ALLOW) | FORBIDDEN_ALLOW
    for subcommand in [git_subcommand_name(entry)]
    if subcommand is not None
}

EXPECTED_DENY = {
    "Read(./.env)": "env file read deny rule",
    "Read(./.env.*)": "env wildcard read deny rule",
    "Read(./.cache/cargo/**)": "cargo cache read deny rule",
    "Grep(./.cache/cargo/**)": "cargo cache grep deny rule",
    "Read(./.cache/home/**)": "home cache read deny rule",
    "Grep(./.cache/home/**)": "home cache grep deny rule",
    "Read(./.cache/pytest/**)": "pytest cache read deny rule",
    "Grep(./.cache/pytest/**)": "pytest cache grep deny rule",
    "Read(./.cache/sccache/**)": "sccache read deny rule",
    "Grep(./.cache/sccache/**)": "sccache grep deny rule",
    "Read(./.cache/uv/**)": "uv cache read deny rule",
    "Grep(./.cache/uv/**)": "uv cache grep deny rule",
    "Read(./**/*.db)": "db file read deny rule",
    "Grep(./**/*.db)": "db file grep deny rule",
    "Read(./**/*.sqlite)": "sqlite file read deny rule",
    "Grep(./**/*.sqlite)": "sqlite file grep deny rule",
    "Read(./**/*.pem)": "pem read deny rule",
    "Read(./**/*.key)": "key read deny rule",
    "Read(./**/credentials*)": "credentials read deny rule",
    "Read(./**/*secret*)": "secret-pattern read deny rule",
    "Read(./private/**)": "private dir read deny rule",
    "Grep(./private/**)": "private dir grep deny rule",
    "Read(./config/secrets/**)": "config secrets read deny rule",
    "Grep(./config/secrets/**)": "config secrets grep deny rule",
    "Read(~/.ssh/**)": "ssh read deny rule",
    "Read(~/.aws/**)": "aws read deny rule",
    "Bash(rm -rf /)": "rm root deny rule",
    "Bash(rm -rf ~)": "rm home deny rule",
    "Bash(cargo make --allow-private:*)": "host allow-private deny rule",
}

SUBAGENT_MODEL_ALLOWLIST = {
    "claude-sonnet-4-6",
    "claude-opus-4-6",
    "claude-haiku-4-5-20251001",
}

HARDCODED_CODEX_MODEL_RE = re.compile(r"gpt-\d+")

MODEL_RESOLUTION_TARGETS = {
    Path(".claude/skills/codex-system/SKILL.md"): "codex-system override-first resolution",
    Path(".claude/skills/track-plan/SKILL.md"): "track-plan override-first resolution",
    Path(".claude/commands/track/review.md"): "track review override-first resolution",
}

EXPECTED_MODEL_RESOLUTION_SNIPPETS = {
    Path(".claude/skills/codex-system/SKILL.md"): [
        "profiles.<active_profile>.provider_model_overrides.codex  →  {model}",
        "fallback: providers.codex.default_model  →  {model}",
    ],
    Path(".claude/skills/track-plan/SKILL.md"): [
        "Resolve `{model}` from `profiles.<active_profile>.provider_model_overrides.codex` first, then `providers.codex.default_model`",
    ],
    Path(".claude/commands/track/review.md"): [
        "Resolve `{model}` from `profiles.<active_profile>.provider_model_overrides.<provider>` first, then fall back to `providers.<provider>.default_model`.",
    ],
}

FORBIDDEN_DEFAULT_MODEL_ONLY_SNIPPETS = {
    Path(".claude/skills/codex-system/SKILL.md"): [
        "read `providers.codex.default_model` from `.claude/agent-profiles.json` and pass as `--model {model}`",
    ],
    Path(".claude/skills/track-plan/SKILL.md"): [
        'codex exec --model gpt-5.3-codex --sandbox read-only --full-auto "',
    ],
    Path(".claude/commands/track/review.md"): [
        "Read the provider's `default_model` to get `{model}`.",
    ],
}

REVIEW_WRAPPER_TARGETS = {
    Path(".claude/agent-profiles.json"): "agent profile reviewer wrapper path",
    Path(".claude/commands/track/review.md"): "track review wrapper path",
    Path(".claude/skills/codex-system/SKILL.md"): "codex-system reviewer wrapper path",
    Path(".claude/rules/02-codex-delegation.md"): "codex delegation reviewer wrapper path",
    Path(".claude/docs/research/planner-pr-review-cycle-2026-03-12.md"): "reviewer research doc wrapper path",
}

EXPECTED_REVIEW_WRAPPER_SNIPPETS = {
    Path(".claude/agent-profiles.json"): [
        'cargo make track-local-review -- --model {model} --prompt \\"{task}\\"',
    ],
    Path(".claude/commands/track/review.md"): [
        "cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md",
        "cargo make track-local-review -- --model {model} --prompt \"",
        '{"verdict":"zero_findings","findings":[]}',
        '{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}',
        "Every object field is required by the output schema.",
        "use `null` for that field instead of omitting it.",
    ],
    Path(".claude/skills/codex-system/SKILL.md"): [
        "cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md",
        "cargo make track-local-review -- --model {model} --prompt \"",
        '{"verdict":"zero_findings","findings":[]}',
        '{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}',
        "Every object field is required by the output schema.",
        "use `null` for that field instead of omitting it.",
    ],
    Path(".claude/rules/02-codex-delegation.md"): [
        "cargo make track-local-review -- --model {model} --prompt \\",
        '{"verdict":"zero_findings","findings":[]}',
        '{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}',
        "field 自体は省略せず `null` を使う。",
    ],
    Path(".claude/docs/research/planner-pr-review-cycle-2026-03-12.md"): [
        "cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md",
    ],
}

FORBIDDEN_STALE_REVIEWER_SNIPPETS = {
    Path(".claude/agent-profiles.json"): [
        "codex exec review --uncommitted --json --model {model} --full-auto",
    ],
    Path(".claude/commands/track/review.md"): [
        "timeout 180 codex exec --model {model} --sandbox read-only --full-auto",
        "timeout 600 codex exec --model {model} --sandbox read-only --full-auto",
    ],
    Path(".claude/skills/codex-system/SKILL.md"): [
        'timeout 180 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"',
        'timeout 600 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"',
        "codex exec review --uncommitted --json --model {model} --full-auto",
    ],
    Path(".claude/rules/02-codex-delegation.md"): [
        'timeout 180 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"',
        'timeout 600 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"',
        "codex exec review --uncommitted --json --model {model} --full-auto",
    ],
    Path(".claude/docs/research/planner-pr-review-cycle-2026-03-12.md"): [
        "codex exec review --uncommitted --json --model {model} --full-auto",
    ],
}

BLOCK_HOOK_MARKERS = {
    "os._exit(2)": "hard block via os._exit(2) confirmed (thin launcher fail-closed)",
    "GIT_ADD_MESSAGE": "git add block confirmed",
    "GIT_COMMIT_MESSAGE": "git commit block confirmed",
    "GIT_BRANCH_DELETE_MESSAGE": "git branch delete block confirmed",
    "GIT_PUSH_MESSAGE": "git push block confirmed",
    "GIT_SWITCH_MESSAGE": "git switch/checkout -b block confirmed",
    "GIT_MERGE_MESSAGE": "git merge block confirmed",
    "GIT_REBASE_MESSAGE": "git rebase block confirmed",
    "GIT_CHERRY_PICK_MESSAGE": "git cherry-pick block confirmed",
    "GIT_RESET_MESSAGE": "git reset block confirmed",
}

# Phrases that must appear in the TeammateIdle feedback to prevent drift.
TEAMMATE_IDLE_MARKERS = {
    "parent directory": "TeammateIdle feedback instructs creating parent directory before writing",
    "agent-teams": "TeammateIdle feedback references agent-teams log directory",
}


def load_settings(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            data = json.load(handle)
    except FileNotFoundError as err:
        raise ValueError(f"Missing settings file: {path}") from err
    except json.JSONDecodeError as err:
        raise ValueError(f"Invalid JSON in {path}: line {err.lineno}") from err
    if not isinstance(data, dict):
        raise ValueError(f"{path} must decode to a JSON object")
    return data


def load_permission_extensions(path: Path) -> set[str]:
    if not path.exists():
        return set()

    try:
        with path.open(encoding="utf-8") as handle:
            data = json.load(handle)
    except json.JSONDecodeError as err:
        raise ValueError(f"Invalid JSON in {path}: line {err.lineno}") from err

    if not isinstance(data, dict):
        raise ValueError(f"{path} must decode to a JSON object")

    entries = data.get("extra_allow", [])
    if not isinstance(entries, list) or any(
        not isinstance(item, str) for item in entries
    ):
        raise ValueError(f"{path} field 'extra_allow' must be an array of strings")
    return set(entries)


def validate_permission_extensions(extra_allow: set[str], allow: set[str]) -> bool:
    failed = False
    for entry in sorted(extra_allow):
        if entry not in allow:
            emit_error(
                f"{PERMISSION_EXTENSIONS_PATH} contains latent extra_allow entry not present in "
                f"{SETTINGS_PATH} permissions.allow: {entry}"
            )
            failed = True
            continue

        if entry in EXPECTED_ALLOW:
            emit_error(
                f"{PERMISSION_EXTENSIONS_PATH} contains baseline allow entry: {entry} - "
                "extra_allow is only for project-specific additions"
            )
            failed = True
            continue

        if entry in FORBIDDEN_ALLOW:
            emit_error(
                f"{PERMISSION_EXTENSIONS_PATH} contains forbidden extra_allow entry: {entry} - "
                "direct access would be silently allowed"
            )
            failed = True
            continue

        if entry.startswith("Bash(") and "scripts/" in entry:
            emit_error(
                f"{PERMISSION_EXTENSIONS_PATH} contains {entry} - "
                "direct repo scripts must be routed through cargo make wrappers"
            )
            failed = True
            continue

        task_name = cargo_make_task_name(entry)
        if task_name is not None:
            if task_name in KNOWN_CARGO_MAKE_TASKS:
                emit_error(
                    f"{PERMISSION_EXTENSIONS_PATH} contains extension for guarded cargo make task: {entry} - "
                    "baseline or approval-gated cargo make task names cannot be widened via extra_allow"
                )
                failed = True
                continue
            emit_ok(
                f"{PERMISSION_EXTENSIONS_PATH}: allowed project cargo make extension: {entry}"
            )
            continue

        subcommand = git_subcommand_name(entry)
        if subcommand is not None and subcommand in ALLOWED_EXTRA_GIT_SUBCOMMANDS:
            if subcommand in KNOWN_GIT_SUBCOMMANDS:
                emit_error(
                    f"{PERMISSION_EXTENSIONS_PATH} contains extension for guarded git subcommand: {entry} - "
                    "baseline git permissions cannot be widened via extra_allow"
                )
                failed = True
                continue
            emit_ok(
                f"{PERMISSION_EXTENSIONS_PATH}: allowed read-only git extension: {entry}"
            )
            continue

        emit_error(
            f"{PERMISSION_EXTENSIONS_PATH} contains unsupported extra_allow entry: {entry} - "
            "only project-specific Bash(cargo make <task>) / Bash(cargo make <task>:*) and "
            "whitelisted read-only Bash(git <subcommand>) / Bash(git <subcommand>:*) are allowed"
        )
        failed = True
    return failed


def hook_commands(settings: dict[str, Any]) -> list[str]:
    hooks = settings.get("hooks")
    if not isinstance(hooks, dict):
        raise ValueError(".claude/settings.json missing object field 'hooks'")

    commands: list[str] = []
    for event_bindings in hooks.values():
        if not isinstance(event_bindings, list):
            raise ValueError("Each hooks event binding list must be an array")
        for binding in event_bindings:
            if not isinstance(binding, dict):
                raise ValueError("Each hooks event binding must be an object")
            nested_hooks = binding.get("hooks", [])
            if not isinstance(nested_hooks, list):
                raise ValueError("Each hooks binding must contain a hooks array")
            for hook in nested_hooks:
                if not isinstance(hook, dict):
                    raise ValueError("Each hook entry must be an object")
                command = hook.get("command")
                if isinstance(command, str):
                    commands.append(command)
    return commands


def permission_set(settings: dict[str, Any], key: str) -> set[str]:
    permissions = settings.get("permissions")
    if not isinstance(permissions, dict):
        raise ValueError(".claude/settings.json missing object field 'permissions'")

    values = permissions.get(key)
    if not isinstance(values, list):
        raise ValueError(f".claude/settings.json permissions.{key} must be an array")
    if any(not isinstance(item, str) for item in values):
        raise ValueError(
            f".claude/settings.json permissions.{key} entries must be strings"
        )
    return set(values)


def emit_ok(message: str) -> None:
    print(f"[OK] {message}")


def emit_error(message: str) -> None:
    print(f"[ERROR] {message}")


def verify_hook_paths(commands: list[str]) -> bool:
    failed = False
    for hook_path, label in EXPECTED_HOOK_PATHS.items():
        if any(hook_path in command for command in commands):
            emit_ok(f"{SETTINGS_PATH}: {label}")
        else:
            emit_error(f"Missing in {SETTINGS_PATH}: {label}")
            failed = True

        if Path(hook_path).is_file():
            emit_ok(f"{hook_path}: file exists")
        else:
            emit_error(f"Missing hook file: {hook_path}")
            failed = True

    for label, fragments in EXPECTED_HOOK_COMMANDS.items():
        if any(all(fragment in command for fragment in fragments) for command in commands):
            emit_ok(f"{SETTINGS_PATH}: {label}")
        else:
            emit_error(
                f"Missing in {SETTINGS_PATH}: {label} "
                f"(expected fragments: {', '.join(fragments)})"
            )
            failed = True
    return failed


def verify_allowlist(allow: set[str], extra_allow: set[str]) -> bool:
    failed = False
    for entry, label in EXPECTED_ALLOW.items():
        if entry in allow:
            emit_ok(f"{SETTINGS_PATH}: {label}")
        else:
            emit_error(f"Missing in {SETTINGS_PATH}: {label}")
            failed = True

    for entry in sorted(FORBIDDEN_ALLOW):
        if entry in allow:
            emit_error(
                f"{SETTINGS_PATH} contains {entry} - direct access would be silently allowed"
            )
            failed = True
        else:
            emit_ok(f"{SETTINGS_PATH}: {entry} absent")

    for entry in sorted(allow):
        if entry in FORBIDDEN_ALLOW:
            continue

        if entry.startswith("Bash(") and "scripts/" in entry:
            emit_error(
                f"{SETTINGS_PATH} contains {entry} - direct repo scripts must be routed through cargo make wrappers"
            )
            failed = True
            continue

        if entry not in EXPECTED_ALLOW and entry not in extra_allow:
            emit_error(
                f"{SETTINGS_PATH} contains unexpected allow entry: {entry} - "
                f"add it to {PERMISSION_EXTENSIONS_PATH} if this project intentionally extends the baseline"
            )
            failed = True
    return failed


def verify_denylist(deny: set[str]) -> bool:
    failed = False
    for entry, label in EXPECTED_DENY.items():
        if entry in deny:
            emit_ok(f"{SETTINGS_PATH}: {label}")
        else:
            emit_error(f"Missing in {SETTINGS_PATH}: {label}")
            failed = True
    return failed


def verify_env(settings: dict[str, Any]) -> bool:
    env = settings.get("env")
    if not isinstance(env, dict):
        emit_error(f"{SETTINGS_PATH} is missing env configuration")
        return True
    failed = False

    if env.get("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS") == "1":
        emit_ok(f"{SETTINGS_PATH}: agent teams enabled")
    else:
        emit_error(f"Missing in {SETTINGS_PATH}: agent teams enabled")
        failed = True

    model = env.get("CLAUDE_CODE_SUBAGENT_MODEL")
    if model in SUBAGENT_MODEL_ALLOWLIST:
        emit_ok(f"{SETTINGS_PATH}: CLAUDE_CODE_SUBAGENT_MODEL allowlisted ({model})")
    else:
        emit_error(
            f"{SETTINGS_PATH}: CLAUDE_CODE_SUBAGENT_MODEL must be one of "
            f"{sorted(SUBAGENT_MODEL_ALLOWLIST)}, got {model!r}"
        )
        failed = True

    return failed


def verify_block_hook() -> bool:
    if not BLOCK_HOOK_PATH.is_file():
        emit_error(f"Missing hard-block hook: {BLOCK_HOOK_PATH}")
        return True

    content = BLOCK_HOOK_PATH.read_text(encoding="utf-8")
    failed = False
    for marker, label in BLOCK_HOOK_MARKERS.items():
        if marker in content:
            emit_ok(f"{BLOCK_HOOK_PATH}: {label}")
        else:
            emit_error(f"{BLOCK_HOOK_PATH} does not include marker: {marker}")
            failed = True
    return failed


def verify_teammate_idle_feedback(settings: dict[str, Any]) -> bool:
    hooks = settings.get("hooks", {})
    idle_bindings = hooks.get("TeammateIdle", [])
    feedback_text = ""
    for binding in idle_bindings:
        for hook in binding.get("hooks", []):
            command = hook.get("command", "")
            if isinstance(command, str):
                feedback_text += command

    failed = False
    for marker, label in TEAMMATE_IDLE_MARKERS.items():
        if marker in feedback_text:
            emit_ok(f"{SETTINGS_PATH}: {label}")
        else:
            emit_error(f"Missing in {SETTINGS_PATH} TeammateIdle feedback: {label!r}")
            failed = True
    return failed


def verify_agent_definitions() -> bool:
    failed = False
    for required in sorted(REQUIRED_AGENT_FILES):
        if (AGENTS_DIR / required).is_file():
            emit_ok(f"{AGENTS_DIR}/{required}: agent definition exists")
        else:
            emit_error(f"Missing required agent definition: {AGENTS_DIR}/{required}")
            failed = True
    return failed


def verify_no_hardcoded_codex_model_literals() -> bool:
    failed = False
    for root in (Path(".claude/skills"), Path(".claude/commands")):
        if not root.is_dir():
            continue
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            if "__pycache__" in path.parts:
                continue
            content = path.read_text(encoding="utf-8")
            if HARDCODED_CODEX_MODEL_RE.search(content):
                emit_error(
                    f"{path} contains hardcoded Codex model literal matching "
                    f"{HARDCODED_CODEX_MODEL_RE.pattern}"
                )
                failed = True
    if not failed:
        emit_ok(
            f".claude/skills/ and .claude/commands/: no hardcoded Codex model literals "
            f"matching {HARDCODED_CODEX_MODEL_RE.pattern}"
        )
    return failed


def verify_override_first_model_resolution() -> bool:
    failed = False
    for path, label in MODEL_RESOLUTION_TARGETS.items():
        if not path.is_file():
            emit_error(f"Missing model resolution target: {path}")
            failed = True
            continue
        content = path.read_text(encoding="utf-8")
        missing_snippets = [
            snippet
            for snippet in EXPECTED_MODEL_RESOLUTION_SNIPPETS.get(path, [])
            if snippet not in content
        ]
        if not missing_snippets:
            emit_ok(f"{path}: {label}")
        else:
            emit_error(
                f"{path} is missing canonical override-first guidance for {label}: "
                + "; ".join(missing_snippets)
            )
            failed = True
        for forbidden in FORBIDDEN_DEFAULT_MODEL_ONLY_SNIPPETS.get(path, []):
            if forbidden in content:
                emit_error(
                    f"{path} still contains stale default_model-only guidance: {forbidden}"
                )
                failed = True
    return failed


def verify_reviewer_wrapper_guidance() -> bool:
    failed = False
    for path, label in REVIEW_WRAPPER_TARGETS.items():
        if not path.is_file():
            emit_error(f"Missing reviewer wrapper target: {path}")
            failed = True
            continue
        content = path.read_text(encoding="utf-8")
        missing_snippets = [
            snippet
            for snippet in EXPECTED_REVIEW_WRAPPER_SNIPPETS.get(path, [])
            if snippet not in content
        ]
        if not missing_snippets:
            emit_ok(f"{path}: {label}")
        else:
            emit_error(
                f"{path} is missing reviewer wrapper guidance for {label}: "
                + "; ".join(missing_snippets)
            )
            failed = True
        for forbidden in FORBIDDEN_STALE_REVIEWER_SNIPPETS.get(path, []):
            if forbidden in content:
                emit_error(
                    f"{path} still contains stale reviewer command guidance: {forbidden}"
                )
                failed = True
    return failed


def verify_no_local_settings_committed() -> bool:
    """Fail if settings.local.json is tracked by git (should be gitignored)."""
    import subprocess

    repo_root = Path(__file__).resolve().parent.parent
    result = subprocess.run(
        ["git", "ls-files", "--error-unmatch", str(SETTINGS_LOCAL_PATH)],
        capture_output=True,
        check=False,
        cwd=repo_root,
    )
    if result.returncode == 0:
        emit_error(
            f"{SETTINGS_LOCAL_PATH} is tracked by git. "
            "Local overrides must not be committed — add to .gitignore and run: "
            "git rm --cached .claude/settings.local.json"
        )
        return True

    stderr_text = result.stderr.decode(errors="replace").strip()

    if result.returncode == 128 and "not a git repository" in stderr_text.lower():
        emit_ok(f"{SETTINGS_LOCAL_PATH}: no git repo (cannot be tracked)")
        return False

    if result.returncode != 1:
        emit_error(f"git ls-files failed (exit {result.returncode}): {stderr_text}")
        return True

    if SETTINGS_LOCAL_PATH.is_file():
        emit_ok(f"{SETTINGS_LOCAL_PATH}: exists but not tracked (local override)")
    else:
        emit_ok(f"{SETTINGS_LOCAL_PATH}: absent or gitignored (expected)")
    return False


def main() -> int:
    print("--- Verify orchestra guardrails ---")
    try:
        settings = load_settings(SETTINGS_PATH)
        extra_allow = load_permission_extensions(PERMISSION_EXTENSIONS_PATH)
        commands = hook_commands(settings)
        allow = permission_set(settings, "allow")
        deny = permission_set(settings, "deny")
    except ValueError as err:
        emit_error(str(err))
        print("--- verify_orchestra_guardrails FAILED ---")
        return 1

    failed = False
    failed = verify_hook_paths(commands) or failed
    failed = validate_permission_extensions(extra_allow, allow) or failed
    failed = verify_allowlist(allow, extra_allow) or failed
    failed = verify_denylist(deny) or failed
    failed = verify_env(settings) or failed
    failed = verify_block_hook() or failed
    failed = verify_teammate_idle_feedback(settings) or failed
    failed = verify_agent_definitions() or failed
    failed = verify_no_hardcoded_codex_model_literals() or failed
    failed = verify_override_first_model_resolution() or failed
    failed = verify_reviewer_wrapper_guidance() or failed
    failed = verify_no_local_settings_committed() or failed

    if failed:
        print("--- verify_orchestra_guardrails FAILED ---")
        return 1

    print("--- verify_orchestra_guardrails PASSED ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
