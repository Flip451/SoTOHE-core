# Planner Briefing — TDDD-01 Multilayer Extension (Step 3)

## Task

Perform a design review for the TDDD-01 implementation track. The full design
has already been accepted in ADR `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md`.
Your job is NOT to redesign; it is to:

1. Enumerate every data-flow and every caller/callee that the planned changes
   touch, so the resulting task plan covers all of them.
2. Identify edge cases that are not obvious from the ADR text and that would
   otherwise surface as review findings 10 rounds into the fix-review loop.
3. Decide concrete type shapes for the new domain types (`MethodDeclaration`,
   `ParamDeclaration`, `MemberDeclaration`) following
   `.claude/rules/04-coding-principles.md`.
4. Propose a sub-task breakdown with commit sizes ≤500 lines each, respecting
   the dependency order in ADR §Phase 1 (1 → 2 → 3 → 4 → 5 → 6 → 7).

Return your response in a structured markdown document with a dedicated
`## Canonical Blocks` section. Blocks in that section will be copied verbatim
into `plan.md` / `DESIGN.md`; all other text is summarised in Japanese by the
orchestrator. Use English for the Canonical Blocks and technical content.

## Scope (authoritative: ADR 0002)

Phase 1 from the ADR covers four streams. They must land in a single track
because they are mutually dependent:

- **3a. Rename**: `DomainTypeKind`/`DomainTypeEntry`/`DomainTypesDocument`/
  `DomainTypeSignal`/`evaluate_domain_type_signals`/`check_domain_types_signals`
  → layer-neutral names. Files: `libs/domain/src/tddd/catalogue.rs` (currently
  2088 lines — DM-06 requires splitting into `catalogue.rs` / `signals.rs` /
  `consistency.rs` at the same time; see `knowledge/strategy/TODO.md` DM-06).
  Also rename `infrastructure::tddd::catalogue_codec`, `domain_types_render`,
  `apps/cli/src/commands/track/tddd/{signals,baseline}.rs`, CLI subcommands,
  all usecase imports (`merge_gate.rs`, `task_completion.rs`), and the
  `merge_gate_adapter`. No v1 codec alias; one-shot rename.
- **3b. MethodDeclaration** (structural signature catalogue): Extend
  `expected_methods: Vec<String>` → `Vec<MethodDeclaration>` with
  `{ name, receiver, params: [{name, ty}], returns, async }`. Add forward-check
  logic (step 1–7 from ADR D2) and reverse-check (undeclared trait method
  detection).
- **3c. TypeGraph extension**: Add `MemberDeclaration` (field name + type),
  extend `FunctionInfo` with `{ params, returns, receiver, is_async }`
  (~~keep the existing `signature: String` for human readability~~ — **[C1 accepted: `signature: String` is deleted; BRIDGE-01 breaking change accepted; human-readable form provided by `MethodDeclaration::signature_string()`]**), and
  extend `TypeNode::methods: Vec<MethodDeclaration>` /
  `TraitNode::methods: Vec<MethodDeclaration>`. `build_type_graph`
  becomes the single conversion point.
- **3d. Multilayer**: Add optional `tddd` block to each `layers[]` entry in
  `architecture-rules.json`. `sotp track type-signals` and
  `sotp track baseline-capture` accept `--layer <layer>`. `sotp verify
  spec-states` aggregates every enabled layer's catalogue (AND-merge). The
  `/track:design` skill discovers enabled layers dynamically.

Out of scope (deferred to Phase 2):

- L2 generics/bounds fields
- Cross-layer reference validation
- Multi-language support

## Codebase orientation (already read)

The orchestrator has already read every file below. Refer to them by path
rather than asking for them to be re-read:

**ADR / strategy**:
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` (SSoT — all D1–D6 sections)
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` (TDDD-02, completed — baseline was introduced here)
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` (TDDD-03, completed — `action` field was introduced here)
- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` (completed 2026-04-12; this is the direct upstream of the merge gate and CI Stage 2 paths)
- `knowledge/strategy/tddd-implementation-plan.md` (Step sequencing, 3a–3d breakdown)
- `knowledge/strategy/TODO.md` DM-06 (2088-line catalogue.rs split — co-execute with 3a)

**Rules**:
- `.claude/rules/04-coding-principles.md` (enum-first, typestate, hybrid decision table)
- `.claude/rules/05-testing.md` (TDD workflow)
- `.claude/rules/08-orchestration.md` (Planner Briefing Requirements — provider-agnostic)

**Current implementation — domain layer**:
- `libs/domain/src/tddd/mod.rs` — only exposes `baseline` and `catalogue` (the 2088-line file)
- `libs/domain/src/tddd/catalogue.rs` — `TypeAction`, `DomainTypeKind`, `DomainTypeEntry`, `DomainTypeSignal`, `DomainTypesDocument`, `evaluate_domain_type_signals`, `undeclared_to_signals`, `ActionContradiction`, `ActionContradictionKind`, `ConsistencyReport`, `check_consistency`, `check_domain_types_signals`
- `libs/domain/src/tddd/baseline.rs` — `TypeBaselineEntry { kind, members: Vec<String>, method_return_types: Vec<String> }`, `TraitBaselineEntry { methods: Vec<String> }`, `TypeBaseline { schema_version, captured_at, types, traits }`
- `libs/domain/src/schema.rs` — `SchemaExport`, `TypeKind (Struct/Enum/TypeAlias)`, `TypeInfo`, `FunctionInfo { name, signature: String, docs, return_type_names: Vec<String>, has_self_receiver: bool }`, `TraitInfo { name, docs, methods: Vec<FunctionInfo> }` (already has full method info!), `ImplInfo`, `SchemaExporter` port, `TypeGraph`, `TypeNode { kind, members: Vec<String>, method_return_types: HashSet<String>, outgoing: HashSet<String>, module_path }`, `TraitNode { method_names: Vec<String> }` (discards method info!)
- `libs/domain/src/lib.rs` — re-exports every symbol above. Rename must update this file.
- `libs/domain/src/spec.rs` and `libs/domain/src/signal.rs` — related but not in scope. `check_spec_doc_signals` is in `spec.rs`.

**Current implementation — infrastructure layer**:
- `libs/infrastructure/src/tddd/mod.rs`
- `libs/infrastructure/src/tddd/catalogue_codec.rs` — serde DTO with internally-tagged `kind`, `TypeActionDto`, delete+add pair validation, transitions_to integrity, `schema_version: 1` only (no v0 alias)
- `libs/infrastructure/src/tddd/baseline_codec.rs` — `BaselineDto` with required `types`/`traits`, duplicate-key rejecting deserializer, schema_version 1
- `libs/infrastructure/src/tddd/baseline_builder.rs` — `build_baseline(graph, captured_at) -> TypeBaseline`
- `libs/infrastructure/src/schema_export.rs` — `RustdocSchemaExporter`. Already parameterized by `crate_name`. `extract_return_type_names`, `format_sig`, `extract_methods`, etc. operate per item.
- `libs/infrastructure/src/code_profile_builder.rs` — `build_type_graph(&SchemaExport, &HashSet<String> /* typestate names */) -> TypeGraph`. Discards method info when building `TraitNode`. Uses `last_segment` to normalize impl targets.
- `libs/infrastructure/src/domain_types_render.rs` — markdown renderer; section header is `## Domain Types`, table columns are `Name | Kind | Action | Details | Signal`.
- `libs/infrastructure/src/verify/spec_states.rs` — `verify_from_spec_json(spec_path, strict, trusted_root)`. Reads sibling `domain-types.json` from `spec_path.parent()`. TDDD opt-in: Stage 2 NotFound → skip. Symlink-guarded.
- `libs/infrastructure/src/verify/merge_gate_adapter.rs` — `GitShowTrackBlobReader`. Reads `domain-types.json` via `git show origin/<branch>:track/items/<track_id>/domain-types.json`.

**Current implementation — usecase layer**:
- `libs/usecase/src/merge_gate.rs` — `TrackBlobReader` port with `read_domain_types_document`. `check_strict_merge_gate` delegates to `check_domain_types_signals`. 18 test matrix U1–U18.
- `libs/usecase/src/task_completion.rs` — Uses the same `TrackBlobReader` port (`read_track_metadata`).

**Current implementation — CLI layer**:
- `apps/cli/src/commands/track/tddd/mod.rs` — exposes `baseline` and `signals`
- `apps/cli/src/commands/track/tddd/signals.rs` — `sotp track domain-type-signals <id>`; calls `exporter.export("domain")` (HARDCODED), reads `domain-types.json` and `domain-types-baseline.json` by fixed name, renders `domain-types.md`
- `apps/cli/src/commands/track/tddd/baseline.rs` — `sotp track baseline-capture <id>`; same hardcoded `exporter.export("domain")` and fixed baseline filename
- `apps/cli/src/commands/verify.rs` — `sotp verify spec-states <path> [--strict]`
- `apps/cli/src/commands/track/mod.rs` — subcommand dispatch

**Architecture rules**:
- `architecture-rules.json` — currently has `layers[]` for domain/usecase/infrastructure/cli but NO `tddd` block. Version 2.
- `Makefile.toml` — `cargo make track-baseline-capture`, `track-type-signals`, etc. may exist (orchestrator has not enumerated them)

**Skill definitions**:
- `.claude/commands/track/design.md` — `/track:design`. Currently single-layer. Needs dynamic layer discovery + loop.
- `.claude/skills/track-plan/SKILL.md` — this workflow

## Key ADR decisions that must guide your plan

1. **D1**: `architecture-rules.json` layers get an optional `tddd { enabled, catalogue_file, schema_export: { method, targets: [...] } }` block. `targets` is an array to allow 1 layer = many crates. Default `catalogue_file` = `<crate>-types.json`.
2. **D2**: L1 signature JSON schema is `{ name, receiver ("&self"/"&mut self"/"self"/null), params: [{name, ty}], returns, async }`. Module paths use last-segment short names; generics preserved verbatim. Forward check is strict equality, fuzziness excluded.
3. **D3**: All `Domain*` type names become `TypeDefinition*` / `TypeCatalogue*`. No v1 alias; one-shot rename.
4. **D4**: No Kind restriction per layer inside TDDD core. `forbidden_kinds` lint lives elsewhere.
5. **D5**: Phase 1 does NOT cross-layer validate. Cargo + L1 cover the real damage.
6. **D6**: rustdoc JSON is the only evaluator basis; syn is JSON-design reference only.
7. **TypeGraph extension table** (ADR Phase 1-4): `TypeInfo::members` becomes `Vec<MemberDeclaration>`; `FunctionInfo` gains `params`/`returns`/`receiver`/`is_async`; `TypeNode::method_return_types` is replaced by `methods: Vec<MethodDeclaration>` (~~outgoing is now derived from methods filtered by typestate names~~ — **[Q4 accepted: `FunctionInfo::return_type_names` is NOT deleted; `outgoing` continues to use `return_type_names ∩ typestate_names`; `MethodDeclaration::returns` strings are NOT reparsed]**); `TraitNode::method_names` becomes `methods: Vec<MethodDeclaration>`.
8. **Baseline schema migration**: `TypeBaselineEntry`/`TraitBaselineEntry` gain structured members/methods. Per the ADR, no migration — existing baselines become invalid and the new baseline is regenerated on next `baseline-capture`. Old tracks are untouched. This implies the baseline `schema_version` must bump so that attempting to read an old baseline fails loudly rather than silently accepting incompatible v1 data.
9. **Strict signal gate v2 compatibility**: The merge gate currently calls `check_domain_types_signals(&dt_doc, /* strict */ true)` once per track. In a multilayer world the gate must aggregate results across every `tddd.enabled` layer. `TrackBlobReader::read_domain_types_document` must evolve (or be replaced) to return a collection keyed by layer. The symlink guard in `spec_states.rs` must apply to every layer's catalogue.

## Questions that the ADR leaves open — you MUST decide

1. **Catalogue file split structure** (DM-06): Given that `catalogue.rs` is 2088 lines AND the rename is touching almost every line, should the split be:
   - `libs/domain/src/tddd/catalogue.rs` — types only (`TypeDefinitionKind`, `TypeCatalogueEntry`, `TypeCatalogueDocument`, `TypeAction`, `TypestateTransitions`, `MethodDeclaration`, `ParamDeclaration`, `MemberDeclaration`)
   - `libs/domain/src/tddd/signals.rs` — `TypeSignal`, `evaluate_type_signals`, per-kind evaluators
   - `libs/domain/src/tddd/consistency.rs` — `ConsistencyReport`, `ActionContradiction`, `ActionContradictionKind`, `check_consistency`, `check_type_signals` (the signal-gate)
   - or a different split? Recommend one and explain the rationale.
2. **[RESOLVED — ADR C1 — 2026-04-12] MethodDeclaration vs `signature: String`**: `signature: String` is **deleted**. Human-readable form is `MethodDeclaration::signature_string()`. BRIDGE-01 breaking change accepted (no consumer of `FunctionInfo::signature()` found). This question is fully closed — do NOT propose keeping `signature` as a field or backwards-compatibility shim.
3. **MemberDeclaration**: Currently `TypeInfo::members: Vec<String>` holds variant names (enum) or field names (struct) without type info. Is the rename safe — do Enum catalogues use only variant names (no payload types) today, and do ValueObject/struct checks rely on field names? Propose the type:
   - `struct MemberDeclaration { name: String, ty: Option<String> }` with `ty = None` for enum variants and `Some(...)` for struct fields; or
   - `enum MemberDeclaration { Variant(String), Field { name, ty } }` (enum-first style)
4. **[RESOLVED — Q4 — 2026-04-12] Typestate transitions**: `TypeNode::outgoing` continues to use `FunctionInfo::return_type_names ∩ typestate_names`. `FunctionInfo::return_type_names` is NOT deleted; `MethodDeclaration::returns` strings are NOT reparsed for outgoing derivation. ~~With structured methods the outgoing derivation changes to `methods.iter().filter(|m| typestate_names.contains(parse_typestate_target(&m.returns))).map(...)`. How do you parse "Result<Option<User>, DomainError>" to extract the typestate target? Propose: either (a) keep the existing string-based return_type_names hack for typestate detection only, or (b) introduce a helper `fn first_return_type_name(s: &str) -> &str` that returns the outermost type name.~~
5. **Layer loop in verify**: `verify_from_spec_json` currently reads a single sibling `domain-types.json`. In multilayer mode it must:
   - Discover layers from `architecture-rules.json`
   - For each `tddd.enabled` layer, read `<catalogue_file>` from the same track dir
   - AND-merge the findings
   Does this imply that `architecture-rules.json` needs a trusted path resolution (relative to the workspace root)? Where does `verify_from_spec_json` load it from?
6. **`TrackBlobReader::read_domain_types_document` vs. layers**: The port reads a single document. Should we:
   - Add `read_type_catalogue(branch, track_id, layer) -> BlobFetchResult<TypeCatalogueDocument>` and loop in the usecase; or
   - Replace with `read_type_catalogues(branch, track_id) -> BlobFetchResult<HashMap<LayerId, TypeCatalogueDocument>>`?
   The former keeps the port stateless and testable per-layer; the latter is simpler for the CI path.
7. **Baseline schema version**: Bump to 2? Leave at 1? What error message do we return if a v1 baseline is encountered after the change?
8. **Baseline file naming**: `domain-types-baseline.json` → `<layer>-baseline.json`? `<crate>-types-baseline.json`? Align with `catalogue_file` naming convention.
9. **CLI invocation shape**: `sotp track type-signals <id> --layer domain`? Or `sotp track type-signals <id>` that iterates every enabled layer and writes per-layer output? Or both?
10. **`/track:design` ordering across layers**: For multiple layers, does the developer design each layer sequentially, or is there a single design session that outputs all layers? Propose a workflow.
11. **Rename surface in `.claude/commands/track/design.md`**: The command text mentions `DomainTypeKind`, `domain-types.json`, `domain-types-baseline.json`. These must all be updated. Describe the update strategy (single PR, interleave with rename commit?).
12. **Merge gate contract drift risk**: The strict signal gate v2 test matrix (U1–U18) asserts specific behavior against a single `domain-types.json`. Does the test matrix need to grow to cover multilayer? Outline the new test matrix dimensions (layer count × Blue/Yellow/Red × NotFound/FetchError).

## Edge cases to enumerate

Minimum list (you may add more):

- All `tddd.enabled: false` — existing opt-out behavior preserved (TDDD not active globally)
- Some layers enabled, others not — verify aggregates only the enabled subset
- A layer has `tddd.enabled: true` but no catalogue file in the track — is that "TDDD not active for this layer" (skip) or "misconfiguration" (error)?
- Parallel `baseline-capture` for multiple layers — each layer's baseline is independent, but a single CLI invocation should succeed atomically or not at all
- `catalogue_file` mixed with `<layer>-baseline.json`: if the catalogue rename drifts from the baseline rename, tracks may end up with `usecase-types.json` + `domain-types-baseline.json` in the same directory. Prevent by rule.
- Symlink guard coverage: must fire for every layer's catalogue + baseline path, anchored at `trusted_root`
- `strict-signal-gate-v2` U3 (spec=Blue, dt=Yellow → BLOCKED): with multilayer, if ANY layer has a Yellow declared signal the whole gate blocks
- Delete+add pair + MethodDeclaration: The existing codec rejects same-partition same-name pairs. With MethodDeclaration added, the "partition" definition stays the same (trait_port vs non-trait_port), but the forward check for TraitPort now uses structural methods. Confirm that `Delete` action still works on trait ports (it inverts a `profile.get_trait()` presence check — unaffected by method signatures)
- Typestate signal evaluation: `outgoing` derivation from structured methods. If a method returns `Result<Foo, Bar>`, do we classify the target as `Foo` or `Result`? How is this consistent with the current `return_type_names` which returns `["Foo"]` by stripping Result?
- ActionContradiction for `Reference`: Requires the forward signal to be Blue. With MethodDeclaration, Blue requires strict signature match. A Reference entry that used to be Blue (name-only match) may drop to Yellow. Is this acceptable as a deliberate behavior change, or do we add a migration note?
- CI performance: Multi-layer means multiple `cargo +nightly rustdoc` invocations. Note in Consequences the CI time increase and propose caching.
- Cross-layer name collisions: `TypeGraph` is built per-crate. Two crates can have the same type name (`Error`). The current `last_segment` + `module_path` system partially handles this within a single layer. Across layers the signal evaluation stays per-layer (no global aggregation), so collisions are naturally isolated.

## Deliverable format

Return a markdown document with these top-level sections in this order:

1. `## Understanding Summary` — a short paragraph proving you understood the scope
2. `## Impact Surface` — every file that must change, grouped by sub-task 3a/3b/3c/3d. Use repo-relative paths.
3. `## Type Design` — concrete Rust signatures for `MethodDeclaration`, `ParamDeclaration`, `MemberDeclaration`, and the updated `FunctionInfo`/`TypeNode`/`TraitNode`/`TypeBaselineEntry`/`TraitBaselineEntry`. Follow `.claude/rules/04-coding-principles.md` (enum-first / typestate decision table). Justify struct vs enum choices.
4. `## JSON Schema` — the `tddd` block for `architecture-rules.json`, the catalogue JSON schema for L1, and the updated baseline JSON schema.
5. `## Data-flow Diagrams` — for each affected data flow (catalogue decode → evaluate → encode; rustdoc → schema → TypeGraph → baseline; verify spec-states; merge gate; track:design), show the multilayer-aware version.
6. `## Open Decisions` — your answers to the 12 questions above.
7. `## Edge Cases` — expanded from the list above; at least 5 new ones not listed.
8. `## Sub-task Plan` — ordered task list respecting the dependency order (1→2→3→4→5→6→7 from ADR §Phase 1). Each task:
   - a short imperative title
   - purpose
   - touched files (repo-relative)
   - red/green test expectations
   - estimated diff size in lines — aim for ≤500 per commit; split further if larger
   - direct link back to ADR section(s)
9. `## Canonical Blocks` — verbatim-embedded blocks for plan.md / DESIGN.md. Include at least:
   - `architecture-rules.json` `tddd` schema (complete JSON example)
   - L1 catalogue JSON example with a MethodDeclaration trait port
   - `MethodDeclaration` Rust struct (domain layer)
   - new `TypeNode` / `TraitNode` Rust structs
   - baseline JSON v2 example
   - migration instructions (delete old baseline, re-run baseline-capture)

Use real repo-relative paths throughout. Do not paraphrase type names or file paths — they are used to auto-generate task descriptions.

Respond in English (Canonical Blocks must be English; surrounding analysis may
be in Japanese but English is preferred for cross-provider compatibility).
