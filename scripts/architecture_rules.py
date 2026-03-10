#!/usr/bin/env python3
"""
Architecture rule helpers shared by validation scripts.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def rules_path() -> Path:
    return project_root() / "docs" / "architecture-rules.json"


def cargo_toml_path() -> Path:
    return project_root() / "Cargo.toml"


def deny_toml_path() -> Path:
    return project_root() / "deny.toml"


def claude_md_path() -> Path:
    return project_root() / "CLAUDE.md"


def load_toml_text(text: str, *, source_name: str) -> dict[str, Any]:
    if tomllib is None:
        raise ValueError(
            f"{source_name} parsing requires Python 3.11+; set PYTHON_BIN to a compatible interpreter"
        )
    try:
        data = tomllib.loads(text)
    except Exception as err:
        raise ValueError(f"Invalid {source_name}: {err}") from err
    if not isinstance(data, dict):
        raise ValueError(f"{source_name} must decode to a TOML table")
    return data


def load_rules() -> dict:
    with open(rules_path(), encoding="utf-8") as handle:
        return json.load(handle)


def layer_rules(rules: dict) -> list[dict]:
    layers = rules.get("layers")
    if not isinstance(layers, list) or not layers:
        raise ValueError("architecture rules must define a non-empty 'layers' list")

    normalized: list[dict] = []
    seen_crates: set[str] = set()
    seen_paths: set[str] = set()
    for layer in layers:
        if not isinstance(layer, dict):
            raise ValueError("each layer entry must be an object")
        crate = layer.get("crate")
        path = layer.get("path")
        may_depend_on = layer.get("may_depend_on", [])
        deny_reason = layer.get("deny_reason", "")
        if not isinstance(crate, str) or not crate:
            raise ValueError("layer 'crate' must be a non-empty string")
        if not isinstance(path, str) or not path:
            raise ValueError(f"layer '{crate}' must define a non-empty 'path'")
        if not isinstance(may_depend_on, list) or any(
            not isinstance(item, str) or not item for item in may_depend_on
        ):
            raise ValueError(f"layer '{crate}' has invalid 'may_depend_on' entries")
        if not isinstance(deny_reason, str):
            raise ValueError(f"layer '{crate}' has invalid 'deny_reason'")
        if crate in seen_crates:
            raise ValueError(f"duplicate crate in architecture rules: {crate}")
        if path in seen_paths:
            raise ValueError(f"duplicate path in architecture rules: {path}")
        seen_crates.add(crate)
        seen_paths.add(path)
        normalized.append(
            {
                "crate": crate,
                "path": path,
                "may_depend_on": list(may_depend_on),
                "deny_reason": deny_reason,
            }
        )

    known_crates = {layer["crate"] for layer in normalized}
    for layer in normalized:
        unknown = [
            crate for crate in layer["may_depend_on"] if crate not in known_crates
        ]
        if unknown:
            raise ValueError(
                f"layer '{layer['crate']}' references unknown dependencies: {', '.join(sorted(unknown))}"
            )
        if layer["crate"] in layer["may_depend_on"]:
            raise ValueError(f"layer '{layer['crate']}' cannot depend on itself")

    return normalized


def workspace_members(rules: dict) -> list[str]:
    return [layer["path"] for layer in layer_rules(rules)]


def crate_names(rules: dict) -> list[str]:
    return [layer["crate"] for layer in layer_rules(rules)]


def expected_deny_rules(rules: dict) -> list[dict]:
    layers = layer_rules(rules)
    dependents = {layer["crate"]: [] for layer in layers}

    for layer in layers:
        for dependency in layer["may_depend_on"]:
            dependents[dependency].append(layer["crate"])

    derived = []
    for crate in crate_names(rules):
        wrappers = sorted(dependents[crate])
        if wrappers:
            layer = next(layer for layer in layers if layer["crate"] == crate)
            if not layer["deny_reason"].strip():
                raise ValueError(
                    f"layer '{crate}' must define a non-empty 'deny_reason' when it has dependents"
                )
            derived.append(
                {"crate": crate, "wrappers": wrappers, "reason": layer["deny_reason"]}
            )
    return derived


def direct_check_matrix(rules: dict) -> list[tuple[str, list[str]]]:
    crates = crate_names(rules)
    matrix = []
    for layer in layer_rules(rules):
        forbidden = sorted(
            crate
            for crate in crates
            if crate != layer["crate"] and crate not in layer["may_depend_on"]
        )
        matrix.append((layer["crate"], forbidden))
    return matrix


def parse_workspace_members(cargo_text: str) -> list[str]:
    cargo_data = load_toml_text(cargo_text, source_name="Cargo.toml")
    workspace = cargo_data.get("workspace")
    if workspace is None:
        return []
    if not isinstance(workspace, dict):
        raise ValueError("Cargo.toml [workspace] must be a TOML table")

    members = workspace.get("members")
    if members is None:
        return []
    if not isinstance(members, list) or any(
        not isinstance(item, str) or not item for item in members
    ):
        raise ValueError(
            "Cargo.toml workspace.members must be an array of non-empty strings"
        )
    return list(members)


def parse_deny_rules(deny_text: str) -> list[dict]:
    deny_data = load_toml_text(deny_text, source_name="deny.toml")

    deny_entries = deny_data.get("deny")
    if deny_entries is None:
        bans = deny_data.get("bans")
        if bans is None:
            return []
        if not isinstance(bans, dict):
            raise ValueError("deny.toml [bans] must be a TOML table")
        deny_entries = bans.get("deny")

    if deny_entries is None:
        return []
    if not isinstance(deny_entries, list):
        raise ValueError("deny.toml deny must be an array of inline tables")

    rules: list[dict] = []
    for entry in deny_entries:
        if not isinstance(entry, dict):
            raise ValueError("deny.toml deny entries must be inline tables")

        crate = entry.get("crate")
        wrappers = entry.get("wrappers")
        reason = entry.get("reason")
        if not isinstance(crate, str) or not crate:
            raise ValueError(
                "deny.toml deny entry is missing required string field 'crate'"
            )
        if not isinstance(wrappers, list) or any(
            not isinstance(item, str) or not item for item in wrappers
        ):
            raise ValueError(f"deny.toml deny entry '{crate}' has invalid 'wrappers'")
        if not isinstance(reason, str):
            raise ValueError(f"deny.toml deny entry '{crate}' has invalid 'reason'")
        rules.append({"crate": crate, "wrappers": sorted(wrappers), "reason": reason})
    return rules


def extract_workspace_map_block(claude_text: str) -> list[str]:
    lines = claude_text.splitlines()
    in_section = False
    in_code_block = False
    block: list[str] = []

    for line in lines:
        stripped = line.strip()
        if not in_section:
            if stripped == "## 7. Workspace Map":
                in_section = True
            continue

        if not in_code_block:
            if stripped == "```text":
                in_code_block = True
                continue
            if stripped.startswith("## "):
                break
            continue

        if stripped == "```":
            return block
        block.append(line.rstrip("\n"))

    if not in_section:
        raise ValueError("CLAUDE.md is missing the '## 7. Workspace Map' section")
    if not in_code_block:
        raise ValueError(
            "CLAUDE.md Workspace Map section is missing a ```text code block"
        )
    raise ValueError("CLAUDE.md Workspace Map code block is not closed")


WORKSPACE_TREE_ENTRY_PATTERN = re.compile(
    r"^(?P<prefix>(?:(?:│   )|(?:    ))*)(?P<branch>├── |└── )(?P<entry>.+)$"
)


def claude_workspace_map_paths(claude_text: str) -> set[str]:
    block = extract_workspace_map_block(claude_text)
    stack: list[str] = []
    paths: set[str] = set()

    for raw_line in block:
        content = raw_line.split("#", 1)[0].rstrip()
        if not content.strip():
            continue

        tree_match = WORKSPACE_TREE_ENTRY_PATTERN.match(content)
        if tree_match:
            entry = tree_match.group("entry").strip()
            depth = len(tree_match.group("prefix")) // 4 + 1
        else:
            entry = content.strip()
            depth = 0

        if not entry.endswith("/"):
            if depth == 0:
                stack = []
            continue

        directory = entry[:-1]
        if depth == 0:
            stack = [directory]
        else:
            if len(stack) < depth:
                raise ValueError(
                    f"CLAUDE.md Workspace Map has an invalid tree structure near '{raw_line.strip()}'"
                )
            stack = stack[:depth]
            stack.append(directory)

        paths.add("/".join(stack))

    return paths


def verify_claude_workspace_map(root: Path | None = None) -> list[str]:
    repo_root = root or project_root()
    claude_file = repo_root / "CLAUDE.md"
    rules_file = repo_root / "docs" / "architecture-rules.json"

    errors: list[str] = []
    try:
        claude_text = claude_file.read_text(encoding="utf-8")
    except OSError as err:
        return [f"Failed to read CLAUDE.md: {err}"]

    try:
        with open(rules_file, encoding="utf-8") as handle:
            rules = json.load(handle)
        expected_members = workspace_members(rules)
    except (OSError, json.JSONDecodeError, ValueError) as err:
        return [f"Failed to load architecture rules: {err}"]

    try:
        workspace_map_paths = claude_workspace_map_paths(claude_text)
    except ValueError as err:
        return [str(err)]

    missing = [
        member for member in expected_members if member not in workspace_map_paths
    ]
    if missing:
        errors.append(
            "CLAUDE.md Workspace Map is missing workspace members: "
            + ", ".join(missing)
        )

    return errors


def verify_sync(root: Path | None = None) -> list[str]:
    repo_root = root or project_root()
    rules_file = repo_root / "docs" / "architecture-rules.json"
    cargo_file = repo_root / "Cargo.toml"
    deny_file = repo_root / "deny.toml"

    errors: list[str] = []
    try:
        with open(rules_file, encoding="utf-8") as handle:
            rules = json.load(handle)
        layers = layer_rules(rules)
    except (OSError, json.JSONDecodeError, ValueError) as err:
        return [f"Failed to load architecture rules: {err}"]

    try:
        cargo_text = cargo_file.read_text(encoding="utf-8")
    except OSError as err:
        errors.append(f"Failed to read Cargo.toml: {err}")
        cargo_text = ""

    try:
        deny_text = deny_file.read_text(encoding="utf-8")
    except OSError as err:
        errors.append(f"Failed to read deny.toml: {err}")
        deny_text = ""

    expected_members = [layer["path"] for layer in layers]
    try:
        actual_members = parse_workspace_members(cargo_text)
    except ValueError as err:
        errors.append(f"Failed to parse Cargo.toml: {err}")
    else:
        if sorted(actual_members) != sorted(expected_members):
            errors.append(
                "Cargo.toml workspace members mismatch: "
                f"expected {expected_members}, got {actual_members}"
            )

    try:
        actual_deny = sorted(
            parse_deny_rules(deny_text), key=lambda item: item["crate"]
        )
    except ValueError as err:
        errors.append(f"Failed to parse deny.toml: {err}")
    else:
        expected_deny = sorted(
            expected_deny_rules(rules), key=lambda item: item["crate"]
        )
        if actual_deny != expected_deny:
            errors.append(
                "deny.toml layer policy mismatch: "
                f"expected {expected_deny}, got {actual_deny}"
            )

    return errors


def print_workspace_members(rules: dict) -> int:
    for member in workspace_members(rules):
        print(member)
    return 0


def print_direct_checks(rules: dict) -> int:
    for crate, forbidden in direct_check_matrix(rules):
        print(f"{crate}\t{'|'.join(forbidden)}")
    return 0


def run_verify_sync() -> int:
    errors = verify_sync()
    if errors:
        for error in errors:
            print(f"[ERROR] {error}", file=sys.stderr)
        return 1
    print("[OK] docs/architecture-rules.json matches Cargo.toml and deny.toml")
    return 0


def run_verify_claude_workspace_map() -> int:
    errors = verify_claude_workspace_map()
    if errors:
        for error in errors:
            print(f"[ERROR] {error}", file=sys.stderr)
        return 1
    print("[OK] CLAUDE.md Workspace Map covers all workspace members")
    return 0


def show_help() -> int:
    print("Usage:")
    print("  cargo make architecture-rules-workspace-members")
    print("  cargo make architecture-rules-direct-checks")
    print("  cargo make architecture-rules-verify-sync")
    print("  cargo make architecture-rules-verify-claude-workspace-map")
    return 0


def main(argv: list[str] | None = None) -> int:
    args = argv or sys.argv
    if len(args) < 2 or args[1] in {"-h", "--help", "help"}:
        return show_help()

    command = args[1]
    if command == "verify-sync":
        return run_verify_sync()
    if command == "verify-claude-workspace-map":
        return run_verify_claude_workspace_map()

    rules = load_rules()
    if command == "workspace-members":
        return print_workspace_members(rules)
    if command == "direct-checks":
        return print_direct_checks(rules)

    print(f"[ERROR] Unknown command: {command}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
