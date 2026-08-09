## Limited rollout completion record

- Track: `2026-08-02-codex-reasoning-effort-max`
- Recorded after: all rollout assignments, applicable gates, retries, batches, and lifecycle tail
- Historical comparison: No historical A/B comparison is available or required.

The profile switch was active before the review-fix-lead assignments below. Each assignment
completed without an incomplete-output, timeout, or gate-failure outcome, so no Terra retry was
opened. Provider-reported credits were not emitted by the runner and are recorded as
`unavailable`. The runner does not persist provider-only start-to-verdict timing for the fixer
capability; elapsed values are therefore `unavailable` rather than inferred from enclosing command
wall time. The enclosing terminal commands did report end-to-end durations, but those include
pre-review gates and are not provider-only measurements.

| Assignment id / lane | Provider / effort | Completion-condition / quality result | Applicable gate results | Flags: incomplete-output; timeout; gate failure | Credits | Elapsed: start → recorded completion result or verdict | Executions (including Terra retry) | Terra retry result | Model-regression candidate | Evidence |
|---|---|---|---|---|---|---|---:|---|---|---|
| `review-fix-lead:harness-policy:fast` | `codex / max (gpt-5.6-luna)`; retry: not run | completed; fast review `zero_findings` | task-contract: pass; fast review: `zero_findings` | no; no; no | unavailable | unavailable | 1 | not run | unavailable | `review.json`; terminal capability result; committed profile |
| `review-fix-lead:infrastructure:fast` | `codex / max (gpt-5.6-luna)`; retry: not run | completed; fast review `zero_findings` | task-contract: pass; fast review: `zero_findings` | no; no; no | unavailable | unavailable | 1 | not run | unavailable | `review.json`; terminal capability result; committed profile |
| `review-fix-lead:impl-plan:fast` | `codex / max (gpt-5.6-luna)`; retry: not run | completed; fast review `zero_findings` | task-contract: pass; fast review: `zero_findings` | no; no; no | unavailable | unavailable | 1 | not run | unavailable | `review.json`; terminal capability result; committed profile |
| `review-fix-lead:harness-policy:final` | `codex / max (gpt-5.6-luna)`; retry: not run | completed; final review `zero_findings` | task-contract: pass; final review: `zero_findings` | no; no; no | unavailable | unavailable | 1 | not run | unavailable | `review.json`; terminal capability result; committed profile |
| `review-fix-lead:infrastructure:final` | `codex / max (gpt-5.6-luna)`; retry: not run | completed; final review `zero_findings` | task-contract: pass; final review: `zero_findings` | no; no; no | unavailable | unavailable | 1 | not run | unavailable | `review.json`; terminal capability result; committed profile |
| `review-fix-lead:impl-plan:final` | `codex / max (gpt-5.6-luna)`; retry: not run | completed; final review `zero_findings` | task-contract: pass; final review: `zero_findings` | no; no; no | unavailable | unavailable | 1 | not run | unavailable | `review.json` (`final@2026-08-02T12:00:21Z zero_findings`); terminal capability result; committed profile |
| `dry-fix-lead:DFP` | not run / not applicable | not run; DRY gate was already approved | dry check-approved: `APPROVED` | not run; not run; not run | unavailable | not applicable | 0 | not run | not applicable | `logs/telemetry.jsonl`; dry check-approved result |

The T002 implementation assignment itself ran before the profile switch under its prior Terra
profile, so it is not counted as a Luna rollout assignment. No historical Luna Max A/B comparison
is available or required.
