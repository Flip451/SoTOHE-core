#!/usr/bin/env python3
"""pr_review.py — PR-based review cycle orchestration via Codex Cloud @codex review.

Subcommands:
    push           Push current track branch to origin.
    ensure-pr      Create or reuse a PR for the current track branch.
    trigger-review Post '@codex review' comment on the PR.
    poll-review    Poll GitHub API until Codex Cloud review appears.
    run            Full cycle: push → ensure-pr → trigger → poll → parse.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
DEFAULT_POLL_INTERVAL = 15  # seconds
DEFAULT_POLL_TIMEOUT = 600  # 10 minutes
CODEX_BOT_LOGIN_PATTERN = re.compile(r"codex", re.IGNORECASE)
STRUCTURED_PROVIDERS = {"codex"}

# Paths that must be stripped from review output
_ABS_PATH_RE = re.compile(
    r"(/(?:home|Users|tmp|var|etc|opt|srv|workspace|root|usr/local)/[^\s]+)"
)
_ENV_INFO_RE = re.compile(
    r"((?:https?://)?(?:localhost|127\.0\.0\.1|0\.0\.0\.0)(?::\d+)?(?:/[^\s]*)?)"
)
_SECRET_PATTERN_RE = re.compile(
    r"(?:sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{36,}|github_pat_[a-zA-Z0-9_]{20,}|"
    r"glpat-[a-zA-Z0-9\-]{20,}|"
    r"AKIA[A-Z0-9]{16}|xox[bprs]-[a-zA-Z0-9\-]+)"
)
_RFC1918_RE = re.compile(
    r"(?<!\d)"
    r"(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|"
    r"172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}|"
    r"192\.168\.\d{1,3}\.\d{1,3})"
    r"(?::\d+)?"
    r"(?!\d)"
)

# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass
class ReviewFinding:
    """A normalized finding from a Codex Cloud review."""

    severity: str  # P0, P1, LOW, INFO
    path: str
    line: int | None = None
    end_line: int | None = None
    body: str = ""
    rule_id: str | None = None


@dataclass
class ReviewResult:
    """Parsed result of a Codex Cloud review."""

    review_id: int
    state: str  # APPROVED, CHANGES_REQUESTED, COMMENTED
    body: str
    findings: list[ReviewFinding] = field(default_factory=list)
    inline_comment_count: int = 0
    actionable_count: int = 0
    passed: bool = False


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _run_gh(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    """Run gh CLI and return result."""
    result = subprocess.run(
        ["gh", *args],
        capture_output=True,
        text=True,
    )
    if check and result.returncode != 0:
        print(f"[ERROR] gh {' '.join(args)}: {sanitize_text(result.stderr.strip())}", file=sys.stderr)
        raise SystemExit(1)
    return result


def _run_git(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    """Run git CLI and return result."""
    result = subprocess.run(
        ["git", *args],
        capture_output=True,
        text=True,
    )
    if check and result.returncode != 0:
        print(f"[ERROR] git {' '.join(args)}: {sanitize_text(result.stderr.strip())}", file=sys.stderr)
        raise SystemExit(1)
    return result


def _parse_paginated_json(text: str) -> list[dict]:
    """Parse potentially paginated gh api JSON output into a flat list.

    gh api --paginate for REST endpoints automatically merges JSON arrays,
    but in edge cases it may concatenate multiple JSON arrays. This handles
    both single-array and concatenated-array output robustly.
    """
    text = text.strip()
    if not text:
        return []
    try:
        data = json.loads(text)
        if isinstance(data, list):
            return data
        if isinstance(data, dict):
            return [data]
        return []
    except json.JSONDecodeError:
        # Concatenated JSON arrays: try to parse each one
        results: list[dict] = []
        decoder = json.JSONDecoder()
        pos = 0
        while pos < len(text):
            if text[pos] in " \t\n\r":
                pos += 1
                continue
            try:
                obj, end = decoder.raw_decode(text, pos)
                if isinstance(obj, list):
                    results.extend(obj)
                elif isinstance(obj, dict):
                    results.append(obj)
                pos = end
            except json.JSONDecodeError:
                break
        return results


def sanitize_text(text: str) -> str:
    """Remove absolute paths, secrets, and internal env info from text."""
    text = _SECRET_PATTERN_RE.sub("[REDACTED]", text)
    text = _ABS_PATH_RE.sub("[PATH]", text)
    text = _ENV_INFO_RE.sub("[INTERNAL]", text)
    text = _RFC1918_RE.sub("[INTERNAL_IP]", text)
    return text


def _resolve_track_context() -> tuple[str, str, Path]:
    """Resolve current track branch, track ID, and track directory.

    Returns (branch_name, track_id, track_dir).
    """
    result = _run_git(["rev-parse", "--abbrev-ref", "HEAD"])
    branch = result.stdout.strip()
    if not branch.startswith("track/"):
        print(
            "[ERROR] Not on a track branch (expected track/<id>). "
            "For planning-only tracks, run /track:activate <track-id> first.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    track_id = branch[len("track/"):]
    track_dir = Path("track/items") / track_id
    if not track_dir.is_dir():
        print(f"[ERROR] Track directory not found: {track_dir}", file=sys.stderr)
        raise SystemExit(1)
    return branch, track_id, track_dir


def _resolve_reviewer_provider() -> str:
    """Resolve the reviewer provider from agent-profiles.json.

    Returns the provider name. Raises SystemExit if non-structured.
    """
    profiles_path = Path(".claude/agent-profiles.json")
    if not profiles_path.is_file():
        print("[ERROR] .claude/agent-profiles.json not found", file=sys.stderr)
        raise SystemExit(1)
    data = json.loads(profiles_path.read_text(encoding="utf-8"))
    active_profile = data.get("active_profile", "default")
    profiles = data.get("profiles", {})
    profile = profiles.get(active_profile, {})
    provider = profile.get("reviewer", "codex")
    if provider not in STRUCTURED_PROVIDERS:
        print(
            f"[ERROR] Reviewer provider '{provider}' does not support structured output. "
            f"/track:pr-review requires a structured provider ({', '.join(sorted(STRUCTURED_PROVIDERS))}). "
            f"Use /track:review for local review with non-structured providers.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    return provider


def _get_repo_nwo() -> str:
    """Get the repository owner/name from gh."""
    result = _run_gh(["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"])
    return result.stdout.strip()


# ---------------------------------------------------------------------------
# Subcommands
# ---------------------------------------------------------------------------


def cmd_push() -> None:
    """Push the current track branch to origin."""
    branch, _track_id, _track_dir = _resolve_track_context()
    print(f"Pushing {branch} to origin...")
    _run_git(["push", "-u", "origin", branch])
    print(f"[OK] Pushed {branch}")


def cmd_ensure_pr() -> str:
    """Create or reuse a PR for the current track branch.

    Returns the PR number as a string.
    """
    branch, track_id, _track_dir = _resolve_track_context()

    # Check for existing PR
    result = _run_gh([
        "pr", "list",
        "--head", branch,
        "--base", "main",
        "--state", "open",
        "--json", "number",
        "-q", ".[0].number",
    ])
    pr_number = result.stdout.strip()
    if pr_number:
        print(f"[OK] Reusing existing PR #{pr_number}")
        return pr_number

    # Create new PR
    result = _run_gh([
        "pr", "create",
        "--head", branch,
        "--base", "main",
        "--title", f"track: {track_id}",
        "--body", f"Track implementation for `{track_id}`.\n\nCreated by `/track:pr-review`.",
    ])
    # Extract PR number from URL
    url = result.stdout.strip()
    pr_match = re.search(r"/pull/(\d+)", url)
    if pr_match:
        pr_number = pr_match.group(1)
    else:
        # Try to get it from pr view
        view_result = _run_gh(["pr", "view", branch, "--json", "number", "-q", ".number"])
        pr_number = view_result.stdout.strip()
    print(f"[OK] Created PR #{pr_number}: {url}")
    return pr_number


def cmd_trigger_review(pr_number: str) -> datetime:
    """Post '@codex review' comment on the PR.

    Returns the trigger timestamp (UTC).
    """
    _resolve_reviewer_provider()  # fail-closed check

    result = _run_gh([
        "api",
        f"repos/{{owner}}/{{repo}}/issues/{pr_number}/comments",
        "-f", "body=@codex review",
    ])
    # Use GitHub's server-issued timestamp to avoid local clock skew.
    # The API response includes the created_at of the posted comment.
    comment_data = json.loads(result.stdout) if result.stdout.strip() else {}
    created_at_raw = comment_data.get("created_at", "")
    if created_at_raw:
        trigger_time = datetime.fromisoformat(created_at_raw.replace("Z", "+00:00"))
    else:
        # Fallback to local clock if API response lacks created_at
        trigger_time = datetime.now(UTC).replace(microsecond=0)
    print(f"[OK] Posted '@codex review' on PR #{pr_number} at {trigger_time.isoformat()}")
    return trigger_time


def cmd_poll_review(
    pr_number: str,
    trigger_time: datetime,
    *,
    poll_interval: int = DEFAULT_POLL_INTERVAL,
    poll_timeout: int = DEFAULT_POLL_TIMEOUT,
) -> dict | None:
    """Poll GitHub API for a Codex Cloud review created after trigger_time.

    Returns the review dict if found, or None on timeout.
    Distinguishes GitHub App not installed from generic timeout.
    """
    deadline = time.monotonic() + poll_timeout
    any_bot_activity = False

    print(f"Polling for Codex review on PR #{pr_number} (interval={poll_interval}s, timeout={poll_timeout}s)...")

    while time.monotonic() < deadline:
        # Fetch reviews
        result = _run_gh([
            "api",
            f"repos/{{owner}}/{{repo}}/pulls/{pr_number}/reviews",
            "--paginate",
        ], check=False)

        if result.returncode == 0 and result.stdout.strip():
            reviews = _parse_paginated_json(result.stdout)
            for review in reviews:
                author = review.get("user", {}).get("login", "")
                if CODEX_BOT_LOGIN_PATTERN.search(author):
                    submitted_at_raw = review.get("submitted_at", "")
                    if submitted_at_raw:
                        submitted_at = datetime.fromisoformat(
                            submitted_at_raw.replace("Z", "+00:00")
                        )
                        # trigger_time is captured AFTER posting the comment, so
                        # any pre-existing review has submitted_at < trigger_time.
                        # Use >= to accept reviews created in the same second.
                        if submitted_at >= trigger_time:
                            any_bot_activity = True
                            state = review.get("state", "")
                            if state in (
                                "APPROVED",
                                "CHANGES_REQUESTED",
                                "COMMENTED",
                            ):
                                print(f"[OK] Found Codex review (id={review['id']}, state={state})")
                                return review

        # Also check comments for post-trigger bot activity
        if not any_bot_activity:
            comments_result = _run_gh([
                "api",
                f"repos/{{owner}}/{{repo}}/issues/{pr_number}/comments",
                "--paginate",
            ], check=False)
            if comments_result.returncode == 0 and comments_result.stdout.strip():
                comments = _parse_paginated_json(comments_result.stdout)
                for comment in comments:
                    author = comment.get("user", {}).get("login", "")
                    if CODEX_BOT_LOGIN_PATTERN.search(author):
                        created_raw = comment.get("created_at", "")
                        if created_raw:
                            created_at = datetime.fromisoformat(
                                created_raw.replace("Z", "+00:00")
                            )
                            if created_at >= trigger_time:
                                any_bot_activity = True
                                break

        remaining = int(deadline - time.monotonic())
        print(f"  Waiting... ({remaining}s remaining)")
        time.sleep(poll_interval)

    # Timeout — distinguish cause
    if not any_bot_activity:
        print(
            "[ERROR] Timeout: No Codex bot activity detected on this PR. "
            "Ensure the Codex Cloud GitHub App is installed on this repository. "
            "See: https://github.com/apps/openai-codex",
            file=sys.stderr,
        )
    else:
        print(
            "[ERROR] Timeout: Codex bot is active but review not yet completed. "
            "The review may still be in progress. Try again later or increase timeout.",
            file=sys.stderr,
        )
    return None


def cmd_parse_review(pr_number: str, review: dict) -> ReviewResult:
    """Parse a Codex Cloud review into normalized findings.

    Fetches inline comments via gh api.
    """
    review_id = review["id"]
    state = review.get("state", "COMMENTED")
    body = sanitize_text(review.get("body", "") or "")

    # Fetch inline comments for this review
    result = _run_gh([
        "api",
        f"repos/{{owner}}/{{repo}}/pulls/{pr_number}/reviews/{review_id}/comments",
        "--paginate",
    ], check=False)

    findings: list[ReviewFinding] = []
    inline_count = 0

    if result.returncode == 0 and result.stdout.strip():
        comments = _parse_paginated_json(result.stdout)
        for comment in comments:
            inline_count += 1
            comment_body = sanitize_text(comment.get("body", ""))
            path = comment.get("path", "")
            # GitHub API: start_line = first line, line = last line
            start = comment.get("start_line") or comment.get("original_start_line")
            end = comment.get("line") or comment.get("original_line")
            # For single-line comments, start_line is null; use line as both
            line = start if start else end
            end_line = end

            # Classify severity from comment content
            severity = _classify_severity(comment_body)
            findings.append(ReviewFinding(
                severity=severity,
                path=path,
                line=line,
                end_line=end_line,
                body=comment_body,
            ))

    # Also parse findings from review body
    if body:
        body_findings = _parse_body_findings(body)
        findings.extend(body_findings)

    # Count actionable findings (P0 and P1)
    actionable = sum(1 for f in findings if f.severity in ("P0", "P1"))
    # CHANGES_REQUESTED always fails, even with zero parseable actionable findings
    passed = actionable == 0 and state != "CHANGES_REQUESTED"

    return ReviewResult(
        review_id=review_id,
        state=state,
        body=body,
        findings=findings,
        inline_comment_count=inline_count,
        actionable_count=actionable,
        passed=passed,
    )


def _classify_severity(text: str) -> str:
    """Classify a finding's severity from its text content."""
    lower = text.lower()
    if any(kw in lower for kw in ("critical", "security", "vulnerability", "panic", "data loss")):
        return "P0"
    if any(kw in lower for kw in ("error", "bug", "incorrect", "wrong", "missing error")):
        return "P0"
    if any(kw in lower for kw in ("warning", "should", "consider", "suggest", "improve")):
        return "P1"
    # Default to P1 for inline comments (they're likely actionable)
    return "P1"


def _parse_body_findings(body: str) -> list[ReviewFinding]:
    """Extract findings from review body text."""
    findings: list[ReviewFinding] = []
    # Look for bullet-pointed items in the body
    for line in body.split("\n"):
        stripped = line.strip()
        if stripped.startswith(("- ", "* ", "• ")):
            content = stripped[2:].strip()
            if len(content) > 10:  # Skip very short items
                severity = _classify_severity(content)
                findings.append(ReviewFinding(
                    severity=severity,
                    path="",
                    body=content,
                ))
    return findings


def cmd_run() -> None:
    """Full PR review cycle: push → ensure-pr → trigger → poll → parse."""
    _resolve_reviewer_provider()  # fail-closed check early
    _branch, _track_id, _track_dir = _resolve_track_context()

    # Step 1: Push
    cmd_push()

    # Step 2: Ensure PR
    pr_number = cmd_ensure_pr()

    # Step 3: Trigger review
    trigger_time = cmd_trigger_review(pr_number)

    # Step 4: Poll for review
    review = cmd_poll_review(pr_number, trigger_time)
    if review is None:
        raise SystemExit(1)

    # Step 5: Parse results
    parsed = cmd_parse_review(pr_number, review)

    # Step 6: Report
    _print_summary(pr_number, parsed)

    if not parsed.passed:
        raise SystemExit(1)


def _print_summary(pr_number: str, result: ReviewResult) -> None:
    """Print a human-readable summary of the review result."""
    status = "PASS" if result.passed else "FAIL"
    print()
    print(f"=== PR Review Result: {status} ===")
    print(f"PR: #{pr_number}")
    print(f"Review ID: {result.review_id}")
    print(f"State: {result.state}")
    print(f"Inline comments: {result.inline_comment_count}")
    print(f"Total findings: {len(result.findings)}")
    print(f"Actionable (P0/P1): {result.actionable_count}")

    if result.findings:
        print()
        print("Findings:")
        for i, f in enumerate(result.findings, 1):
            location = f"{f.path}:{f.line}" if f.path and f.line else (f.path or "general")
            print(f"  {i}. [{f.severity}] {location}: {f.body[:120]}")


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    """CLI dispatcher."""
    args = argv if argv is not None else sys.argv[1:]

    if not args:
        print("Usage: pr_review.py <push|ensure-pr|trigger-review|poll-review|run>", file=sys.stderr)
        return 1

    command = args[0]

    if command == "push":
        cmd_push()
    elif command == "ensure-pr":
        pr_number = cmd_ensure_pr()
        print(pr_number)
    elif command == "trigger-review":
        if len(args) < 2:
            print("Usage: pr_review.py trigger-review <pr-number>", file=sys.stderr)
            return 1
        cmd_trigger_review(args[1])
    elif command == "poll-review":
        if len(args) < 3:
            print("Usage: pr_review.py poll-review <pr-number> <trigger-timestamp-iso>", file=sys.stderr)
            return 1
        trigger_time = datetime.fromisoformat(args[2])
        review = cmd_poll_review(args[1], trigger_time)
        if review is None:
            return 1
        print(json.dumps(review, indent=2))
    elif command == "run":
        cmd_run()
    else:
        print(f"Unknown command: {command}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
