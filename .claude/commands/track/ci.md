---
description: Run the standard CI checks for the current track.
---

Canonical command for validation before review or commit.

Execution:
- Run:
  `cargo make ci`

Behavior:
- After execution, summarize:
  1. Pass/fail result
  2. Failing check names, if any
  3. Whether track artifacts (`spec.md`, `plan.md`, `metadata.json`) are complete
  4. Recommended next action
