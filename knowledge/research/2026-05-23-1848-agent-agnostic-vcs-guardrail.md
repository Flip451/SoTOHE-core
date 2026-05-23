# Research: Agent-Agnostic VCS Operation Guardrail (2026-05-23)

Researcher: Gemini (researcher capability). Curated by orchestrator.

## Motivation

The current direct-VCS-write block (`block-direct-git-ops`) is a **Claude Code
PreToolUse hook** — it only intercepts Claude Code's own tool calls. External actors
(Codex CLI in `--sandbox workspace-write`, future agents, humans on the terminal)
bypass it. We want a **provider-agnostic** mechanism that enforces the policy
regardless of which process invokes Git. This research informs a prospective ADR.

> Verification status (2026-05-23):
> - `prek` — VERIFIED real (`github.com/j178/prek`). MIT license. Latest v0.4.1
>   (2026-05-20) — **pre-1.0**. Rust-native single binary, drop-in
>   `.pre-commit-config.yaml`-compatible. ~7.7k stars; adopted by CPython, FastAPI,
>   Apache Airflow, Godot. Caveat: some languages not yet full drop-in parity. Install:
>   prebuilt binary / `cargo install` / Homebrew / npm. → Credible but pre-1.0; for a
>   security-critical guardrail, weigh against `lefthook` (Go, 1.0+, mature) and native
>   `core.hooksPath`.
> - **Orchestrator read after verification**: this repo already has `bin/sotp` (custom
>   Rust CLI) + `cargo make bootstrap`. Hook bodies only need to delegate to
>   `bin/sotp hook dispatch ...`. That is a *thin* need — native `core.hooksPath` →
>   committed `.githooks/` (zero new deps, leverages existing sotp) is likely the best
>   fit; prek/lefthook add value mainly when orchestrating many heterogeneous hooks,
>   which this project may not require. Decide in planning.
> - STILL TO CONFIRM: process-tree parent check robustness across the Codex sandbox /
>   container boundary.

## 1. Shipping/installing Git hooks repo-wide

| Tool | Language | Activation | Rust fit | Notes |
|---|---|---|---|---|
| `core.hooksPath` → committed dir | native | manual `git config` (bootstrap) | low (raw scripts) | stable, zero deps, but raw shell |
| `lefthook` | Go binary | manual `install` | high (fast, parallel) | mature, monorepo-proven |
| `prek` | Rust | binary install | extreme | VERIFIED real, MIT, v0.4.1 (pre-1.0); `.pre-commit-config.yaml`-compatible |
| `pre-commit` | Python | manual `install` | low (needs Python) | legacy standard, slow env setup; project is moving away from Python |
| `cargo-husky` | Rust | on compile | high | Cargo-integrated; less multi-hook flexibility |

Recommendation (Gemini): `prek` if it verifies; otherwise `lefthook` or native
`core.hooksPath` driven by a `cargo make bootstrap` step that runs the install. Hook
bodies should delegate to `bin/sotp` (single source of enforcement logic), not raw shell.

## 2. Coverage gaps and mitigations

- **Staging gap**: blocking the stage operation is impossible via native hooks, but
  **does not matter** — the changeset-record (commit) is the real gate. Staged changes
  with no record are harmless and local.
- **`--no-verify` bypass**: skips local pre-record/pre-publish hooks entirely. The local
  hook is therefore a *convenience/fast-feedback gate*, not a hard wall. The true
  invariant (CI + review passed) must live on the **server-side backstop** which no
  local flag can skip.

## 3. Distinguishing authorized wrapper from rogue caller

| Pattern | Mechanism | Robustness |
|---|---|---|
| Sentinel env var | `SOTP_AUTH=1` set by wrapper | moderate (spoofable if known) |
| Ephemeral file | wrapper writes a UUID to `.git/<token>` | high (needs file access to spoof) |
| Signed token | wrapper mints short-lived HMAC/JWT | extreme (enterprise monorepo pattern) |

Recommended: **env sentinel + process-tree check** — the wrapper sets an authorized
token; the pre-record hook (in Rust) verifies it AND confirms its parent process is
`sotp` (via `sysinfo`/`procfs`). Raises the bar above a plain env var.

## 4. GitHub server-side backstop (the true guarantee)

Use **GitHub Repository Rulesets** (modern branch protection):
- Require PR — disallow direct publish to the default branch.
- Require status checks — server re-runs/re-verifies CI.
- Metadata restrictions — commit-message pattern (e.g. Track ID), verified committer.
- Bypass list — only a review/deploy app; no human or general-purpose coding agent.

This is the only layer that cannot be bypassed by any local actor (catches
`--no-verify` escapes, direct records, and locally-deleted hooks at publish time).

## 5. Constraining external coding agents from VCS writes

> **VERIFIED 2026-05-23 — Codex's workspace-write sandbox already does this for free.**
> OpenAI Codex docs ("Protected paths in writable roots",
> developers.openai.com/codex/agent-approvals-security): in `workspace-write` mode,
> `.git` (and `.agents` / `.codex`) is **protected read-only, recursively, with NO
> config key to override** — documented intended behavior. So a Codex fixer launched in
> workspace-write **physically cannot mutate `.git`** (commit/stage/branch all fail with
> "Read-only file system"); `--no-verify` is moot (the block is below git, at the FS).
> Confirmed enforced on Linux (issue #15505, Manjaro, v0.115.0+); the project runs on
> WSL2/Linux. Windows is reportedly NOT protected (issue #9313) — not our platform.
> `.git` stays READABLE, so the fixer can still run `git status` / `git diff`.
> Constraints to keep the guarantee: launch with `--sandbox workspace-write` (or
> `--full-auto`, which implies workspace-write); NEVER disable the sandbox
> (`danger-full-access` / bypass flags); pin + smoke-test the Codex version.
>
> **EMPIRICALLY CONFIRMED on this machine (2026-05-23, Codex CLI v0.125.0, WSL2/Linux):**
> `codex exec --sandbox workspace-write -m gpt-5.5` (approval: never) was told to create
> a scratch file and stage it. Result: workspace file creation SUCCEEDED (`apply_patch`),
> but `git add` FAILED with exit 128 —
> `fatal: Unable to create '.../.git/index.lock': Read-only file system`. `git status`
> (read) succeeded and showed the file still untracked. Independently verified the file
> was never staged. → The `.git` read-only protection is real and active here; the fixer
> can edit source but cannot stage/commit. (Sandbox writable roots reported by Codex:
> `[workdir, /tmp, $TMPDIR, ~/.codex/memories]`.)

For an autonomous fixer (Codex workspace-write) that should ONLY edit files, in
defense-in-depth order:
0. **Codex workspace-write sandbox (primary — built-in, structural)**: `.git` read-only
   by default removes all local git-write capability. No extra infra. See box above.
1. **Credential isolation (defense-in-depth)**: do not give the agent `GITHUB_TOKEN`/SSH.
   Even if `.git` protection regressed, no push credentials = cannot publish.
2. **Binary shadowing**: a `git` shim earlier on the agent's PATH. Largely redundant
   given (0); fragile to absolute-path invocation.
3. **Read-only `.git` via container/user isolation**: only needed if NOT relying on
   Codex's own sandbox (e.g. a non-Codex agent) — heavier infra.

## 6. Layered recommendation for this repo

| Layer | Component | Implementation |
|---|---|---|
| 1 Gatekeeper | wrapper `bin/sotp` | sets authorized token; runs CI / signals / review check before recording |
| 2 Local enforcer | pre-record hook | installed via `prek`/`lefthook`; re-verifies token + process-tree; fails on direct record |
| 3 Agent sandbox | credential isolation + PATH shadow | agent edits only; orchestrator records/publishes |
| 4 Final backstop | GitHub Rulesets | blocks any publish not cleared via PR + mandatory CI/review |

### Residual risks
1. **Manual hook deletion** by a privileged actor → caught by Rulesets at publish time.
2. **Env var spoofing** by a sophisticated agent → mitigated by process-tree parent check.
3. **`--no-verify` escape** → mitigated by unskippable server-side CI/review.

### Read for this project specifically
- The cleanest fix for the rfl-Codex case (fixer only edits, orchestrator commits) is
  **Layer 3 credential isolation** — it makes the fixer structurally unable to publish,
  independent of hooks. Layers 1/2/4 harden the general/local case.
- Keep the existing Claude Code `block-direct-git-ops` hook as fast feedback for Claude;
  the new layers are additive, not a replacement.
