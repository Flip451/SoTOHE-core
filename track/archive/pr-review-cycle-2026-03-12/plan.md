<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# GitHub PR-based review cycle

Add /track:pr-review command for GitHub PR-based review cycle using Codex Cloud @codex review.
Push track branch, create/reuse PR to main, post @codex review comment to trigger Codex Cloud review.
Poll GitHub API for review completion, parse findings from the posted review.
Review -> fix locally -> push -> trigger again loop until zero findings.
Route all git/GitHub operations through cargo make wrappers (no direct gh or git push).
Keep existing /track:review unchanged as fast local reviewer loop.
Requires Codex Cloud GitHub App installed on the repository.

## Permissions, Wrappers, and AGENTS.md

Establish permissions and guardrails before any wrapper execution.
Add Makefile.toml wrapper tasks for PR operations.
Create AGENTS.md with review guidelines for Codex Cloud.

- [x] Permissions and guardrails: add wrapper allow entries (track-pr-push, track-pr-ensure, track-pr-review) to .claude/settings.json, update verify_orchestra_guardrails.py allowlists, keep raw gh and git push blocked
- [x] Makefile wrappers: add track-pr-push, track-pr-ensure, track-pr-review wrapper tasks invoking scripts/pr_review.py subcommands
- [x] AGENTS.md setup: create AGENTS.md with Review guidelines section (coding rules from .claude/rules/04-06, security convention, severity policy P0/P1 only). Document Codex Cloud GitHub App prerequisite in spec.md

## PR Orchestration and Lifecycle

Create scripts/pr_review.py with push, ensure-pr, trigger-review, poll-review, and run subcommands.
Implement PR lifecycle management (discover, create, reuse).

- [x] PR orchestration script skeleton: create scripts/pr_review.py with push/ensure-pr/trigger-review/poll-review/run subcommands, track context resolution. trigger-review posts '@codex review' comment via gh api. Non-structured reviewer providers fail closed
- [x] PR lifecycle: implement gh pr list/create/view for deterministic PR discovery and creation (one track = one PR, create or reuse)

## Async Review Polling and Result Parsing

Poll GitHub API for Codex Cloud review completion after trigger.
Parse review body and inline comments into normalized finding types.

- [x] Review polling and result collection: poll GitHub API for new review from Codex bot after trigger, with configurable interval (default 15s) and timeout (default 10min). Correlate review to current trigger by filtering reviews created after trigger timestamp (reject stale pre-existing reviews). Detect review completion by checking latest review author and state. On timeout, distinguish GitHub App not installed (no Codex bot activity on PR) from bot-busy (comment posted but review pending) with distinct error messages
- [x] Review result parsing: fetch completed Codex review via gh api (review body + inline comments), normalize into ReviewFinding types, count actionable findings (P0/P1), determine pass/fail status

## Command, Tests, and Documentation

Create /track:pr-review command prompt with async flow guidance.
Add tests for all components.
Update workflow and developer documentation.

- [x] /track:pr-review command prompt: create .claude/commands/track/pr-review.md with track resolution, push/ensure-pr/trigger/poll/parse flow, async handling guidance, summary output with PR URL and finding counts
- [x] Tests: PR lifecycle via mocked gh, trigger comment posting, poll timeout/success (including stale review rejection and GitHub App missing detection), review result parsing, fail-closed for non-structured providers, path sanitization (absolute paths, secrets, internal env info), guardrail allowlist sync, /track:review regression (existing local review unaffected)
- [x] Documentation: update CLAUDE.md (command list, Codex Cloud prerequisite), track/workflow.md (pr-review section with async flow), DEVELOPER_AI_WORKFLOW.md (GitHub review workflow)
