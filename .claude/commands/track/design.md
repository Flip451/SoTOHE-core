---
description: Design types for the current track across all TDDD-enabled layers (TDDD workflow).
---

Canonical command for TDDD (Type-Definition-Driven Development) type design.

Creates or updates the per-layer type catalogue files for the current track by analyzing the plan and existing code, then declaring the types that need to exist. TDDD is multilayer: each `layers[]` entry in `architecture-rules.json` may opt in with a `tddd.enabled: true` block, in which case its catalogue file (default `<layer>-types.json`) is designed, captured, and evaluated independently.

The 13 `TypeDefinitionKind` variants are available for all enabled layers: `typestate`, `enum`, `value_object`, `error_type`, `secondary_port`, `application_service`, `use_case`, `interactor`, `dto`, `command`, `query`, `factory`, `secondary_adapter`. See `knowledge/adr/` for the ADR that records the full taxonomy and the rationale behind each variant.

Arguments:

- `--layer <layer_id>` (optional): restrict processing to a single layer (e.g., `domain`, `usecase`). The `<layer_id>` must be `tddd.enabled = true` in `architecture-rules.json`. When omitted, all enabled layers are processed in `layers[]` order.

The current branch (`track/<id>` or `plan/<id>`) determines the target track.

## Step 0: Resolve track and layer scope

- Extract the track ID from the current git branch (`track/<id>` or `plan/<id>`). If the branch matches neither pattern, stop and instruct the user to switch first.
- Read `track/items/<id>/spec.json` if it exists (the authoritative behavioral contract per the SoT Chain ① → ② → ③). Fall back to `track/items/<id>/spec.md` (rendered view) if `spec.json` is absent; fall back further to `track/items/<id>/plan.md` only when neither spec artifact exists. The goal is to feed the type-designer the JSON SSoT when available.
- Read `track/items/<id>/metadata.json` for identity fields (track name, description, branch). Note: task definitions are in `impl-plan.json` (Phase 3), which does not exist yet at Phase 2 design time.
- Read `architecture-rules.json` and call `parse_tddd_layers` (or equivalent) to enumerate `tddd.enabled = true` layers. If `architecture-rules.json` is absent (file not found), fall back to a single synthetic `domain` binding so legacy tracks continue to work; any other I/O error is fatal.
- If `--layer <id>` is provided: validate that `<id>` is `tddd.enabled = true`. If not, stop with an error. Otherwise, restrict processing to that single layer.
- If `--layer` is omitted: process every enabled layer in `layers[]` order (typically `domain` first, then `usecase`, then any future enabled layers). If no layers have `tddd.enabled = true`, the command fails with an error — it does not silently succeed with an empty loop.
- For each layer to process, derive:
  - `catalogue_file` from the layer's `tddd.catalogue_file` (default `<crate>-types.json`)
  - `baseline_file` as `<catalogue-stem>-baseline.json`
  - `rendered_file` as `<catalogue-stem>.md`

## Step 1: Gather context (once, shared across layers)

- Read every convention file referenced by the spec. The field/section layout depends on the artifact type:
  - `track/items/<id>/spec.json` (preferred, SSoT): read the `related_conventions[]` JSON array. Each entry is either a path string or a `ConventionRef` struct (`{file, anchor}` — `file` is the convention path).
  - `track/items/<id>/spec.md` (rendered view, fallback when `spec.json` is absent): read the `## Related Conventions (Required Reading)` Markdown section.
  - `track/items/<id>/plan.md` (legacy tracks only): read the `## Related Conventions (Required Reading)` Markdown section — same layout as `spec.md` for legacy compatibility.
- Read `.claude/rules/04-coding-principles.md` for type design patterns (enum-first, typestate, hybrid decision table).
- Read `knowledge/DESIGN.md` for existing architecture context.
- Check `knowledge/adr/` for ADRs governing this feature. If an ADR exists, cross-validate type design decisions against ADR constraints (layer placement, type choices, rejected alternatives).
- **ADR pre-check**: If no ADR covers this track's design decisions, stop and instruct the user to author an ADR first. Design-phase artefacts (type catalogue entries) reference the spec layer via `spec_refs[]`; direct `AdrRef` from the type catalogue is a SoT Chain layer skip and is forbidden. See `knowledge/conventions/pre-track-adr-authoring.md`.
- Read the TDDD taxonomy ADRs under `knowledge/adr/` for the 13 `TypeDefinitionKind` variants and their intended usage (look up by grepping the ADR index for `tddd-taxonomy` to find the base taxonomy, and for `secondary-adapter` to find the 13th variant addition).

For each layer to process, additionally:

- If `track/items/<id>/<catalogue_file>` exists, read it for incremental update.
- If the layer's crate code exists, consider current types for alignment.

## Step 2: Design types per layer

Using the type-designer capability (resolved via `.harness/config/agent-profiles.json`):

For each layer in the processing scope, and for each type needed by the plan at that layer:

1. Determine the `TypeDefinitionKind`:
   - `value_object`: validated wrapper around a primitive or small struct
   - `enum`: finite set of variants with `expected_variants`
   - `typestate`: state machine with `transitions_to` (empty array = terminal, non-empty = target state names)
   - `error_type`: error enum with `expected_variants`
   - `secondary_port`: secondary/driven port trait with `expected_methods` (infrastructure implements)
   - `application_service`: primary/driving port trait with `expected_methods` (external actor drives)
   - `use_case`: struct-only use case, no trait abstraction (existence check only)
   - `interactor`: struct implementing an `application_service` trait (Clean Architecture; existence check only)
   - `dto`: pure data container crossing layer boundaries (existence check only)
   - `command`: CQRS command object (existence check only)
   - `query`: CQRS query object (existence check only)
   - `factory`: aggregate/entity factory struct (existence check only)
   - `secondary_adapter`: struct implementing a `secondary_port` trait — use in the infrastructure layer with `implements: Vec<TraitImplDecl>` (existence check with L1 method-signature validation)

2. Apply the decision table from `04-coding-principles.md`:
   - No state transitions (finite value set)? → enum
   - Any state transition (even minimal)? → typestate + transition functions (preferred)
   - State-dependent data + transitions? → typestate + enum-first state types
   - Serde/persistence needed? → keep typestate in the layer owning the domain logic; convert to/from a serde-compatible DTO in the persistence/adapter layer (e.g., domain: typestate, infrastructure: serde enum DTO)

3. Determine the `action` field. The authority for whether a type "pre-exists" is:
   - If `<baseline_file>` already exists: a type pre-exists if it is in the baseline
   - If no baseline exists yet (first run): a type pre-exists if it currently exists in the layer's crate code
   - Note: `"delete"`, `"modify"`, and `"reference"` are validated against `<baseline_file>` at Step 4; using them for types not in the baseline triggers contradiction warnings or errors

   Using "pre-exists" as defined above:
   - Omit or use `"add"` for types that do NOT pre-exist (default — omitted from JSON on encode)
   - `"modify"` when changing an existing type's structure — type must pre-exist
   - `"reference"` when declaring an existing type for documentation purposes only — type must pre-exist
   - `"delete"` when intentionally removing an existing type from the codebase — type must pre-exist
   - For cross-partition kind migration (non-trait ↔ trait, e.g., `value_object` → `secondary_port`): if the type pre-exists, use two entries with the same name — one with `action: "delete"` (old kind) and one with `action: "add"` (new kind). The delete entry turns Blue when the type disappears from the old partition, and the add entry turns Blue when the new code is present. If the type does NOT pre-exist, use a single entry with the new `kind` and keep `action: "add"`.
   - For same-partition kind migration (within non-trait kinds, e.g., `value_object` → `enum`; or within trait kinds, e.g., `secondary_port` → `application_service`): update `kind` in place. Do NOT use a delete+add pair — the delete forward check looks up the type by name within the same partition, and the entry will stay Yellow as long as any type with that name exists in that partition. Use `"modify"` if the type pre-exists; otherwise keep `"add"` (omitted).

4. For each type, declare:
   - `name`: PascalCase Rust type name
   - `kind`: one of the 13 `TypeDefinitionKind` values above
   - `action`: (optional) one of `add`, `modify`, `reference`, `delete` — omit for `add`
   - `description`: one-line English description
   - `approved`: `true` — required catalogue schema field (`TypeCatalogueEntry::approved` in Rust); marks this entry as human-authored rather than tool-generated. This is a schema discriminant, not a workflow approval ceremony
   - Kind-specific fields:
     - `expected_variants` for `enum` / `error_type`
     - `transitions_to` for `typestate`
     - `expected_methods` for `secondary_port` / `application_service`
     - `implements` for `secondary_adapter` — `Vec<{ trait_name, expected_methods? }>` declaring which `secondary_port` traits this adapter implements
     - (other struct-only variants carry no extra fields)

## Step 3: Write `{layer}-types.json`

For each layer in the processing scope, write the designed types to `track/items/<id>/<catalogue_file>` (e.g., `domain-types.json`, `usecase-types.json`):

```json
{
  "schema_version": 2,
  "type_definitions": [
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

If the file already exists, merge new types with existing ones:

- Preserve existing entries that are still in the plan, except during cross-partition kind migrations (see below)
- Update changed fields (`action`, `description`, `expected_variants`, `transitions_to`, `expected_methods`) for types whose design has evolved. When a type's `kind` changes:
  - Cross-partition migration (non-trait ↔ trait): if the type pre-exists, REPLACE the old single entry with a `delete` + `add` pair (one entry with `action: "delete"` for the old kind, one with `action: "add"` for the new kind). The old entry must not be preserved alongside the pair — the codec rejects any duplicate name that is not exactly one delete + one add pair. If the type does NOT pre-exist, update the entry in place (new `kind`, keep `action: "add"`).
  - Same-partition migration (within non-trait kinds or within trait kinds): update `kind` in place and set `action: "modify"` only if the type pre-exists; otherwise keep `action: "add"` (omitted). Do NOT use a delete+add pair. Also remove any kind-specific fields that no longer apply (e.g., remove `transitions_to` when changing from `typestate` to `enum`).
- Add new entries for types not yet declared
- Remove entries for types no longer in the plan (with user confirmation)
- Do not modify the `approved` field of existing entries (it is a schema discriminant set at authoring time)
- Clear the `signals` field (omit or set to `null`) — Step 4 runs `type-signals` which always does a full rebuild

## Step 4: Capture baseline and validate per layer

For each layer in the processing scope, run the per-layer capture and evaluation:

1. Run `sotp track baseline-capture <id> [--layer <layer_id>]` to snapshot the current TypeGraph as `<catalogue-stem>-baseline.json`. When `--layer` is omitted here, the command iterates all enabled layers in order — equivalent to calling it once per layer. The baseline filename is derived from the layer's `catalogue_file` (e.g., `domain-types-baseline.json`, `usecase-types-baseline.json`). The capture is always idempotent — if a baseline already exists it is preserved. Re-capturing mid-implementation would overwrite the pre-implementation snapshot and collapse signal semantics, so there is no `--force` flag; delete the stale baseline file manually if a genuine re-capture is required.
2. Run `sotp track type-signals <id> [--layer <layer_id>]` to evaluate signals for the layer. When `--layer` is omitted, the command iterates all enabled layers. Per-layer results are recorded in each layer's `<catalogue_file>` and rendered as `<rendered_file>` (both derived from Step 0).
3. Compose and display a per-layer summary (this is Claude Code's own output, not the `type-signals` CLI output). Head each block with the layer identifier so multi-layer runs are unambiguous:
   - Layer: `<layer_id>` (e.g., `## domain` or `## usecase`)
   - Total types declared per layer
   - Breakdown by kind (13 variants)
   - New types added (if incremental update)
   - Types removed (if any)
   - Signal counts (blue / yellow / red)

After all layers are processed:

4. If `track/items/<id>/spec.json` exists, run `sotp verify spec-states track/items/<id>/spec.md` — the Stage 2 loop evaluates every enabled layer's catalogue file and aggregates findings; a Red signal on any layer blocks. (Requires `architecture-rules.json`.) When `spec.json` is absent, skip this step — Stage 2 TDDD signal checking is not available without it.
5. Run `cargo make ci` to verify everything passes.

## Step 5: Next steps guidance

After design completion, inform the user:

- Commit `<catalogue_file>`, `<baseline_file>`, and `<rendered_file>` for each processed layer together as design artifacts (all three names derived from Step 0).
- If the track is planning-only (branchless, `status=planned`, `branch=null`), run `/track:activate <track-id>` first.
- Then run `/track:implement` to start implementing the declared types.
- Types will start as Yellow (defined but not yet implemented).
- As implementation proceeds, signals will turn Blue.
- `verify spec-states` (default mode) blocks ALL Red signals — both forward (declared but mismatched) and reverse (undeclared in spec). Yellow WIP is allowed for interim commits.
- For merge (`--strict` mode), all signals must be Blue (Yellow also blocks).

## Behavior

- Present the type design to the user for review before writing — per layer when multiple layers are being processed.
- If the user requests changes, iterate until the user accepts the design.
- All type names and descriptions must be in English.
- Keep the design minimal — only declare types that the plan requires.
- When processing multiple layers in one invocation, stop at the first layer that the user rejects and wait for guidance before continuing.
