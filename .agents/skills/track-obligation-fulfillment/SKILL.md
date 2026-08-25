---
name: track-obligation-fulfillment
description: Use when Codex is asked to author or repair a track's test-obligation bindings — drives the derive → skeleton → triangulated authoring → totality → repair loop toward a green `sotp test-obligation check`, with `evaluate` reserved for the orchestrator host.
---

# Track-Obligation-Fulfillment (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/obligation-fulfillment.md` —
the provider-agnostic workflow contract for this skill (timing, sequence, gates, cache
semantics, failure recovery). Per-record authoring discipline (record forms, canonical waiver
shapes, the declaration × anchor triangulation rule) lives in
`.harness/capabilities/implementer.md` Step 3. Do not duplicate either document here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-obligation-fulfillment` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file. The dispatch
  prompt then only needs the round-specific deltas (scope, current lane counts, arbitration
  notes) — never a restatement of the methodology.

### (2) Sandbox constraints

- Authoring/repair rounds require `--sandbox workspace-write` (edits `test-bindings.json`
  and test code).
- **Never run `bin/sotp test-obligation evaluate` from inside this skill**: it spawns
  provider verifier subprocesses that cannot initialize inside a provider sandbox. The
  orchestrator host runs `evaluate` between rounds. `derive` (on-branch), `check`,
  `results`, and `bindings-skeleton` are sandbox-safe.
- Do not run `git add` / `git commit` / `git push` or any git write operation.

### (3) File-safety discipline (mandatory)

- Follow the workflow SSoT's file-safety discipline (Step 3 of
  `.harness/workflows/track/obligation-fulfillment.md`) verbatim for every editing round.
- Codex-specific detail: scratch backups and scratch copies live under `tmp/` (the
  sandbox-writable scratch root).

### (4) Gate waiting

- Each implementer round and each orchestrator-host `bin/sotp test-obligation evaluate` is run as
  one blocking call whose result is read once; `evaluate` is a synchronous repair step, never a
  background or fire-and-forget launch, and `check` — not `evaluate` — is what the commit gate
  runs. Do not poll for round completion; if the host backgrounds a call, read the result once
  after the single completion notification.

### (5) Reporting format

- Report per-round: records repaired by method (bind-existing / new-test / waiver-convert),
  tests added (file + test name), and any untouched remainder as exact
  `entry_key × anchor` pairs.
- Final line, exactly one of:
  `OBLIGATION_FULFILLMENT_STATUS: completed` / `blocked` / `failed`.
