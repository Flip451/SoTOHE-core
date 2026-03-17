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

## Permission Guardrails

The `FORBIDDEN_ALLOW` list in `scripts/verify_orchestra_guardrails.py` prevents the following
shell commands from being added to `permissions.allow` in `.claude/settings.json`:

- `Bash(ls:*)`, `Bash(cat:*)`, `Bash(find:*)`, `Bash(grep:*)`, `Bash(head:*)`, `Bash(tail:*)` etc. — use dedicated tools (`Glob`, `Read`, `Grep`) instead
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

## Hook Constraint

The `sotp hook dispatch block-direct-git-ops` guard scans the entire Bash command string for protected git-operation keywords.
This includes string literals, prompt text, and heredocs.

To avoid unnecessary retries:

- `python3 -c`: do not embed code containing protected git keywords. Write a `.py` file, then run it.
- `codex exec` / `gemini -p`: do not embed prompts containing protected git keywords. Write the prompt to a file first.
- heredoc / `cat >`: also scanned. Use the Write/Edit tool instead.
- **New file creation**: The `Write` tool rejects writes to unread files, so first `Read` the target path (an error is returned if the file does not exist). Then `Write` can create it. `touch` is in `FORBIDDEN_ALLOW` and must not be added to `allow`.
- Fallback: when Codex review is blocked by the hook, write the prompt to a file and retry with `--briefing-file`.

## Review Escalation Threshold

When the **same category of bug fix occurs 3 consecutive rounds**, stop patching and execute
the following steps instead:

### Step 1: Workspace Search

Use `Grep` to check whether existing code in the workspace already solves the same problem.

### Step 2: Reinvention Check (Automated Researcher Survey)

Invoke the `researcher` capability (default: Gemini CLI) to investigate:

1. **Generalize the problem**: Abstract the problem domain that triggered 3 consecutive fixes (e.g., "shell tokenization edge cases" → "POSIX shell parsing")
2. **Survey existing crates**: Search crates.io for Rust crates that solve the generalized problem
3. **Evaluate fitness**: Assess whether discovered crates meet the following criteria:
   - Maintenance status (last release within 6 months, or stable)
   - License compatibility (`cargo make deny` allowlist)
   - API fit (covers the project's use cases)
   - Dependency tree bloat risk

Example survey prompt:

```
Research Rust crates that solve: {generalized problem description}.
Requirements: {specific needs from the 3 consecutive failures}.
Evaluate: maintenance status, license (MIT/Apache-2.0), API fit, dependency footprint.
Compare top 3 candidates. Recommend: adopt existing crate, or justify continued self-implementation.
```

Save survey results to `.claude/docs/research/reinvention-check-{concern}.md`.

### Step 3: Decision and Escalation

Present options to the user based on the survey results:

| Decision | Condition | Action |
|----------|-----------|--------|
| **Adopt crate** | A suitable crate exists | Present `cargo add` + migration plan |
| **Migrate within workspace** | Solvable via existing canonical module | Present migration diff |
| **Continue self-implementation** | No suitable crate, or special requirements | Document justification and continue |

Do not adopt crates or perform large-scale migrations without user approval.

Example: shell tokenization bypasses found 3 times → researcher discovers "conch-parser is already in vendor/" → propose migration to `domain::guard::parser`

## Duplicate Implementation Prevention

Before writing new parsing/analysis logic, verify the following:

1. Check whether a related convention exists in `project-docs/conventions/`
2. Use `Grep` to search for similar utilities in other crates within the workspace
3. Check whether a matching concern exists in `canonical_modules` in `docs/architecture-rules.json`
4. If none of the above finds a match, have the `researcher` capability perform a quick survey of crates.io for equivalent functionality

Reference: `project-docs/conventions/shell-parsing.md`

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
- `.claude/docs/WORKFLOW.md`
- `.claude/settings.json`
- `.claude/hooks/`
