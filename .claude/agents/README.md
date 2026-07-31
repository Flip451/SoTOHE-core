# Agent Definitions

`.claude/agents/` holds the custom subagent definitions that Claude Code invokes through `subagent_type`. Each file is a thin adapter that points back to a provider-agnostic capability contract under `.harness/capabilities/`; the contract owns the behaviour, the adapter owns only the Claude-side invocation surface.

## Why the frontmatter carries `model` and `effort`

`bin/sotp capability exec` returns one of two outcomes. When the resolved provider differs from the host it runs the provider subprocess itself and passes `--model` / `--effort` from `.harness/config/agent-profiles.json`. When the resolved provider **is** the host — a Claude orchestrator dispatching a `provider: claude` capability — it returns `delegate-in-host`, and that payload carries only `capability`, `briefing_file`, and `discipline`. It carries neither model nor effort.

So on the `delegate-in-host` path the frontmatter in these files is the *only* surface that sets model and effort. A file whose `effort:` disagrees with its capability's `reasoning_effort` in `agent-profiles.json` produces different behaviour depending on which host runs it. Keep the two in sync.

## Included agents

One file per `provider: claude` capability. `orchestrator` has no file — it is the main session itself.

| agent file | capability | model | effort | invoked by |
|---|---|---|---|---|
| `spec-designer.md` | spec-designer | `claude-opus-5` | `high` | `/track:spec-design` (Phase 1) — authors `spec.json` |
| `type-designer.md` | type-designer | `claude-opus-5` | `high` | `/track:type-design` (Phase 2) — authors `<layer>-types.json` |
| `impl-planner.md` | impl-planner | `claude-opus-5` | `medium` | `/track:impl-plan` (Phase 3) — authors `impl-plan.json` + `task-coverage.json` + `task-contract.json` + `batch-plan.json` |
| `adr-editor.md` | adr-editor | `claude-opus-5` | `high` | the sole in-track writer for `knowledge/adr/*.md` |
| `adr-diagnoser.md` | adr-diagnoser | `claude-opus-5` | `xhigh` | guardian for recorded ADR decisions; read-only verdicts, no `Edit`/`Write` |
| `rollback-diagnoser.md` | rollback-diagnoser | `claude-opus-5` | `xhigh` | `/track:diagnose` — routes a finding back to the phase owning its root cause |
| `implementer.md` | implementer | `claude-opus-5` | `medium` | `/track:implement` — implements assigned plan tasks |
| `review-fix-lead.md` | review-fix-lead | `claude-opus-5` | `medium` | `/track:review` — owns one scope's fix+review loop |
| `researcher.md` | researcher | `claude-opus-5` | `medium` | crate research, codebase-wide analysis, external research |

The two `xhigh` entries are the pure-judgment lanes: they run rarely, their verdicts have the highest leverage in the pipeline, and their outputs are small structured objects, so the cost of the top effective tier is bounded.

`dry-fix-lead.md` is present but dormant: `capabilities.dry-fix-lead` routes to codex, and `cargo make track-local-dry-fix` implements only the codex provider path, so a Claude resolution fails closed rather than reaching the file. It declares no `effort:` for that reason. Unlike `review-fix-lead`, the dry wrapper has no subagent-dispatch sentinel; adding one spans usecase, infrastructure, cli-composition, and cli-driver, and is deliberately left as separate work.

## Capabilities with no agent file

These resolve to a non-Claude provider or to the host, and are never dispatched as Claude subagents.

| capability | provider | how it runs |
|---|---|---|
| `orchestrator` | claude | the Claude Code main session itself |
| `reviewer` | codex | dispatched internally by `bin/sotp review local` |
| `dry-checker` | codex | invoked by the `sotp dry` CLI |
| `pr-reviewer` | codex | fail-closes unless the provider supports structured PR review output |
| `ref-verifier-chain1` / `ref-verifier-chain2` | codex | invoked by the reference-verification pipeline |
| `obligation-fulfillment-verifier` / `waiver-verifier` | codex | invoked by the obligation pipeline |

## Dispatch rule

Never invoke these agents directly through the Agent tool. Direct invocation bypasses provider and model resolution. The canonical route is `bin/sotp capability exec <capability> --host <host> --briefing-file <path>`.

`review-fix-lead` is the exception: dispatch it through `cargo make track-local-review-fix`, which resolves the profile and, on a Claude resolution, emits a subagent-dispatch sentinel for the caller to act on. `cargo make track-local-dry-fix` has no such sentinel — it executes the codex provider directly and fails closed on any other resolution, which is why `dry-fix-lead` stays on codex.

`.harness/config/agent-profiles.json` is the routing SSoT. `.harness/config/samples/agent-profiles.*.json` hold alternative provider mixes.
