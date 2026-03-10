---
description: Run the autonomous implementation full-cycle for a track task.
---

Canonical wrapper for autonomous implementation in this template.

Arguments:
- Use `$ARGUMENTS` as the task summary.
- If empty, ask for a short task summary and stop.

Execution:
- Run:
  `cargo make takt-full-cycle "$ARGUMENTS"`
- If `$ARGUMENTS` matches `docs/external-guides.json` `trigger_keywords`, rely on the injected guide summaries before opening cached raw documents.

Behavior:
- This is equivalent to terminal `cargo make takt-full-cycle "<task>"`.
- After execution, summarize:
  1. Result (success/failure)
  2. Key outputs or blockers
  3. Next recommended action
