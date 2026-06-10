---
description: Run the SoT Chain semantic reference verification for the current track — bin/sotp ref-verify run with the firing-surface context, then report the verdict lane result.
---

Canonical command for the **Chain①/Chain② semantic reference verification lane**. A thin wrapper around `bin/sotp ref-verify run`: every firing surface (phase, commit gate, standalone) calls the same CLI command — no per-surface reimplementation (D9 of `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md`).

This command is an **independent verdict lane** (SoT Chain layer ③: the semantic verification layer that asks "does the cited evidence actually support the claim?", distinct from layer ① presence signals and layer ② structural freshness checks): it is separate from `/track:review` (code review, OS-06 / AC-11) and from the `sotp verify *` presence checks (layer ②, AC-11). It does not read or write `review.json`.

Requires being on a `track/<id>` branch. If on any other branch, stop and instruct the user to switch first.

## Arguments

Optional context argument: `spec-design` / `type-design <layer>` / `commit-gate` / `standalone` (default: `standalone`).

## Step 1: Run the semantic review

Map the argument onto the CLI invocation:

- no argument → `bin/sotp ref-verify run --context standalone`
- `spec-design` → `bin/sotp ref-verify run --context spec-design` (Chain① only: spec → ADR, judged with `.harness/prompts/ref-verifier-chain1.md`)
- `type-design <layer>` → `bin/sotp ref-verify run --context type-design --layer <layer>` (Chain② for that layer: catalogue → spec, judged with `.harness/prompts/ref-verifier-chain2.md`)
- `commit-gate` → `bin/sotp ref-verify run --context commit-gate` (both chains, all layers)

The CLI resolves the scope from the typed context (IN-12), re-reviews only pairs whose `(claim_hash, evidence_hash)` changed (differential cache, AC-07), and skips calibration probes when no production pair misses the cache (D12). Run the command in the foreground — the next step interprets the exit status and `[OK]`/`[BLOCKED]`/`[ESCALATE]` output, so the command must complete before Step 2 is evaluated.

## Step 2: Interpret the outcome

The run ends in exactly ONE of three mutually-exclusive outcomes:

- **`[OK]` (exit 0)** — every production pair has a Pass verdict; the verify-cache artifacts are up to date.
- **`[BLOCKED]` (SemanticFailuresConfirmed)** — final-tier evaluation confirmed production Fail pair(s). This is a writer/fix-loop outcome: the cited claim does not hold against its evidence. Identify the failing pair(s) from the relevant verify-cache artifact (`spec-adr-verify-cache.json` for Chain①, `<layer>-catalogue-spec-verify-cache.json` for Chain②) and route the fix to the owning writer (spec-designer for spec text, type-designer for catalogue entries, adr-editor for ADR decisions). Do NOT edit verify-cache artifacts by hand.
- **`[ESCALATE]` (HumanEscalationRequired)** — Pending verdicts remain after the final tier, or known-bad probe calibration fell below the detection threshold (verifier degradation). Report to the user; do not retry in a loop.

## Behavior

After execution, summarize:

1. The context/scope that ran and the outcome (`OK` / `BLOCKED` / `ESCALATE`).
2. For `BLOCKED`: the failing pair(s) with their `reason`, and the recommended writer re-entry.
3. For `ESCALATE`: whether it is Pending pairs or probe-calibration failure.

`/track:ref-verify` runs the semantic lane only. Sequencing against other gates (review / DRY) is owned by the caller (`/track:plan` phase orchestration, the commit gate chain in `Makefile.toml`, or the user).
