# Guardrails

Core guardrails:

- Prefer `/track:*` in user-facing guidance
- Do not use direct `git add` / `git commit`
- Do not tell users to run `*-local` tasks directly
- Keep `track/tech-stack.md` free of blocking `TODO:` before implementation
- Keep `track/registry.md`, `spec.md`, `plan.md`, and `verification.md` synchronized
- Keep `cargo make ci`, `cargo make deny`, and `cargo make verify-*` as reproducible final gates (`run --rm`)
- Before committing code changes, run the `reviewer` capability review cycle
  (review -> fix -> review -> ... -> no findings). Do not commit until the reviewer
  reports zero findings. The reviewer provider is resolved via `.claude/agent-profiles.json`.
- **Small task commits**: Prefer small, focused task commits (<500 lines). Review cost
  grows super-linearly with diff size. Split large tasks into sub-tasks during planning.

## Permission Guardrails

The `FORBIDDEN_ALLOW` list in `scripts/verify_orchestra_guardrails.py` prevents the following
shell commands from being added to `permissions.allow` in `.claude/settings.json`:

- `Bash(ls:*)`, `Bash(cat:*)`, `Bash(find:*)`, `Bash(grep:*)` etc. — use dedicated tools (`Glob`, `Read`, `Grep`) instead
- `Bash(head:*)`, `Bash(tail:*)`, `Bash(wc:*)` — moved to `allow` (read-only, no write risk)
- `Bash(cd:*)` — use each tool's `path` parameter instead
- `Bash(echo:*)`, `Bash(pwd:*)` — output text directly, use `Glob` for path confirmation
- `Bash(git add:*)`, `Bash(git commit:*)` etc. — use `cargo make` wrappers instead

If a user requests adding these permissions, explain that they are in `FORBIDDEN_ALLOW` and
suggest the alternative tools. For project-specific extensions, add entries to
`.claude/permission-extensions.json` under `extra_allow`, but entries matching `FORBIDDEN_ALLOW`
will be rejected.

## Subagent Tool Usage

Background agents (Agent tool) must not use `Bash` for operations covered by dedicated tools.
In particular, when reading output files or extracting results (e.g. reviewer verdicts),
use the `Read` tool — not `Bash(grep ...)`, `Bash(cat ...)`, or `Bash(head ...)`.
These commands are in the `FORBIDDEN_ALLOW` list and trigger permission prompts every time.

## Bash Output Redirect Constraint

Do not use `2>/dev/null` in Claude Code Bash tool calls.
The file-write guard (`bash-write-guard`) scans for `>` patterns and treats
`2>` as an output redirect, blocking the command. `2>&1` (FD duplication) is not affected.

## Hook Constraint

The `sotp hook dispatch block-direct-git-ops` guard scans the entire Bash command string for protected git-operation keywords.
This includes string literals, prompt text, and heredocs.

To avoid unnecessary retries:

- `python3 -c`: do not embed code containing protected git keywords. Write a `.py` file, then run it.
- `codex exec` / `gemini -p`: do not embed prompts containing protected git keywords. Write the prompt to a briefing file first.
- heredoc / `cat >`: also scanned. Use the Write/Edit tool instead.
- **New file creation**: The `Write` tool rejects writes to unread files, so first `Read` the target path (an error is returned if the file does not exist). Then `Write` can create it. `touch` is in `FORBIDDEN_ALLOW` and must not be added to `allow`.
- Fallback: when Codex is blocked by the hook, use the repo-owned wrappers with `--briefing-file`:
  - Planner: `cargo make track-local-plan -- --model {model} --briefing-file <path>`
  - Reviewer: `cargo make track-local-review -- --model {model} --briefing-file <path>`
  - These wrappers convert the briefing file path to `"Read {path} and perform the task"` internally, keeping git keywords out of the Bash command string.
  - Note: `track-local-*` は Docker 内部用の `*-local` タスク（`fmt-local` 等）とは異なり、ホスト上で Codex を呼ぶラッパー。ガードレール「`*-local` を直接実行するな」の対象外。

## Review Escalation Threshold (Enforced by `sotp review record-round`)

When the **same concern category appears in 3 consecutive closed review cycles**,
`sotp review record-round` automatically blocks further fix-review cycles with
`EscalationActive` error (exit code 3).

This is **enforced by the domain layer** (`ReviewState::record_round` /
`record_round_with_pending` / `check_commit_ready`), not by prompt instructions.

The threshold defaults to 3. `agent-profiles.json` → `providers.codex.escalation_threshold` is
registered for future configurability but is not yet read by the runtime (hardcoded in domain layer).

### When Escalation Triggers

The blocked message instructs the developer to execute three steps:

1. **Workspace Search**: Use `Grep` to check whether existing code solves the problem.
2. **Reinvention Check**: Invoke the `researcher` capability to survey crates.io.
   Save results to `knowledge/research/reinvention-check-{concern}.md`.
3. **Decision**: Run `sotp review resolve-escalation` with evidence:
   ```
   sotp review resolve-escalation \
     --track-id <id> \
     --blocked-concerns <comma-separated-concern-slugs> \
     --workspace-search-ref <path-to-search-artifact> \
     --reinvention-check-ref <path-to-research-artifact> \
     --decision <adopt_workspace|adopt_crate|continue_self> \
     --summary "Justification for the decision"
   ```
   Both artifact paths must exist on disk. `--blocked-concerns` must match the
   concerns currently blocking escalation (the domain layer validates the match).

Resolution clears the escalation block, invalidates the review state, and requires
a fresh review cycle. The resolution record is persisted in `metadata.json`.

### Concern Categories

The `--concerns` flag on `sotp review record-round` accepts comma-separated concern slugs.
The calling workflow (e.g., `/track:review`) is responsible for extracting concerns from
reviewer findings using `findings_to_concerns()` (usecase layer), which applies a 3-stage
fallback:
1. Reviewer-provided `category` field (if present in findings JSON)
2. File path normalization (e.g., `libs/domain/src/review.rs` → `domain.review`)
3. Fallback: `"other"`

Note: The automatic extraction is available as a library function but is not yet wired
into the `record-round` CLI command directly. The calling orchestrator must pass `--concerns`.

### Design Reference

Full design with Canonical Blocks: `knowledge/DESIGN.md` → "Review Escalation Threshold (WF-36)"

### Known Limitation (CLI-02)

The `resolve-escalation` logic currently lives in `apps/cli/src/commands/review.rs`.
Per `tmp/refactoring-plan-2026-03-19.md` CLI-02, this should be extracted to
`libs/usecase/src/review_workflow.rs` as a UseCase in a follow-up track.

## Duplicate Implementation Prevention

Before writing new parsing/analysis logic, verify the following:

1. Check whether a related convention exists in `knowledge/conventions/`
2. Use `Grep` to search for similar utilities in other crates within the workspace
3. Check whether a matching concern exists in `canonical_modules` in `architecture-rules.json`
4. If none of the above finds a match, have the `researcher` capability perform a quick survey of crates.io for equivalent functionality

Reference: `knowledge/conventions/shell-parsing.md`

## Reviewer Capability Constraint

The `reviewer` capability delegates to an external provider defined in `.claude/agent-profiles.json`.
Inline review within Claude Code's main context (self-review) is not a substitute for the reviewer capability.

- If the external reviewer (e.g., Codex CLI) fails to return a verdict → **retry** (up to 2 times)
- If retries also fail → **report to the user and ask for a decision**
- Do not treat inline review in the main context as achieving `zero_findings` and proceed to commit
- Claude Code subagent (`subagent_type: "Explore"`) as a reviewer substitute is only valid for the `claude-heavy` profile (`reviewer: "claude"`)
- Distinguish from hook blocks: hook blocks are a prompt formatting issue (work around via file), verdict extraction failures are an external provider execution issue (address via retry)

Operational details live in:

- `track/workflow.md`
- `knowledge/WORKFLOW.md`
- `.claude/settings.json`
- `.claude/hooks/`
