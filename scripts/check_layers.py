#!/usr/bin/env python3
"""
Validate workspace layer dependencies from cargo metadata.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import deque
from pathlib import Path

try:
    import architecture_rules
except (
    ModuleNotFoundError
):  # pragma: no cover - used when imported as scripts.check_layers
    from scripts import architecture_rules


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def load_cargo_metadata(root: Path) -> dict:
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked"],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
    except FileNotFoundError as err:
        raise ValueError("cargo not found") from err

    if result.returncode != 0:
        detail = (
            result.stderr.strip()
            or result.stdout.strip()
            or "unknown cargo metadata error"
        )
        raise ValueError(f"cargo metadata failed: {detail}")

    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as err:
        raise ValueError(f"cargo metadata returned invalid JSON: {err}") from err


def workspace_graph(metadata: dict) -> dict[str, set[str]]:
    packages = metadata.get("packages")
    workspace_members = metadata.get("workspace_members")
    resolve = metadata.get("resolve")
    if (
        not isinstance(packages, list)
        or not isinstance(workspace_members, list)
        or not isinstance(resolve, dict)
    ):
        raise ValueError("cargo metadata is missing packages/workspace_members/resolve")

    package_by_id = {
        package["id"]: package
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("id"), str)
    }
    workspace_ids = {
        package_id for package_id in workspace_members if isinstance(package_id, str)
    }
    name_by_id = {
        package_id: package_by_id[package_id]["name"]
        for package_id in workspace_ids
        if package_id in package_by_id
        and isinstance(package_by_id[package_id].get("name"), str)
    }
    graph = {name: set() for name in name_by_id.values()}

    node_by_id = {
        node["id"]: node
        for node in resolve.get("nodes", [])
        if isinstance(node, dict) and isinstance(node.get("id"), str)
    }
    for package_id, package_name in name_by_id.items():
        node = node_by_id.get(package_id)
        if not isinstance(node, dict):
            continue
        # Use "deps" with dep_kinds when available (cargo metadata v1),
        # falling back to the flat "dependencies" list for compatibility.
        deps = node.get("deps")
        if isinstance(deps, list):
            for dep_entry in deps:
                if not isinstance(dep_entry, dict):
                    continue
                dep_pkg = dep_entry.get("pkg")
                if not isinstance(dep_pkg, str) or dep_pkg not in name_by_id:
                    continue
                dep_kinds = dep_entry.get("dep_kinds")
                if not isinstance(dep_kinds, list) or not dep_kinds:
                    # Missing or empty dep_kinds: assume normal dependency (safe default).
                    graph[package_name].add(name_by_id[dep_pkg])
                    continue
                # Include only normal dependencies (kind is null/None).
                # Skip dev-dependencies and build-dependencies.
                if any(
                    isinstance(dk, dict) and dk.get("kind") is None for dk in dep_kinds
                ):
                    graph[package_name].add(name_by_id[dep_pkg])
        else:
            # Fallback: flat dependency list (no kind info available)
            dependencies = node.get("dependencies", [])
            if isinstance(dependencies, list):
                for dependency_id in dependencies:
                    if dependency_id in name_by_id:
                        graph[package_name].add(name_by_id[dependency_id])
    return graph


def allowed_dependency_graph(rules: dict) -> dict[str, set[str]]:
    return {
        layer["crate"]: set(layer["may_depend_on"])
        for layer in architecture_rules.layer_rules(rules)
    }


def reachable_paths(graph: dict[str, set[str]], start: str) -> dict[str, list[str]]:
    found: dict[str, list[str]] = {}
    queue: deque[list[str]] = deque([[start]])
    while queue:
        path = queue.popleft()
        current = path[-1]
        for dependency in sorted(graph.get(current, set())):
            if dependency == start or dependency in found:
                continue
            next_path = path + [dependency]
            found[dependency] = next_path
            queue.append(next_path)
    return found


def direct_violations(
    crate: str,
    actual_graph: dict[str, set[str]],
    allowed_graph: dict[str, set[str]],
) -> list[str]:
    prohibited = sorted(
        set(actual_graph.get(crate, set())) - set(allowed_graph.get(crate, set()))
    )
    return [
        f"{crate}: prohibited direct dependency path {crate} -> {dependency}"
        for dependency in prohibited
    ]


def transitive_violations(
    crate: str,
    actual_graph: dict[str, set[str]],
    allowed_graph: dict[str, set[str]],
) -> list[str]:
    actual_paths = reachable_paths(actual_graph, crate)
    allowed_targets = set(reachable_paths(allowed_graph, crate))
    direct_dependencies = set(actual_graph.get(crate, set()))
    prohibited = sorted(set(actual_paths) - allowed_targets)
    errors: list[str] = []
    for dependency in prohibited:
        if dependency in direct_dependencies:
            continue
        errors.append(
            f"{crate}: prohibited transitive dependency path {' -> '.join(actual_paths[dependency])}"
        )
    return errors


def validate_dependencies(
    rules: dict, actual_graph: dict[str, set[str]], mode: str = "transitive"
) -> list[str]:
    errors: list[str] = []
    allowed_graph = allowed_dependency_graph(rules)

    missing = sorted(crate for crate in allowed_graph if crate not in actual_graph)
    for crate in missing:
        errors.append(f"{crate}: required crate not found in workspace metadata")

    for crate in sorted(allowed_graph):
        if crate not in actual_graph:
            continue
        errors.extend(direct_violations(crate, actual_graph, allowed_graph))
        if mode == "transitive":
            errors.extend(transitive_violations(crate, actual_graph, allowed_graph))
    return errors


def run(mode: str, metadata: dict | None = None, root: Path | None = None) -> int:
    repo_root = root or project_root()
    rules = (
        architecture_rules.load_rules()
        if repo_root == project_root()
        else json.loads(
            (repo_root / "docs" / "architecture-rules.json").read_text(encoding="utf-8")
        )
    )
    try:
        graph = workspace_graph(
            metadata if metadata is not None else load_cargo_metadata(repo_root)
        )
    except ValueError as err:
        print(f"[ERROR] {err}")
        return 1

    header_mode = (
        "Direct Dependencies Only" if mode == "direct" else "Transitive Dependencies"
    )
    print(f"--- Layer Dependency Check ({header_mode}) ---")

    errors = validate_dependencies(rules, graph, mode=mode)
    failed = False
    error_map: dict[str, list[str]] = {}
    for error in errors:
        crate, message = error.split(": ", 1)
        error_map.setdefault(crate, []).append(message)

    for crate in architecture_rules.crate_names(rules):
        print(f"Checking {crate}...")
        crate_errors = error_map.get(crate, [])
        if crate_errors:
            failed = True
            if mode == "direct":
                print(f"  [ERROR] {crate} has prohibited DIRECT dependencies:")
            else:
                print(
                    f"  [ERROR] {crate} has prohibited DIRECT or TRANSITIVE dependencies:"
                )
            for message in crate_errors:
                print(f"    {message}")
        else:
            print(f"  [OK] No prohibited {mode} dependencies found.")

    if failed:
        print("--- Check FAILED ---")
        return 1
    print("--- Check PASSED ---")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Validate layered workspace dependencies."
    )
    parser.add_argument(
        "--mode", choices=("direct", "transitive"), default="transitive"
    )
    parser.add_argument(
        "--metadata-file", help="Path to cargo metadata JSON for testing."
    )
    parser.add_argument("--root", help="Project root to validate.", default=None)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv[1:] if argv is not None else None)
    metadata = None
    if args.metadata_file:
        metadata = json.loads(Path(args.metadata_file).read_text(encoding="utf-8"))
    root = Path(args.root).resolve() if args.root else None
    return run(args.mode, metadata=metadata, root=root)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
