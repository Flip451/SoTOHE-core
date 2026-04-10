---
description: Design domain types for the current track (TDDD workflow).
---

Canonical command for TDDD (Type-Definition-Driven Development) type design.

Creates or updates `domain-types.json` for the current track by analyzing
the plan and existing code, then declaring the types that need to exist.

Arguments: none. The current branch (`track/<id>` or `plan/<id>`) determines the target track.

## Step 0: Resolve track

- Extract the track ID from the current git branch (`track/<id>` or `plan/<id>`).
  If the branch matches neither pattern, stop and instruct the user to switch first.
- Read `track/items/<id>/spec.md` if it exists, otherwise read `track/items/<id>/plan.md`.
- Read `track/items/<id>/metadata.json` for task definitions.

## Step 1: Gather context

- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `track/items/<id>/spec.md` (or `plan.md` if `spec.md` does not exist).
- Read `.claude/rules/04-coding-principles.md` for type design patterns
  (enum-first, typestate, hybrid decision table).
- If `track/items/<id>/domain-types.json` exists, read it for incremental update.
- If domain crate code exists, consider current types for alignment.
- Read `knowledge/DESIGN.md` for existing architecture context.
- Check `knowledge/adr/` for ADRs governing this feature. If an ADR exists, cross-validate type design decisions against ADR constraints (layer placement, type choices, rejected alternatives).

## Step 2: Design domain types

Using the designer capability (resolved via `.harness/config/agent-profiles.json`):

For each type needed by the plan:
1. Determine the `DomainTypeKind`:
   - `value_object`: validated wrapper around a primitive
   - `enum`: finite set of variants with `expected_variants`
   - `typestate`: state machine with `transitions_to` (empty array = terminal, non-empty = target state names)
   - `error_type`: error enum with `expected_variants`
   - `trait_port`: trait with `expected_methods`

2. Apply the decision table from `04-coding-principles.md`:
   - No state transitions (finite value set)? -> enum
   - Any state transition (even minimal)? -> typestate + transition functions (preferred)
   - State-dependent data + transitions? -> typestate + enum-first state types
   - Serde/persistence needed? -> domain: typestate, infra: serde enum DTO conversion

3. For each type, declare:
   - `name`: PascalCase Rust type name
   - `kind`: one of the DomainTypeKind values above
   - `description`: one-line English description
   - `approved`: `true` (human-reviewed design)
   - Kind-specific fields (expected_variants, transitions_to, expected_methods)

## Step 3: Write domain-types.json

Write the designed types to `track/items/<id>/domain-types.json`:

```json
{
  "schema_version": 1,
  "domain_types": [
    {
      "name": "TypeName",
      "kind": "value_object",
      "description": "One-line description",
      "approved": true
    }
  ]
}
```

If the file already exists, merge new types with existing ones:
- Preserve existing entries that are still in the plan
- Update changed fields (`kind`, `description`, `expected_variants`, `transitions_to`, `expected_methods`) for types whose design has evolved. When a type's `kind` changes, remove kind-specific fields that no longer apply (e.g., remove `transitions_to` when changing from `typestate` to `value_object`)
- Add new entries for types not yet declared
- Remove entries for types no longer in the plan (with user confirmation)
- Do not modify `approved` status of existing entries
- Clear the `signals` field (omit or set to `null`) — Step 4 runs `domain-type-signals` which always does a full rebuild

## Step 4: Validate and summarize

1. Run `sotp track domain-type-signals <id>` to evaluate signals for the new/updated types.
2. Run `sotp verify spec-states <spec-path>` to verify the TDDD gate passes (no Red signals).
3. Run `cargo make ci` to verify everything passes.
3. Print a summary:
   - Total types declared
   - Breakdown by kind (value_object, enum, typestate, error_type, trait_port)
   - New types added (if incremental update)
   - Types removed (if any)

## Step 5: Next steps guidance

After design completion, inform the user:
- If the track is planning-only (branchless, `status=planned`, `branch=null`), run `/track:activate <track-id>` first.
- Then run `/track:implement` to start implementing the declared types.
- Types will start as Yellow (defined but not yet implemented).
- As implementation proceeds, signals will turn Blue.
- `verify spec-states` (default mode) blocks ALL Red signals — both forward (declared but mismatched) and reverse (undeclared in spec). Yellow WIP is allowed for interim commits.
- For merge (`--strict` mode), all signals must be Blue (Yellow also blocks).

## Behavior

- Present the type design to the user for review before writing.
- If the user requests changes, iterate until approved.
- All type names and descriptions must be in English.
- Keep the design minimal — only declare types that the plan requires.
