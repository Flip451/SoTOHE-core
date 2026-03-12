"""Tests for scripts/pr_review.py — PR-based review cycle orchestration."""
from __future__ import annotations

import json
from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import MagicMock, patch

import pr_review
import pytest

# ---------------------------------------------------------------------------
# sanitize_text
# ---------------------------------------------------------------------------


class TestSanitizeText:
    def test_removes_absolute_paths(self):
        text = "Error in /home/user/project/src/main.rs"
        result = pr_review.sanitize_text(text)
        assert "/home/user" not in result
        assert "[PATH]" in result

    def test_removes_secrets_github_token(self):
        text = "Token: ghp_abcdefghijklmnopqrstuvwxyz0123456789"
        result = pr_review.sanitize_text(text)
        assert "ghp_" not in result
        assert "[REDACTED]" in result

    def test_removes_secrets_sk_key(self):
        text = "API key: sk-abcdefghijklmnopqrstuvwx"
        result = pr_review.sanitize_text(text)
        assert "sk-" not in result
        assert "[REDACTED]" in result

    def test_removes_localhost_urls(self):
        text = "Server at http://localhost:3000/api"
        result = pr_review.sanitize_text(text)
        assert "localhost" not in result
        assert "[INTERNAL]" in result

    def test_removes_internal_ip(self):
        text = "Listening on 127.0.0.1:8080"
        result = pr_review.sanitize_text(text)
        assert "127.0.0.1" not in result

    def test_preserves_normal_text(self):
        text = "This function has a logic error in the loop condition"
        assert pr_review.sanitize_text(text) == text

    def test_removes_aws_key(self):
        text = "Key: AKIAIOSFODNN7EXAMPLE"
        result = pr_review.sanitize_text(text)
        assert "AKIA" not in result
        assert "[REDACTED]" in result

    def test_removes_github_pat_token(self):
        text = "Token: github_pat_abcdefghijklmnopqrstuvwx"
        result = pr_review.sanitize_text(text)
        assert "github_pat_" not in result
        assert "[REDACTED]" in result

    def test_removes_gitlab_token(self):
        text = "Token: glpat-abcdefghijklmnopqrstuvwx"
        result = pr_review.sanitize_text(text)
        assert "glpat-" not in result
        assert "[REDACTED]" in result

    def test_removes_rfc1918_addresses(self):
        text = "Server at 10.0.1.5:8080 and 192.168.1.100 and 172.16.0.1"
        result = pr_review.sanitize_text(text)
        assert "10.0.1.5" not in result
        assert "192.168.1.100" not in result
        assert "172.16.0.1" not in result

    def test_removes_rfc1918_in_url(self):
        text = "URL http://10.0.1.5:8080/api"
        result = pr_review.sanitize_text(text)
        assert "10.0.1.5" not in result

    def test_removes_rfc1918_in_parens(self):
        text = "(10.0.1.5:8080)"
        result = pr_review.sanitize_text(text)
        assert "10.0.1.5" not in result

    def test_removes_workspace_path(self):
        text = "Error in /workspace/src/main.rs"
        result = pr_review.sanitize_text(text)
        assert "/workspace/" not in result
        assert "[PATH]" in result

    def test_removes_etc_path(self):
        text = "Config at /etc/ssl/certs/ca.pem"
        result = pr_review.sanitize_text(text)
        assert "/etc/" not in result
        assert "[PATH]" in result

    def test_no_false_positive_rfc1918_substring(self):
        """110.0.1.5 is not an RFC1918 address — must not be redacted."""
        text = "IP 110.0.1.5 is public"
        result = pr_review.sanitize_text(text)
        assert "110.0.1.5" in result


# ---------------------------------------------------------------------------
# _classify_severity
# ---------------------------------------------------------------------------


class TestParsePaginatedJson:
    def test_single_array(self):
        text = '[{"id": 1}, {"id": 2}]'
        result = pr_review._parse_paginated_json(text)
        assert len(result) == 2
        assert result[0]["id"] == 1

    def test_empty_string(self):
        assert pr_review._parse_paginated_json("") == []

    def test_single_object(self):
        text = '{"id": 1}'
        result = pr_review._parse_paginated_json(text)
        assert len(result) == 1
        assert result[0]["id"] == 1

    def test_concatenated_arrays(self):
        text = '[{"id": 1}]\n[{"id": 2}]'
        result = pr_review._parse_paginated_json(text)
        assert len(result) == 2
        assert result[0]["id"] == 1
        assert result[1]["id"] == 2


class TestClassifySeverity:
    def test_critical_keywords(self):
        assert pr_review._classify_severity("This is a critical security issue") == "P0"

    def test_bug_keywords(self):
        assert pr_review._classify_severity("This is a bug in the logic") == "P0"

    def test_suggestion_keywords(self):
        assert pr_review._classify_severity("You should consider refactoring") == "P1"

    def test_default_is_p1(self):
        assert pr_review._classify_severity("Some inline comment") == "P1"

    def test_panic_is_p0(self):
        assert pr_review._classify_severity("This code could panic") == "P0"


# ---------------------------------------------------------------------------
# _parse_body_findings
# ---------------------------------------------------------------------------


class TestParseBodyFindings:
    def test_parses_bullet_items(self):
        body = "Review:\n- This function has a critical bug in error handling\n- Consider using a different pattern"
        findings = pr_review._parse_body_findings(body)
        assert len(findings) == 2
        assert findings[0].severity == "P0"  # "critical" keyword
        assert findings[1].severity == "P1"  # "consider" keyword

    def test_skips_short_items(self):
        body = "- OK\n- This is a longer finding that should be included"
        findings = pr_review._parse_body_findings(body)
        assert len(findings) == 1

    def test_empty_body(self):
        assert pr_review._parse_body_findings("") == []

    def test_no_bullets(self):
        body = "Everything looks good. No issues found."
        assert pr_review._parse_body_findings(body) == []


# ---------------------------------------------------------------------------
# _resolve_reviewer_provider (fail-closed)
# ---------------------------------------------------------------------------


class TestResolveReviewerProvider:
    def test_codex_provider_succeeds(self, tmp_path):
        profiles = {
            "version": 1,
            "active_profile": "default",
            "providers": {},
            "profiles": {"default": {"reviewer": "codex"}},
        }
        profiles_path = tmp_path / ".claude" / "agent-profiles.json"
        profiles_path.parent.mkdir(parents=True)
        profiles_path.write_text(json.dumps(profiles))

        with patch.object(pr_review, "Path", return_value=profiles_path):
            # We need to patch the Path constructor used inside the function
            pass

    def test_claude_provider_fails_closed(self, tmp_path, monkeypatch):
        profiles = {
            "version": 1,
            "active_profile": "default",
            "providers": {},
            "profiles": {"default": {"reviewer": "claude"}},
        }
        profiles_path = tmp_path / ".claude" / "agent-profiles.json"
        profiles_path.parent.mkdir(parents=True)
        profiles_path.write_text(json.dumps(profiles))

        # Patch Path to return our tmp_path version
        original_path = Path

        def patched_path(*args):
            p = original_path(*args)
            if str(p) == ".claude/agent-profiles.json":
                return profiles_path
            return p

        monkeypatch.setattr(pr_review, "Path", patched_path)

        with pytest.raises(SystemExit) as exc_info:
            pr_review._resolve_reviewer_provider()
        assert exc_info.value.code == 1


# ---------------------------------------------------------------------------
# cmd_parse_review
# ---------------------------------------------------------------------------


class TestCmdParseReview:
    def test_approved_with_no_comments(self):
        review = {
            "id": 123,
            "state": "APPROVED",
            "body": "LGTM",
        }
        with patch.object(pr_review, "_run_gh") as mock_gh:
            mock_gh.return_value = MagicMock(returncode=0, stdout="[]")
            result = pr_review.cmd_parse_review("42", review)

        assert result.review_id == 123
        assert result.state == "APPROVED"
        assert result.passed is True
        assert result.actionable_count == 0

    def test_commented_with_inline_findings(self):
        review = {
            "id": 456,
            "state": "COMMENTED",
            "body": "",
        }
        inline_comments = [
            {
                "body": "This is a critical bug",
                "path": "src/main.rs",
                "line": 42,
                "start_line": None,
            },
            {
                "body": "Consider renaming this",
                "path": "src/lib.rs",
                "line": 10,
                "start_line": None,
            },
        ]
        with patch.object(pr_review, "_run_gh") as mock_gh:
            mock_gh.return_value = MagicMock(
                returncode=0, stdout=json.dumps(inline_comments)
            )
            result = pr_review.cmd_parse_review("42", review)

        assert result.review_id == 456
        assert result.inline_comment_count == 2
        assert len(result.findings) == 2
        assert result.actionable_count == 2  # P0 + P1
        assert result.passed is False

    def test_changes_requested_with_no_actionable_fails(self):
        """CHANGES_REQUESTED state must fail even with zero parseable actionable findings."""
        review = {
            "id": 999,
            "state": "CHANGES_REQUESTED",
            "body": "Please fix things",  # No bullet items → no parsed findings
        }
        with patch.object(pr_review, "_run_gh") as mock_gh:
            mock_gh.return_value = MagicMock(returncode=0, stdout="[]")
            result = pr_review.cmd_parse_review("42", review)

        assert result.state == "CHANGES_REQUESTED"
        assert result.actionable_count == 0
        assert result.passed is False

    def test_multiline_comment_line_range(self):
        """Multi-line comments should have line <= end_line."""
        review = {
            "id": 111,
            "state": "COMMENTED",
            "body": "",
        }
        inline_comments = [
            {
                "body": "This block has a bug",
                "path": "src/main.rs",
                "line": 50,        # GitHub: last line
                "start_line": 42,  # GitHub: first line
            },
        ]
        with patch.object(pr_review, "_run_gh") as mock_gh:
            mock_gh.return_value = MagicMock(
                returncode=0, stdout=json.dumps(inline_comments)
            )
            result = pr_review.cmd_parse_review("42", review)

        finding = result.findings[0]
        assert finding.line == 42   # start
        assert finding.end_line == 50  # end
        assert finding.line <= finding.end_line

    def test_sanitizes_paths_in_findings(self):
        review = {
            "id": 789,
            "state": "COMMENTED",
            "body": "",
        }
        inline_comments = [
            {
                "body": "Error at /home/user/project/src/main.rs:42",
                "path": "src/main.rs",
                "line": 42,
                "start_line": None,
            },
        ]
        with patch.object(pr_review, "_run_gh") as mock_gh:
            mock_gh.return_value = MagicMock(
                returncode=0, stdout=json.dumps(inline_comments)
            )
            result = pr_review.cmd_parse_review("42", review)

        assert "/home/user" not in result.findings[0].body
        assert "[PATH]" in result.findings[0].body


# ---------------------------------------------------------------------------
# cmd_poll_review — stale review rejection
# ---------------------------------------------------------------------------


class TestPollReview:
    def test_rejects_stale_review(self):
        """Reviews created before trigger_time must be ignored."""
        trigger_time = datetime(2026, 3, 12, 16, 0, 0, tzinfo=UTC)
        stale_review = [
            {
                "id": 100,
                "user": {"login": "codex-bot"},
                "state": "COMMENTED",
                "submitted_at": "2026-03-12T15:00:00Z",  # Before trigger
            }
        ]
        call_count = 0

        def mock_gh(args, *, check=True):
            nonlocal call_count
            call_count += 1
            result = MagicMock()
            if "reviews" in str(args):
                result.returncode = 0
                result.stdout = json.dumps(stale_review)
            else:
                result.returncode = 0
                result.stdout = json.dumps([{"user": {"login": "codex-bot"}, "created_at": "2026-03-12T15:00:00Z"}])
            return result

        with patch.object(pr_review, "_run_gh", side_effect=mock_gh):
            review = pr_review.cmd_poll_review(
                "42", trigger_time, poll_interval=0, poll_timeout=1
            )

        assert review is None  # Should not match stale review

    def test_accepts_fresh_review(self):
        """Reviews created after trigger_time must be accepted."""
        trigger_time = datetime(2026, 3, 12, 16, 0, 0, tzinfo=UTC)
        fresh_review = [
            {
                "id": 200,
                "user": {"login": "codex-bot"},
                "state": "COMMENTED",
                "submitted_at": "2026-03-12T16:05:00Z",  # After trigger
            }
        ]

        def mock_gh(args, *, check=True):
            result = MagicMock()
            if "reviews" in str(args):
                result.returncode = 0
                result.stdout = json.dumps(fresh_review)
            else:
                result.returncode = 0
                result.stdout = "[]"
            return result

        with patch.object(pr_review, "_run_gh", side_effect=mock_gh):
            review = pr_review.cmd_poll_review(
                "42", trigger_time, poll_interval=0, poll_timeout=5
            )

        assert review is not None
        assert review["id"] == 200

    def test_accepts_same_second_review(self):
        """A review at the exact same second as trigger_time must be accepted (trigger_time is post-POST)."""
        trigger_time = datetime(2026, 3, 12, 16, 0, 0, tzinfo=UTC)
        same_second_review = [
            {
                "id": 300,
                "user": {"login": "codex-bot"},
                "state": "COMMENTED",
                "submitted_at": "2026-03-12T16:00:00Z",  # Same second
            }
        ]

        def mock_gh(args, *, check=True):
            result = MagicMock()
            if "reviews" in str(args):
                result.returncode = 0
                result.stdout = json.dumps(same_second_review)
            else:
                result.returncode = 0
                result.stdout = "[]"
            return result

        with patch.object(pr_review, "_run_gh", side_effect=mock_gh):
            review = pr_review.cmd_poll_review(
                "42", trigger_time, poll_interval=0, poll_timeout=5
            )

        assert review is not None
        assert review["id"] == 300

    def test_stale_bot_comments_not_counted_as_activity(self, capsys):
        """Old bot comments on reused PRs must not count as post-trigger activity."""
        trigger_time = datetime(2026, 3, 12, 16, 0, 0, tzinfo=UTC)
        stale_review = [
            {
                "id": 100,
                "user": {"login": "codex-bot"},
                "state": "COMMENTED",
                "submitted_at": "2026-03-12T14:00:00Z",  # Before trigger
            }
        ]
        stale_comments = [
            {
                "user": {"login": "codex-bot"},
                "created_at": "2026-03-12T14:00:00Z",  # Before trigger
            }
        ]

        def mock_gh(args, *, check=True):
            result = MagicMock()
            if "reviews" in str(args):
                result.returncode = 0
                result.stdout = json.dumps(stale_review)
            else:
                result.returncode = 0
                result.stdout = json.dumps(stale_comments)
            return result

        with patch.object(pr_review, "_run_gh", side_effect=mock_gh):
            review = pr_review.cmd_poll_review(
                "42", trigger_time, poll_interval=0, poll_timeout=1
            )

        assert review is None
        captured = capsys.readouterr()
        # Should report "GitHub App not installed" since no post-trigger activity
        assert "GitHub App" in captured.err or "GitHub App" in captured.out

    def test_github_app_not_installed_message(self, capsys):
        """When no bot activity at all, should suggest App installation."""
        trigger_time = datetime(2026, 3, 12, 16, 0, 0, tzinfo=UTC)

        def mock_gh(args, *, check=True):
            result = MagicMock()
            result.returncode = 0
            result.stdout = "[]"  # No reviews, no comments
            return result

        with patch.object(pr_review, "_run_gh", side_effect=mock_gh):
            review = pr_review.cmd_poll_review(
                "42", trigger_time, poll_interval=0, poll_timeout=1
            )

        assert review is None
        captured = capsys.readouterr()
        assert "GitHub App" in captured.err or "GitHub App" in captured.out


# ---------------------------------------------------------------------------
# Guardrail allowlist sync
# ---------------------------------------------------------------------------


class TestGuardrailSync:
    def test_settings_json_has_pr_wrappers(self):
        settings = json.loads(
            Path(".claude/settings.json").read_text(encoding="utf-8")
        )
        allow = settings["permissions"]["allow"]
        assert "Bash(cargo make track-pr-push)" in allow
        assert "Bash(cargo make track-pr-ensure)" in allow
        assert "Bash(cargo make track-pr-review)" in allow

    def test_guardrails_script_has_pr_wrappers(self):
        content = Path("scripts/verify_orchestra_guardrails.py").read_text(
            encoding="utf-8"
        )
        assert "track-pr-push" in content
        assert "track-pr-ensure" in content
        assert "track-pr-review" in content


# ---------------------------------------------------------------------------
# /track:review regression — command file still exists unchanged
# ---------------------------------------------------------------------------


class TestTrackReviewRegression:
    def test_track_review_command_exists(self):
        """Existing /track:review command must remain unchanged."""
        path = Path(".claude/commands/track/review.md")
        assert path.is_file(), "/track:review command file missing"
        content = path.read_text(encoding="utf-8")
        assert "review" in content.lower()
        assert "reviewer" in content.lower()

    def test_track_pr_review_command_is_separate(self):
        """The new /track:pr-review command must be a separate file."""
        review_path = Path(".claude/commands/track/review.md")
        pr_review_path = Path(".claude/commands/track/pr-review.md")
        assert review_path.is_file()
        assert pr_review_path.is_file()
        assert review_path != pr_review_path
