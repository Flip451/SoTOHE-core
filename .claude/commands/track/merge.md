---
description: Wait for PR CI checks to pass, then merge.
---

> Operational SSoT: `.harness/workflows/track/merge.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:merge`. `$ARGUMENTS` supplies the PR number and, optionally, a merge method appended after a space (e.g. `123 squash`). When `$ARGUMENTS` is empty, resolve the PR for the current branch via `gh pr view --json number -q .number` before delegating to the workflow.

**IMPORTANT — do NOT auto-select the merge method**: when `$ARGUMENTS` does not include a method, omit `--method` entirely so the workflow resolves the configured default from the PR's track `branch_strategy_snapshot.merge_method`. Do NOT substitute `squash`, `rebase`, or `merge` based on prior knowledge of how other projects merge.

## Claude Code invocation constraints

- Bash wrappers used:
  - `gh pr view --json number -q .number` (only when `$ARGUMENTS` is empty)
  - `gh pr view <pr_number> --json headRefName,headRefOid` (workflow Step 0 / pre-invocation
    and post-outcome head-OID verification reads)
  - `bin/sotp pr wait-and-merge <pr_number>` (method omitted → configured default)
  - `bin/sotp pr wait-and-merge <pr_number> --method <method>` (only when the user explicitly supplied a method)
- **Head-OID verification**: perform the workflow SSoT's pre-invocation and post-outcome
  head-OID checks around the wrapper call. Once the head-bound wrapper form ships, it becomes
  the only authorized invocation and replaces the unbound forms above.
- **Strict-gate block handling**: route every strict merge-signal gate block from the merge
  command through the workflow SSoT's block-triage lane. Enter adjudication recovery only when
  that lane directs it; this adapter does not restate or perform either lane.

## Report format

After execution, summarize:

1. PR number and URL.
2. Final check status (all passed / specific failing checks / pending on timeout), or a
   head-OID verification outcome (pre-invocation abort for a fresh audit, or an
   audit-invalidating incident surfaced to the user).
3. Adjudication outcomes, when the recovery lane ran (adopted / rejected drafts, audit
   verdicts, resulting commits).
4. Merge result (success with resolved method and resulting commit, or failure reason).
5. Recommended next command (`/track:done` on success).
