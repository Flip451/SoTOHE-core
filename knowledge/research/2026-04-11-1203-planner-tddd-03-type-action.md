# TDDD-03 Planner Design Review

**Date**: 2026-04-11
**Capability**: planner (Claude Opus)
**Feature**: TDDD-03 Type Action Declarations

## Key Design Decisions

1. **TypeAction** is a plain enum (Add, Modify, Reference, Delete) — enum-first per 04-coding-principles.md
2. **evaluate_single** branches on action=Delete BEFORE kind dispatch — delete logic is kind-agnostic
3. **ConsistencyReport** gains two new fields:
   - `contradictions: Vec<ActionContradiction>` — advisory warnings
   - `delete_errors: Vec<String>` — hard errors for delete of non-existent baseline type
4. **Codec** uses `serde(default)` + `skip_serializing_if` so that files missing the `action` field parse without error (field defaults to `Add`). Note: this is not a backward-compatibility commitment for pre-TDDD-03 track files — see ADR §Consequences.
5. **Render** adds Action column to domain-types.md table

## Data Flows Affected

```
domain-types.json → decode → DomainTypeEntry[] (with action)
  → evaluate_domain_type_signals → DomainTypeSignal[] (delete inversion)
  → check_consistency → ConsistencyReport (with contradictions, delete_errors)
    → verify.rs: findings ← contradictions (warn) + delete_errors (error)
    → signals.rs: output ← contradictions (WARN) + delete_errors (ERROR)
  → render_domain_types → domain-types.md (action column)
```

## Edge Cases

- EC-1: delete + typestate transitions_to → allow (developer's concern)
- EC-2: delete + trait_port expected_methods → accept without validation
- EC-3: Group 3 suppression — already correct (delete entries in declared set)
- EC-4: reference partial methods — acceptable nuance
- EC-5: delete Yellow in non-strict — accepted limitation
- EC-6: typestate_names() for delete entries — leave as-is
- EC-7: Duplicate same-name entries — codec relaxes uniqueness to allow exactly one `delete`+`add` pair per name (kind migration); 3+ entries or `delete`+`delete` / `add`+`add` pairs remain errors (T006)
- EC-8: Update TDDD-03 comment in check_consistency
