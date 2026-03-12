#!/usr/bin/env python3
"""
Wait for PR checks to pass, then merge.

Subcommands:
    wait-and-merge <pr_number> [--interval SEC] [--timeout SEC] [--method METHOD]
        Poll PR checks until all pass, then merge.
    status <pr_number>
        Show current check status without waiting.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import time


def run_gh(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["gh", *args],
        capture_output=True,
        text=True,
    )


def get_pr_checks(pr: str) -> list[dict[str, str]]:
    """Fetch PR check runs as a list of dicts with name, status, conclusion."""
    result = run_gh([
        "pr", "checks", pr, "--json", "name,state,completedAt",
    ])
    if result.returncode != 0:
        # Fallback: parse text output
        return []
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        return []


def checks_summary(checks: list[dict[str, str]]) -> tuple[bool, bool, list[str]]:
    """Return (all_passed, any_failed, pending_names).

    all_passed: every check succeeded.
    any_failed: at least one check failed (no point waiting).
    pending_names: checks still in progress.
    """
    if not checks:
        return False, False, ["(no checks found)"]

    pending: list[str] = []
    failed: list[str] = []
    for check in checks:
        state = check.get("state", "").upper()
        name = check.get("name", "unknown")
        if state == "SUCCESS":
            continue
        if state == "FAILURE":
            failed.append(name)
        else:
            pending.append(name)

    if failed:
        return False, True, failed
    if pending:
        return False, False, pending
    return True, False, []


def get_pr_url(pr: str) -> str:
    result = run_gh(["pr", "view", pr, "--json", "url", "-q", ".url"])
    return result.stdout.strip() if result.returncode == 0 else f"PR #{pr}"


def cmd_status(pr: str) -> int:
    checks = get_pr_checks(pr)
    if not checks:
        print(f"[WARN] No checks found for PR #{pr}")
        return 1

    all_passed, any_failed, names = checks_summary(checks)
    url = get_pr_url(pr)
    print(f"PR: {url}")
    if all_passed:
        print("[OK] All checks passed.")
        return 0
    if any_failed:
        print(f"[FAIL] Failed checks: {', '.join(names)}")
        return 1
    print(f"[PENDING] Waiting: {', '.join(names)}")
    return 2


def cmd_wait_and_merge(
    pr: str,
    *,
    interval: int = 15,
    timeout: int = 600,
    method: str = "merge",
) -> int:
    url = get_pr_url(pr)
    print(f"PR: {url}")
    print(f"Polling checks every {interval}s (timeout {timeout}s)...")

    start = time.monotonic()
    while True:
        elapsed = time.monotonic() - start
        checks = get_pr_checks(pr)
        all_passed, any_failed, names = checks_summary(checks)

        if all_passed:
            print("[OK] All checks passed. Merging...")
            merge_result = run_gh(["pr", "merge", pr, f"--{method}"])
            if merge_result.returncode != 0:
                print(f"[ERROR] Merge failed: {merge_result.stderr.strip()}")
                return 1
            print(f"[OK] PR #{pr} merged ({method}).")
            return 0

        if any_failed:
            print(f"[FAIL] Checks failed: {', '.join(names)}")
            print("Fix the failures and push again.")
            return 1

        if elapsed >= timeout:
            print(f"[TIMEOUT] Still pending after {timeout}s: {', '.join(names)}")
            return 1

        remaining = int(timeout - elapsed)
        print(
            f"  [{int(elapsed)}s] Pending: {', '.join(names)} "
            f"(retry in {min(interval, remaining)}s)"
        )
        time.sleep(min(interval, remaining))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Wait for PR checks and merge.",
    )
    subparsers = parser.add_subparsers(dest="command")

    status_parser = subparsers.add_parser("status", help="Show PR check status.")
    status_parser.add_argument("pr", help="PR number or branch name")

    merge_parser = subparsers.add_parser(
        "wait-and-merge", help="Poll checks, then merge on success."
    )
    merge_parser.add_argument("pr", help="PR number or branch name")
    merge_parser.add_argument(
        "--interval", type=int, default=15, help="Poll interval in seconds (default: 15)"
    )
    merge_parser.add_argument(
        "--timeout", type=int, default=600, help="Timeout in seconds (default: 600)"
    )
    merge_parser.add_argument(
        "--method",
        choices=["merge", "squash", "rebase"],
        default="merge",
        help="Merge method (default: merge)",
    )

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "status":
        return cmd_status(args.pr)
    if args.command == "wait-and-merge":
        return cmd_wait_and_merge(
            args.pr,
            interval=args.interval,
            timeout=args.timeout,
            method=args.method,
        )

    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
