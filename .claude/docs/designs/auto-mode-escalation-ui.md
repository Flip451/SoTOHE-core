# Auto Mode Escalation UI Design

> Defines the `/track:auto` CLI interface, escalation/resume flow,
> progress display, and error handling.

## 1. CLI Interface

```
/track:auto [track-id]                              # Start auto mode for all tasks
/track:auto [track-id] --task T001                   # Process a single task only
/track:auto --resume <run-id> --decision "..."       # Resume from escalation
/track:auto --status [track-id]                      # Show current auto-mode state
/track:auto --abort [run-id]                         # Abort an in-progress auto run
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `track-id` | Optional | Track ID. Auto-detected from branch if omitted. |
| `--task <id>` | Optional | Process only this task (single-task mode). |
| `--resume <run-id>` | For resume | UUID from auto-state.json to resume. |
| `--decision "..."` | With --resume | Human's decision text for the escalated choice. |
| `--status` | Optional | Display current auto-mode state without executing. |
| `--abort` | Optional | Abort an in-progress run and clean up state. |
| `--dry-run` | Optional | Simulate execution without committing. |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All tasks completed successfully |
| 1 | Escalation — human intervention needed (auto-state.json written) |
| 2 | CI failure — fixable, re-run after manual fix |
| 3 | Abort — user requested abort |
| 4 | Fatal error — unexpected failure |

## 2. Escalation Flow

### Trigger Conditions

1. **Reviewer rollback count exceeds threshold**: `max_consecutive_rollbacks` (default 3) consecutive rollbacks without progress
2. **Phase timeout**: Phase exceeds its time budget and `escalation_on_timeout` is true
3. **Context overflow**: Task scope is too large for single-pass processing
4. **Conflicting findings**: Reviewers disagree on direction (e.g., one says "add trait bound", another says "remove trait bound")
5. **External dependency**: Task requires a decision about external crate, API, or architecture not covered by spec

### Escalation Sequence

```
1. Auto-mode detects escalation trigger
2. Collect current context:
   - Current phase and round
   - Recent findings (last 2 rounds)
   - Files modified so far
   - Decision options (generated from findings)
3. Write auto-state.json with escalation info
4. Print escalation summary to terminal:
   ┌─────────────────────────────────────────────┐
   │ ESCALATION: Human decision required          │
   │                                               │
   │ Track: auto-mode-design-2026-03-16           │
   │ Task:  T003 — Implement repository trait     │
   │ Phase: TypeReview (round 3)                  │
   │ Run:   a1b2c3d4-e5f6-7890-abcd-ef1234567890 │
   │                                               │
   │ Reason: Conflicting trait bounds              │
   │                                               │
   │ Options:                                      │
   │   1. Make callback async + Send+Sync         │
   │   2. Use channel-based escalation            │
   │   3. Remove Send+Sync requirement            │
   │                                               │
   │ Resume with:                                  │
   │   /track:auto --resume a1b2c3d4 --decision   │
   │     "Option 2: use channels"                  │
   └─────────────────────────────────────────────┘
5. Exit with code 1
```

### Resume Sequence

```
1. User runs: /track:auto --resume <run-id> --decision "Option 2: use channels"
2. Load auto-state.json, verify run_id matches
3. Record decision in auto-state.json
4. Return to the escalated phase with the decision injected into context
5. Continue the cycle from where it left off
```

## 3. Progress Display

During execution, auto-mode displays a compact status line:

```
[auto] T003/T007 | TypeReview | Round 2 | 0 findings | 4m23s
```

Format: `[auto] {current_task}/{total_tasks} | {phase} | Round {n} | {findings} findings | {elapsed}`

### Phase Transitions

```
[auto] T003 → Plan (round 1)
[auto] T003 → PlanReview (round 1) ✓ zero_findings
[auto] T003 → TypeDesign (round 1)
[auto] T003 → TypeReview (round 1) ✗ 2 findings (P2: type change needed)
[auto] T003 → TypeDesign (round 2) ← rollback
[auto] T003 → TypeReview (round 2) ✓ zero_findings
[auto] T003 → Implement (round 2)
[auto] T003 → CodeReview (round 2) ✓ zero_findings
[auto] T003 → Committed ✓ (abc1234)
```

## 4. Error Handling

| Error Type | Behavior |
|-----------|----------|
| Network/API error | Retry with exponential backoff (1s, 2s, 4s), max 3 retries |
| CI failure (`cargo make ci`) | Stop, report error, exit code 2 (user can fix and re-run) |
| Reviewer timeout | If `escalation_on_timeout`: escalate. Otherwise: retry once. |
| Context overflow | Split task into subtasks, escalate with suggested split |
| Reviewer verdict extraction failure | Retry once. If still fails, escalate. (Provider-agnostic: applies to any reviewer backend.) |
| File conflict | Stop, report conflicting files, exit code 2 |

## 5. State Lifecycle

```
                    /track:auto start
                         │
                    ┌─────▼──────┐
                    │  CREATED   │ auto-state.json written
                    └─────┬──────┘
                          │
                    ┌─────▼──────┐
                    │  RUNNING   │ updated after each phase transition
                    └──┬──────┬──┘
                       │      │
              ┌────────▼┐  ┌──▼────────┐
              │ESCALATED│  │ COMPLETED │
              │(exit 1) │  │           │
              └────┬────┘  └─────┬─────┘
                   │             │
              ┌────▼────┐  ┌────▼─────┐
              │ RESUMED │  │ CLEANED  │ auto-state.json deleted
              │(--resume)│  └──────────┘
              └────┬────┘
                   │
              ┌────▼────┐
              │ RUNNING │ continues from escalated phase
              └─────────┘

              /track:auto --abort
                   │
              ┌────▼────┐
              │ ABORTED │ auto-state.json deleted, no task state changes
              └─────────┘
```

### Cleanup Rules

- **Successful completion**: Delete auto-state.json after all tasks are committed
- **Escalation**: Preserve auto-state.json for --resume
- **Abort**: Delete auto-state.json, leave metadata.json task states as-is (in_progress tasks remain in_progress for manual resolution)
- **Stale state**: On --resume, warn if auto-state.json is older than 24 hours or if track metadata has changed since escalation
