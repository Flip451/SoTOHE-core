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

## Hook Constraint

The `sotp hook dispatch block-direct-git-ops` guard scans the entire Bash command string for protected git-operation keywords.
This includes string literals, prompt text, and heredocs.

To avoid unnecessary retries:

- `python3 -c`: do not embed code containing protected git keywords. Write a `.py` file, then run it.
- `codex exec` / `gemini -p`: do not embed prompts containing protected git keywords. Write the prompt to a file first.
- heredoc / `cat >`: also scanned. Use the Write/Edit tool instead.
- Fallback: when Codex review is blocked by the hook, perform inline review.

Operational details live in:

- `track/workflow.md`
- `.claude/docs/WORKFLOW.md`
- `.claude/settings.json`
- `.claude/hooks/`
