---
description: Design domain types for the current track (TDDD workflow).
---

Canonical command for TDDD (Type-Definition-Driven Development) type design.

Creates or updates the per-layer type catalogue files for the current track
by analyzing the plan and existing code, then declaring the types that need
to exist. TDDD is now multilayer (T007): each `layers[]` entry in
`architecture-rules.json` may opt in with a `tddd.enabled: true` block, in
which case its catalogue file (default `<layer>-types.json`) is designed,
captured, and evaluated independently. **Phase 1** wires the `domain` layer
only. `sotp track type-signals` rejects non-`domain` `--layer` values with
an explicit error. `baseline-capture` accepts `--layer` for forward
compatibility but always captures `domain-types-baseline.json` in Phase 1.
Additional layers are fully wired in Phase 2.

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
1. Determine the `TypeDefinitionKind`:
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

3. Determine the `action` field. The authority for whether a type "pre-exists" is:
   - If `domain-types-baseline.json` already exists: a type pre-exists if it is in the baseline
   - If no baseline exists yet (first run): a type pre-exists if it currently exists in the domain crate code
   - Note: `"delete"`, `"modify"`, and `"reference"` are validated against `domain-types-baseline.json` at Step 4; using them for types not in the baseline triggers contradiction warnings or errors

   Using "pre-exists" as defined above:
   - Omit or use `"add"` for types that do NOT pre-exist (default — omitted from JSON on encode)
   - `"modify"` when changing an existing type's structure — type must pre-exist
   - `"reference"` when declaring an existing type for documentation purposes only — type must pre-exist
   - `"delete"` when intentionally removing an existing type from the codebase — type must pre-exist
   - For cross-partition kind migration (non-trait ↔ trait, e.g., `value_object` → `trait_port`):
     if the type pre-exists, use two entries with the same name — one with `action: "delete"` (old
     kind) and one with `action: "add"` (new kind). The delete entry turns Blue when the type
     disappears from the old partition, and the add entry turns Blue when the new code is present.
     If the type does NOT pre-exist, use a single entry with the new `kind` and keep `action: "add"`.
   - For same-partition kind migration (within non-trait kinds, e.g., `value_object` → `enum`;
     or within trait kinds): update `kind` in place. Do NOT use a delete+add pair — the delete
     forward check looks up the type by name within the same partition, and the entry will stay
     Yellow as long as any type with that name exists in that partition. Use `"modify"` if the
     type pre-exists; otherwise keep `"add"` (omitted).

4. For each type, declare:
   - `name`: PascalCase Rust type name
   - `kind`: one of the TypeDefinitionKind values above
   - `action`: (optional) one of `add`, `modify`, `reference`, `delete` — omit for `add`
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
    },
    {
      "name": "OldType",
      "kind": "value_object",
      "action": "delete",
      "description": "Intentionally deleted type",
      "approved": true
    }
  ]
}
```

Note: `action` defaults to `"add"` when omitted. Only `"delete"`, `"modify"`, and `"reference"` need explicit declaration.
Note: The `action` field applies to tracks started after TDDD-03 (ADR 0003). Migration of existing pre-TDDD-03 `domain-types.json` files is explicitly out of scope per ADR 0003 §Consequences.

If the file already exists, merge new types with existing ones:
- Preserve existing entries that are still in the plan, except during cross-partition kind migrations (see below)
- Update changed fields (`action`, `description`, `expected_variants`, `transitions_to`, `expected_methods`) for types whose design has evolved. When a type's `kind` changes:
  - Cross-partition migration (non-trait ↔ trait): if the type pre-exists, REPLACE the old single entry with a `delete` + `add` pair (one entry with `action: "delete"` for the old kind, one with `action: "add"` for the new kind). The old entry must not be preserved alongside the pair — the codec rejects any duplicate name that is not exactly one delete + one add pair. If the type does NOT pre-exist, update the entry in place (new `kind`, keep `action: "add"`).
  - Same-partition migration (within non-trait kinds or within trait kinds): update `kind` in place and set `action: "modify"` only if the type pre-exists; otherwise keep `action: "add"` (omitted). Do NOT use a delete+add pair. Also remove any kind-specific fields that no longer apply (e.g., remove `transitions_to` when changing from `typestate` to `enum`).
- Add new entries for types not yet declared
- Remove entries for types no longer in the plan (with user confirmation)
- Do not modify `approved` status of existing entries
- Clear the `signals` field (omit or set to `null`) — Step 4 runs `type-signals` which always does a full rebuild

## Step 4: Capture baseline and validate

1. Run `cargo make track-baseline-capture -- <id>` to snapshot the current TypeGraph
   as `domain-types-baseline.json`. **Phase 1** always captures the `domain` layer
   only; `--layer` is accepted for forward compatibility but silently ignored.
   The baseline file name is derived from the layer's catalogue file as
   `<catalogue-stem>-baseline.json` (e.g., `domain-types-baseline.json` for
   the default `domain-types.json` catalogue). Additional layers are captured
   individually in Phase 2 via `bin/sotp track baseline-capture <id> --layer <layer_id>`.
   This baseline enables the reverse signal check to distinguish existing-unchanged types (skip)
   from structurally-changed or newly-added types (Red). The capture is idempotent — if a baseline
   already exists it is skipped (use `--force` to regenerate).
2. Run `sotp track type-signals <id>` to evaluate signals. Phase 1 processes the
   `domain` layer only; `--layer domain` is the only accepted value until Phase 2
   wires additional layers.
3. Run `sotp verify spec-states <spec-path>` — the Stage 2 loop evaluates every enabled layer's
   catalogue file and aggregates findings. A Red signal on any layer blocks.
4. Run `cargo make ci` to verify everything passes.
5. Print a summary:
   - Total types declared
   - Breakdown by kind (value_object, enum, typestate, error_type, trait_port)
   - New types added (if incremental update)
   - Types removed (if any)

## Step 5: Next steps guidance

After design completion, inform the user:
- Commit `domain-types.json` and `domain-types-baseline.json` together as design artifacts.
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
