---
adr_id: 2026-01-06-fixture-grandfathered
decisions:
  - id: D1
    status: accepted
    grandfathered: true
---
# Fixture: grandfathered decision (Blue per D4 exemption)

Test fixture for the grandfathered exemption path. `grandfathered: true`
classifies the decision as `DecisionGrounds::Grandfathered`, which the
usecase tally counts as Blue (D4: skipped by `verify-adr-signals`).
