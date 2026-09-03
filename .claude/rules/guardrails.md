---
paths:
  - ".harness/**"
  - "track/**"
  - ".claude/commands/**"
  - ".claude/agents/**"
  - ".codex/**"
  - ".agents/**"
  - ".claude/settings.json"
  - ".claude/permission-extensions.json"
---

# Guardrails

This conditionally loaded document is a reference guardrail document for orchestrator workflow
work. The concise always-applied root rules are in `.claude/rules/orchestrator.md`; review
briefings are loaded by the review workflow.

Core guardrails:

- Prefer `/track:*` in user-facing guidance
- Do not use direct `git add` / `git commit`
- Keep `track/registry.md`, `spec.md`, and `plan.md` synchronized
- Keep `cargo make ci`, `cargo make deny`, and `cargo make verify-*` as reproducible final gates
- Before committing code changes, run the `reviewer` capability review cycle
  (review -> fix -> review -> ... -> no findings). Do not commit until the reviewer
  reports zero findings. The reviewer provider is resolved via `.harness/config/agent-profiles.json`.
- **Small task commits**: Prefer small, focused task commits (<500 lines). Review cost
  grows super-linearly with diff size. Split large tasks into sub-tasks during planning.
  Review cost scales roughly O(N^2) with diff size (O(N) comprehension x O(N) findings);
  splitting M tasks reduces cost to O(N^2/M).

## Permission Guardrails

`permissions.allow` in `.claude/settings.json` is the template consumer's responsibility
(`.harness/policies/consumer-ownership.md`): SoTOHE ships a recommended default allowlist
and documents which commands are safe vs dangerous to allow, but does **not** CI-enforce it. The
lists below are guidance, not a gate — the consumer owns their permission posture.

### Allowed (present in `permissions.allow`)

These commands are in `permissions.allow`. Use dedicated tools (`Glob`, `Grep`, `Read`) when
they can fully replace the Bash command — the Bash entries exist for cases where dedicated tools
cannot fully substitute (e.g., GNU grep flags, jq JSON filters):

- `Bash(head:*)`, `Bash(tail:*)`, `Bash(wc:*)` — read-only, no write risk
- `Bash(grep:*)`, `Bash(diff:*)`, `Bash(jq:*)`, `Bash(pwd:*)` — read-only; prefer Glob/Grep/Read for normal searches
- `Bash(uniq:*)` — 第2引数で write 可能だが exec 機構を持たないため許容 (Write tool と同等権限)

### Dangerous to allow (excluded from the default allowlist)

The recommended default allowlist excludes the following dangerous shell commands. Adding any of
them to `.claude/settings.json` is the consumer's choice and risk — there is no CI gate rejecting it:

- `Bash(ls:*)`, `Bash(cat:*)` — use dedicated tools (`Glob`, `Read`) instead
- `Bash(cd:*)` — use each tool's `path` parameter instead
- `Bash(echo:*)` — output text directly
- `Bash(sed:*)`, `Bash(awk:*)` — destructive flag (`-i`) 可能のため維持。行内編集は Edit tool を使う
- `Bash(xargs:*)` — 任意 command を exec できるため維持
- `Bash(find:*)` — `-exec`/`-execdir` で任意 utility exec、`-delete`/`-fprint` で destructive 操作可能 (env 型の wrap-execute 脆弱性)。維持
- `Bash(sort:*)` — GNU sort の `--compress-program=PROG` で temporary files 処理時に任意プログラムを exec する wrap-execute 脆弱性 (env / find -exec と同型)。維持
- `Bash(env:*)` — `env [name=value ...] [utility [argument ...]]` 形式で任意 utility を exec する wrapper。allow すると `env git commit` 等で他の guardrail を bypass できるため維持
- `Bash(git add:*)`, `Bash(git commit:*)` etc. — use `/track:*` or guarded `bin/sotp` workflow commands instead

If asked to add one of these, explain the risk and suggest the safer alternative tools. The
consumer may still choose to add it (their responsibility); SoTOHE no longer rejects it via CI.
For project-specific extensions, `.claude/permission-extensions.json` under `extra_allow` remains
the place to record additions.

## Subagent Tool Usage

Background agents (Agent tool) must not use `Bash` for operations covered by dedicated tools.
In particular, when reading output files or extracting results (e.g. reviewer verdicts),
use the `Read` tool — not `Bash(cat ...)`.
`Bash(grep ...)` は permission 上は allow 済だが、
ripgrep ベースの `Grep` / `Glob` で完全置換できる検索は専用 tool を優先した方が UX が良い。
`Bash(find ...)` は FORBIDDEN (wrap-execute 脆弱性)。`Glob` で代替すること。
`Bash(head ...)` / `Bash(tail ...)` も同様に allow 済だが、Read tool の offset/limit で
ファイルの一部を読めるのでまず Read を検討する。

## Hook Constraint

Command-enforcement semantics are owned by the [bash write guard reference](../../.harness/reference/guard-semantics.md)
and its hook dispatcher. The ADR index records the governing decision. Use workflow commands
rather than attempting to construct a guard token yourself.

## Sandbox and Hook Coverage Warning (External Subprocesses)

Claude Code hooks (e.g. `sotp hook dispatch block-direct-git-ops`) only intercept
**Claude Code's own tool calls**. They do NOT apply to operations performed inside
an external subprocess (e.g. Codex CLI with `--sandbox workspace-write`). Repository Git hooks
are separate: Git invokes them for the operations they cover, including when the caller is an
external subprocess. See the hook dispatcher for their current enforcement semantics.

| Sandbox | File writes | Git operations | Hook coverage |
|---------|-------------|----------------|---------------|
| `read-only` | Blocked by sandbox | Blocked by sandbox | N/A |
| `workspace-write` | Allowed | Allowed; repository Git hooks still apply | Claude Code hooks do not apply |

**`--full-auto` implies `--sandbox workspace-write`**: Codex CLI's `--full-auto` flag
forces `--sandbox workspace-write`, overriding any subsequent `--sandbox read-only`.
Do not use `--full-auto` for `reviewer` or `researcher` — use `--sandbox read-only` only.

**Consequences when using `workspace-write`:**

- The external subprocess can run `git add` and write ordinary files without Claude Code
  hook interception. Repository Git hooks continue to govern the ref updates and pushes they cover.
- The external subprocess can write any file without hook-based validation.

**Rules for `workspace-write` usage:**

1. Prefer `read-only` for `researcher` and `rollback-diagnoser` — they should never need to write files.
2. When an external `orchestrator-output` capability needs `workspace-write` (for example, `implementer`), invoke it through `bin/sotp capability exec`. The dispatcher derives the sandbox from the provider-native skill and injects the shared no-direct-git discipline; do not bypass that path with a hand-assembled provider command. Typed-pipeline capabilities keep their dedicated routes.
3. Treat Claude Code hooks and repository Git hooks as separate controls. Do not use an
   external subprocess to evade the workflow-command requirement.

## Duplicate Implementation Prevention

Before writing new parsing/analysis logic, verify the following:

1. Check whether a related convention exists in `knowledge/conventions/`
2. Use `Grep` to search for similar utilities in other crates within the workspace
3. Check whether a matching concern exists in `canonical_modules` in `architecture-rules.json`
4. If none of the above finds a match, have the `researcher` capability perform a quick survey of crates.io for equivalent functionality

Apply the relevant `knowledge/conventions/` guidance before adding new parsing or analysis logic.

## ADR Baseline Guardrail

ADR baseline copies and `ledger.jsonl` under `track/items/<id>/adr-baseline/` are machine-owned.
Only `bin/sotp adr-baseline snapshot` may add them and only `bin/sotp adr-baseline restore` may
restore an ADR from them. Never hand-edit, copy, delete, or re-create a baseline record. At Phase
0, the orchestrator designates primary ADR source(s) by init-stamping them; the ledger init
records are the designation records, with no separate primary identity. The pre-review CLI
requires a nonempty init-record designation set and verifies every recorded ledger copy; a
current ADR that differs from its latest baseline is a normal Phase 0 draft state and does not
block review. `--primary-source <file>` is available only for a direct
`bin/sotp adr-baseline check-review` invocation. Byte matching fail-closes at the commit gate
and track-aware CI (`check-commit`); spec-cited ADR coverage is enforced separately at the
commit gate. A failed `check-review` or `check-commit` is fail-closed; use the
diagnoser/recovery path instead of bypassing the gate.

This is an independent byte-comparison guard. Do not weaken or modify `adr_user` evaluation or
`.harness/config/signal-gates.json` to accommodate it.

## Reviewer Capability Constraint

The `reviewer` capability delegates to a provider defined in `.harness/config/agent-profiles.json`.
Inline review within Claude Code's main context (self-review) is not a substitute for the reviewer capability.

- The official path for `reviewer.provider: claude` is `sotp review local` (resolved in `/track:review` Step 1), which auto-resolves the provider and dispatches to the `ClaudeReviewer` adapter (a read-only `claude -p` headless subprocess). This is the only sanctioned Claude reviewer path; an ad-hoc `subagent_type: "Explore"` self-review is **never** a substitute for it, under any profile.
- If the reviewer fails to return a verdict → **retry** (up to 2 times). This discipline is provider-neutral: it applies to every provider the reviewer lane supports, so a Codex CLI invocation failure, a `ClaudeReviewer` stdout-envelope parse failure, and a Grok reviewer failure are all handled the same way.
- If retries also fail → **report to the user and ask for a decision**
- If the `reviewer` capability resolves to no provider (undefined / unresolvable) → fail-closed; do not run the review with an unknown provider.
- Do not treat inline review in the main context as achieving `zero_findings` and proceed to commit
- Distinguish from hook blocks: hook blocks are a prompt formatting issue (work around via file), verdict extraction failures are an external provider execution issue (address via retry)

Operational details live in:

- `.harness/policies/branch-strategy.md`
- `.harness/policies/track-lifecycle.md`
- `.harness/policies/git-notes.md`
- `README.md`
- `.claude/settings.json`
- `.claude/hooks/`
