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
  reports zero findings. The reviewer provider is resolved via `.harness/config/agent-profiles.json`.
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

## Sandbox and Hook Coverage Warning (External Subprocesses)

Claude Code hooks (e.g. `sotp hook dispatch block-direct-git-ops`) only intercept
**Claude Code's own tool calls**. They do NOT apply to operations performed inside
an external subprocess (e.g. Codex CLI with `--sandbox workspace-write`).

| Sandbox | File writes | Git operations | Hook coverage |
|---------|-------------|----------------|---------------|
| `read-only` | Blocked by sandbox | Blocked by sandbox | N/A |
| `workspace-write` | Allowed | **Allowed — hooks do NOT fire** | None |

**`--full-auto` implies `--sandbox workspace-write`**: Codex CLI's `--full-auto` flag
forces `--sandbox workspace-write`, overriding any subsequent `--sandbox read-only`.
Do not use `--full-auto` for `reviewer` or `researcher` — use `--sandbox read-only` only.

**Consequences when using `workspace-write`:**

- The external subprocess can run `git add` / `git commit` / `git push` directly,
  bypassing the `sotp` guard hook (`block-direct-git-ops`).
- The external subprocess can write any file without hook-based validation.

**Rules for `workspace-write` usage:**

1. Prefer `read-only` for `planner` / `reviewer` / `researcher` — they should never need to write files.
2. When `implementer` is routed to an external provider with `workspace-write`, instruct it explicitly:
   - Do not run `git add` or `git commit` directly.
   - Do not run `git push` under any circumstance.
   - For selective staging, write repo-relative paths to `tmp/track-commit/add-paths.txt` and run `cargo make track-add-paths`.
   - For guarded commits, use `/track:commit` or the exact wrappers `cargo make track-commit-message` / `cargo make track-note`.
3. Hook protections apply to all operations performed during autonomous task execution.
   Do not bypass hook coverage by routing through external subprocesses.

## Duplicate Implementation Prevention

Before writing new parsing/analysis logic, verify the following:

1. Check whether a related convention exists in `knowledge/conventions/`
2. Use `Grep` to search for similar utilities in other crates within the workspace
3. Check whether a matching concern exists in `canonical_modules` in `architecture-rules.json`
4. If none of the above finds a match, have the `researcher` capability perform a quick survey of crates.io for equivalent functionality

Reference: `knowledge/conventions/shell-parsing.md`

## Reviewer Capability Constraint

The `reviewer` capability delegates to an external provider defined in `.harness/config/agent-profiles.json`.
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
