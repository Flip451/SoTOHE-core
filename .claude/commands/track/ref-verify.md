---
description: Run the SoT Chain semantic reference verification for the current track — bin/sotp ref-verify run (scope derived from track artifact existence), then report the verdict lane result.
---

Canonical command for the **Chain①/Chain② semantic reference verification lane**. A thin wrapper around `bin/sotp ref-verify run`: every firing surface (phase tail, commit gate, standalone) calls the same CLI command — no per-surface reimplementation and no firing-surface arguments.

This command is an **independent verdict lane** (SoT Chain layer ③: the semantic verification layer that asks "does the cited evidence actually support the claim?", distinct from layer ① presence signals and layer ② structural freshness checks): it is separate from `/track:review` (code review) and from the `sotp verify *` presence checks (layer ②). It does not read or write `review.json`.

Requires being on a `track/<id>` branch. If on any other branch, stop and instruct the user to switch first.

## Arguments

None. The verification scope is derived from which SoT artifacts exist in the track directory ("file existence = phase state"):

- `spec.json` absent → Chain① has zero pairs (the CLI prints a `[SKIP]` reason line)
- all layer catalogues absent → Chain② has zero pairs
- both present → both chains, all layers

Fail-closed states are rejected before any verification runs: a partial catalogue set, a catalogue present while `spec.json` is absent (SoT Chain ordering violation), and present-but-unparseable artifacts all end the run with an error instead of silently skipping pairs.

## Step 1: Run the semantic review

```bash
bin/sotp ref-verify run
```

The CLI re-reviews only pairs whose `(claim_hash, evidence_hash)` changed (differential cache) and skips calibration probes when no production pair misses the cache. Run the command in the foreground — the next step interprets the exit status and `[OK]`/`[BLOCKED]`/`[ESCALATE]` output, so the command must complete before Step 2 is evaluated.

## Step 2: Interpret the outcome

The run ends in exactly ONE of three mutually-exclusive outcomes:

- **`[OK]` (exit 0)** — every production pair has a Pass verdict; the verify-cache artifacts are up to date. A pre-Phase-1 track (no `spec.json` yet) passes here with zero pairs after printing the `[SKIP]` reason.
- **`[BLOCKED]` (SemanticFailuresConfirmed)** — final-tier evaluation confirmed production Fail pair(s). This is a writer/fix-loop outcome: the cited claim does not hold against its evidence. Run `bin/sotp ref-verify results` to identify failing and pending pairs. Each fail / pending record shows the `claim_origin` (which artifact made the claim) and `evidence_origin` (which artifact provided the evidence), enabling routing to the correct writer (`spec-designer` for spec text, `type-designer` for catalogue entries, `adr-editor` for ADR decisions) without reading the verify-cache JSON files directly. Do NOT edit verify-cache artifacts by hand.
- **`[ESCALATE]` (HumanEscalationRequired)** — Pending verdicts remain after the final tier, or known-bad probe calibration fell below the detection threshold (verifier degradation). Report to the user; do not retry in a loop.

## Behavior

After execution, summarize:

1. The derived scope that ran (which chains contributed pairs) and the outcome (`OK` / `BLOCKED` / `ESCALATE`).
2. For `BLOCKED`: the failing pair(s) with their `reason`, and the recommended writer re-entry.
3. For `ESCALATE`: whether it is Pending pairs or probe-calibration failure.

`/track:ref-verify` runs the semantic lane only. Sequencing against other gates (review / DRY) is owned by the caller (`/track:plan` phase orchestration, the commit gate chain in `Makefile.toml`, or the user).
