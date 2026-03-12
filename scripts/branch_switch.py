#!/usr/bin/env python3
"""
Switch to a branch and pull latest changes.

Subcommands:
    switch-and-pull <branch>
        Checkout the branch and pull from origin.
"""
from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


def _repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def run_git(args: list[str]) -> int:
    result = subprocess.run(
        ["git", *args],
        cwd=str(_repo_root()),
    )
    return result.returncode


def cmd_switch_and_pull(branch: str) -> int:
    print(f"Switching to {branch}...")
    code = run_git(["checkout", branch])
    if code != 0:
        print(f"[ERROR] Failed to checkout {branch}")
        return code

    print(f"Pulling latest from origin/{branch}...")
    code = run_git(["pull", "--ff-only"])
    if code != 0:
        print("[WARN] Pull failed (may not have remote tracking branch)")
        # Not fatal — local checkout succeeded
        return 0

    print(f"[OK] On {branch}, up to date.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Switch branch and pull.")
    subparsers = parser.add_subparsers(dest="command")

    switch_parser = subparsers.add_parser(
        "switch-and-pull", help="Checkout branch and pull."
    )
    switch_parser.add_argument("branch", help="Branch name to switch to")

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "switch-and-pull":
        return cmd_switch_and_pull(args.branch)

    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
