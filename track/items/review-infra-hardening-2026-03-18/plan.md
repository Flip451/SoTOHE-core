<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Review infrastructure hardening: stderr capture, review state in metadata.json, commit guard

Harden review infrastructure: capture Codex session logs for traceability, integrate review state into metadata.json as SSoT, enforce sequential model escalation, and gate commits on review approval

## Codex Session Traceability

Capture Codex stderr to persistent log files and extract verdict from stderr as fallback when --output-last-message file is empty.

- [ ] Capture Codex stderr to session log file and add verdict extraction fallback when codex-last-message is empty

## Review State in metadata.json

Extend metadata.json schema with review section tracking round results, model escalation state, and code hash for stale verdict detection.

- [ ] Add review section to metadata.json schema v3 via serde(default) (ReviewStatus enum: not_started/invalidated/fast_passed/approved, code_hash, groups with fast/final round results)

## Review State Recording + Sequential Escalation

Add sotp review record-round command. Validates code_hash freshness and sequential escalation (fast before final), then writes aggregated results to metadata.json. sotp review codex-local stays stateless — parallel-safe.

- [ ] Add sotp review record-round command that validates sequential escalation (code_hash + review.status) and writes aggregated round results to metadata.json. sotp review codex-local remains stateless (verdict on stdout only).

## Commit Guard

Gate track-commit-message on review.status == approved in metadata.json. Auto-reset review state when code changes are detected via git tree hash.

- [ ] Add review.status == approved guard to track-commit-message with automatic reset on code changes (git tree hash)
