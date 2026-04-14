Now I have confirmed that `TypeDefinitionKind` has NO `UseCase` variant. The valid kinds are `typestate`, `enum`, `value_object`, `error_type`, `trait_port`. The briefing's reference to `use_case` kind is **not a valid catalogue kind**. UseCase structs must be catalogued as `value_object` (or omitted from the initial seed, and seeded only when they add structural value).

This is a critical finding. I now have all the information needed for the full design document.

---

## DESIGN DOCUMENT: tddd-02-usecase-wiring (2026-04-14)

---

### 1. Dataflow Analysis

#### DF-1: `sotp track type-signals <id> --layer usecase`

Step by step with file:line references (post-PR-#95 HEAD):

1. CLI routes to `execute_type_signals` (`signals.rs:75`). `layer = Some("usecase")`.
2. `domain::TrackId::try_new(&track_id)` — `signals.rs:82`.
3. `resolve_layers(&workspace_root, Some("usecase"))` — `signals.rs:89`. Reads `architecture-rules.json` (`signals.rs:34-36`), calls `parse_tddd_layers` (`signals.rs:37`), finds and returns `vec![usecase_binding]`. `resolve_layers` already handles arbitrary layer IDs — no change needed in this function.
4. **Phase 1 rejection block deleted (T003):** Lines `signals.rs:102-136` contain the `non_domain_enabled` filter, the `skipped_enabled_layers` tracking, and the `if filter != "domain"` fail-closed arm. All of this is replaced by a generic loop over the returned bindings.
5. After T003, `execute_type_signals` iterates `bindings` and calls `execute_type_signals_single(items_dir, track_id, workspace_root, &binding)` for each. The function is renamed to `execute_type_signals_for_layer` to drop the "single" qualifier; its signature is otherwise unchanged (takes `&TdddLayerBinding` already).
6. Inside `execute_type_signals_for_layer` (currently `execute_type_signals_single`):
   - Reads `track/items/<id>/usecase-types.json` via `binding.catalogue_file()` — `signals.rs:180-194`.
   - Calls `exporter.export(target)` where `target = binding.targets()[0]` — **currently hardcoded `"domain"` at `signals.rs:201`, replaced in T003 with** `binding.targets()[0]` (with a guard for `targets.is_empty()`). `RustdocSchemaExporter::export("usecase")` runs `cargo +nightly rustdoc -p usecase --lib -- -Z unstable-options --output-format json` (`schema_export.rs:57-73`). The artifact path becomes `target/doc/usecase.json` (`schema_export.rs:87`).
   - Builds `TypeGraph` (`signals.rs:214`), loads baseline from `usecase-types-baseline.json` (`signals.rs:220-234`).
   - `domain::check_consistency` — `signals.rs:267`. Domain logic is fully layer-agnostic; no change.
   - Writes `usecase-types.json` and `usecase-types.md` atomically (`signals.rs:347-358`).
7. `skipped_enabled_layers` logic at `signals.rs:157-164` is deleted entirely (see Section 4 decision).
8. Returns `ExitCode::SUCCESS` if all bindings processed without error.

#### DF-2: `sotp track baseline-capture <id> --layer usecase --force`

1. `execute_baseline_capture` (`baseline.rs:31`). `layer = Some("usecase")`.
2. **Phase 1 rejection block deleted (T004):** `baseline.rs:43-49`.
3. **`enforce_domain_tddd_enabled` replaced by `resolve_layer_binding` (T004):** `baseline.rs:57-58`. After T004, calls `resolve_layer_binding(&workspace_root, "usecase")` — reads `architecture-rules.json`, finds the `usecase` binding (which is `tddd.enabled=true` after T005). Returns `Err` if not found or not enabled.
4. `baseline_filename = binding.baseline_file()` = `"usecase-types-baseline.json"`. `catalogue_filename = binding.catalogue_file()` = `"usecase-types.json"`.
5. Symlink guards, track_dir existence check — unchanged.
6. `--force` skips the `baseline_path.is_file()` check.
7. Reads `usecase-types.json` for typestate names (optional, empty set if absent) — `baseline.rs:114-126`.
8. `exporter.export(binding.targets()[0])` = `exporter.export("usecase")` — **T004 replaces the hardcoded `"domain"` at `baseline.rs:130`**. No code change needed in `schema_export.rs` — `run_rustdoc` is fully parametric.
9. Builds `TypeBaseline`, encodes, writes `usecase-types-baseline.json` atomically — `baseline.rs:143-150`.
10. **`skipped_enabled_layers` logic at `baseline.rs:162-169` is deleted (T004).**

**Multi-target handling (Phase 2 design decision):** For Phase 2 of tddd-02, `targets` is always a single-element `["usecase"]` array. The loop `for target in binding.targets()` with a single element is functionally equivalent to the current hardcoded call. If `targets.is_empty()` at runtime (should not happen given `parse_tddd_layers` defaults), return `Err(CliError::Message("schema_export.targets is empty for layer '...'; cannot export schema"))`. Full multi-target merge (merging multiple `SchemaExport` results) is deferred to a future track when a project with `["domain-core", "domain-events"]` appears.

#### DF-3: `sotp verify spec-states` — no code change required

`check_strict_merge_gate` (`merge_gate.rs:119`) calls `reader.read_enabled_layers(branch)` at line 175. The real adapter (`merge_gate_adapter.rs`) reads `architecture-rules.json` from the PR branch blob. Once T005 flips `usecase.tddd.enabled = true`, `read_enabled_layers` returns `["domain", "usecase"]`. The existing loop at `merge_gate.rs:209-223` iterates and calls `read_type_catalogue(branch, track_id, "usecase")`. If `usecase-types.json` exists on the branch → `check_type_signals` with strict. **No code change in `merge_gate.rs`, `spec_states.rs`, or `merge_gate_adapter.rs` is needed.** T010 adds new tests.

#### DF-4: First-run signal distribution expectations

After seeding with 5 TraitPort entries and the minimum UseCase/error subset:

- **Forward check (5 TraitPorts):** Each method in `expected_methods` is checked against rustdoc export of the `usecase` crate. Since all 5 traits (`TrackBlobReader`, `HookHandler`, `Reviewer`, `DiffGetter`, `ReviewHasher`) are defined directly in the `usecase` crate and are `pub`, they will appear in `target/doc/usecase.json`. If the declared method signatures exactly match the actual signatures (which they will if the seed is authored correctly from the code), all 5 TraitPort entries → **Blue**.
- **Forward check (error_type entries):** `ReviewerError`, `DiffGetError`, `ReviewHasherError`, `ReviewCycleError` are all `pub enum` in `usecase/src/review_v2/error.rs`. If `expected_variants` matches the actual variants → **Blue**. On first seed, declare the actual variants → Blue immediately.
- **Forward check (value_object UseCase structs):** `SaveTrackUseCase<W>` etc. are `pub struct` in the usecase crate. TypeGraph will classify them as `TypeKind::Struct`. Forward check for `ValueObject` confirms the type is found → **Blue**. No variant/method checks.
- **Reverse check (undeclared):** Every `pub` type in the usecase crate NOT in the catalogue → Red "undeclared" signal. The usecase crate has dozens of types (structs, enums, error types). At first run, the catalogue seed covers only the declared subset; the rest become undeclared Reds. **This is expected and correct — the baseline captures the full codebase state, and the undeclared types are then absorbed into the baseline, suppressing their future Reds.** Specifically: run `baseline-capture --layer usecase --force` after seeding, which snapshots all current types. Subsequent `type-signals` runs then show only newly-added types as undeclared.

The minimum catalogue seed that proves end-to-end wiring: the 5 TraitPort entries. If all 5 turn Blue and the baseline captures undeclared types correctly, the wiring is confirmed.

---

### 2. Schema Impact

| Schema | Change | Backward compat | Migration needed |
|---|---|---|---|
| `architecture-rules.json` | `usecase.tddd` block: `enabled` flips `false` → `true`, adds `catalogue_file: "usecase-types.json"`, adds `schema_export: { method: "rustdoc", targets: ["usecase"] }` | Additive — v2 format, `tddd` block is optional. Existing tracks that do not read `usecase-types.json` see no change | None — existing domain-only tracks continue to work; new tracks get usecase gating automatically |
| `usecase-types.json` | New file created by seed task (T006) | N/A — new file | None |
| `usecase-types-baseline.json` | New file created by T007 (`baseline-capture --layer usecase`) | N/A — new file | None |
| `usecase-types.md` | New file generated by first `type-signals` run (T008) | N/A — new file | None |
| `domain-types.json` / `domain-types-baseline.json` | No change | — | None |
| `catalogue_codec` v2 (`schema_version: 2`) | No change to schema; `usecase-types.json` uses the identical codec | Same v2 codec | None |
| `baseline_codec` v2 | No change | — | None |
| `TypeSignal`, `ConsistencyReport`, domain types | No change | — | None |

---

### 3. Edge Cases Resolution

**EC-1: `usecase-types.json` exists but `usecase-types-baseline.json` does not.**

Decision: **Hard error, matching domain behavior.** `execute_type_signals_for_layer` reads the baseline at `signals.rs:222-234`; if `NotFound`, returns:
```
Err(CliError::Message(format!("{baseline_filename} not found for track '{track_id}'. Run `sotp track baseline-capture {track_id} --layer usecase` first.")))
```
The error message must be updated (T003) to include `--layer usecase` (currently says `sotp track baseline-capture {track_id}`). This is a targeted string change in the existing `NotFound` arm.

**EC-2: `--layer usecase` passed but `usecase.tddd.enabled = false`.**

Decision: **Fail-closed — error.** `resolve_layer_binding("usecase")` (or the generalized equivalent in `resolve_layers`) finds the usecase entry in `parse_tddd_layers`, but since `enabled = false`, it is excluded from the returned bindings. The `layer_filter` path at `signals.rs:51-55` returns:
```
Err(CliError::Message(format!("layer 'usecase' is not tddd.enabled in architecture-rules.json")))
```
This error message is already present in the current `resolve_layers` code. No change needed — it applies to any layer_id that is either absent or disabled.

**EC-3: `schema_export.targets = []` (empty array).**

Decision: **Parse-time rejection in `parse_tddd_layers`.** Currently `parse_tddd_layers` at `tddd_layers.rs:215-222` uses `filter(|t| !t.is_empty())` to detect an empty targets array and substitutes `[layer_id]`. This means an explicit `"targets": []` silently defaults to `[layer_id]` — the empty array is indistinguishable from the omitted case. This behavior is **correct and intentional** per the T007 contract ("An absent or empty `schema_export.targets` both default to `[layer_id]`"). No change needed. The empty-targets case is not an error; it means "use the layer id as the single target crate name."

**EC-4: Multi-target: same type name in two target crates.**

Decision: **Document as known limitation, deferred to tddd-03.** For Phase 2 of tddd-02, `targets` is always `["usecase"]` (single element). If multiple targets are specified in the future, and both crates export a type named `Foo`, `build_type_graph` will encounter a key collision in its `HashMap<String, TypeNode>`. The collision is currently silently overwritten (last-writer wins). This is the same underlying issue as the `Finding` collision in the domain layer (deferred to tddd-03 per briefing scope §B). The multi-target merge path is not implemented in this track; if `targets.len() > 1` is detected at export time, the implementation should log a warning and use only `targets[0]` for this Phase 2 track.

**EC-5: Cross-layer type references in forward check — `ReviewTarget` declared in usecase catalogue, but defined in domain.**

Decision: **By design, per ADR 0002 §D5.** The forward check for `TraitPort.expected_methods` compares the declared `params[].ty` strings against the rustdoc export of the *usecase* crate only. Since the usecase crate depends on domain, when rustdoc runs `cargo +nightly rustdoc -p usecase`, it produces `usecase.json` where domain types referenced in usecase method signatures appear as `ResolvedPath` with their full path. `format_type` at `schema_export.rs:431-437` strips the module path to the last segment: `"domain::review_v2::ReviewTarget"` → `"ReviewTarget"`. The catalogue declares `"ty": "ReviewTarget"`. String match → **passes**. No cross-layer resolution needed. Confirmed: `format_type` produces the short name for any `ResolvedPath` regardless of crate origin.

**EC-6: Merge gate with domain=Blue + usecase=Yellow.**

Decision: **Yellow blocks in strict mode — existing behavior, no code change.** `check_type_signals` with `strict=true` treats Yellow as a finding (`has_errors()` returns true). Tests U23 (`test_u23_two_layers_one_yellow_one_blue_blocks_strict`) already covers this scenario exactly. For the usecase-enabled configuration, if `usecase-types.json` has Yellow signals on the PR branch, the gate blocks. This is the intended design. T010 adds a concrete test that uses `usecase.tddd.enabled = true` in the mock `read_enabled_layers` response rather than a hard-coded `["domain", "usecase"]` pair, confirming that the real `architecture-rules.json` adapter feeds the correct layer list.

**EC-7: `usecase-types.md` header collisions with layer-agnostic templates.**

Decision: **No collision risk.** `infrastructure::type_catalogue_render::render_type_catalogue` generates the markdown from the catalogue document. The file is written to `track/items/<id>/usecase-types.md` (distinct path). Section headers like `## Type Declarations` within the file are scoped to that file; there is no shared template that would concatenate them. If a developer manually creates a doc that includes both `domain-types.md` and `usecase-types.md`, they are responsible for top-level heading disambiguation. No code change needed.

**EC-8: `/track:design` with both `domain-types.json` and `usecase-types.json`.**

Decision: **`design.md` is rewritten (T009) to loop over all enabled layers by default; `--layer` selects a single layer.** The rewritten skill: (1) reads all `tddd.enabled` layers from `architecture-rules.json`; (2) if `--layer <id>` is specified, processes only that layer's catalogue; (3) if no `--layer`, iterates all enabled layers in `layers[]` order (domain first, then usecase). This is the multi-layer loop design described in the briefing.

---

### 4. Type Design Decisions

#### Decision 1: `resolve_layer_binding` vs `resolve_layers_to_process`

**Recommendation: Option B — `resolve_layers_to_process(workspace_root, layer_filter: Option<&str>) -> Result<Vec<TdddLayerBinding>, CliError>`**

Justification:
- `execute_type_signals` already calls `resolve_layers` (which is Option B's shape) and works correctly. We are extending it, not replacing it.
- `execute_baseline_capture` currently uses `enforce_domain_tddd_enabled` (a domain-specific special case). Replacing it with `resolve_layers_to_process` (the same shape as `resolve_layers`) unifies both code paths.
- Option A (`resolve_layer_binding` single-layer) would require `execute_type_signals` to call it once per binding in a loop, which it already assembles via `resolve_layers`. Using Option B means `execute_type_signals` needs no structural changes — just delete the Phase 1 guard code.
- Hexagonal principle: the caller (CLI command) decides the filter; the resolver returns the full enabled set or a filtered single. This is stateless, pure logic — no enum-first or typestate pattern is warranted here (no state transitions, no variant-dependent data).
- Naming: rename `enforce_domain_tddd_enabled` → `resolve_layers_to_process` (or reuse `resolve_layers` since `signals.rs` already has the correct shape). In practice, T004 should replace `enforce_domain_tddd_enabled` by calling `resolve_layers` from `signals.rs` (extracted to a shared module or duplicated — see T004 note).

**`resolve_layer_binding` convenience function:** For the case where `--layer <id>` is mandatory (like `baseline-capture`), a thin wrapper `resolve_layer_binding(workspace_root, layer_id) -> Result<TdddLayerBinding, CliError>` is useful:

```rust
pub fn resolve_layer_binding(
    workspace_root: &Path,
    layer_id: &str,
) -> Result<TdddLayerBinding, CliError> {
    let mut layers = resolve_layers(workspace_root, Some(layer_id))?;
    layers.into_iter().next().ok_or_else(|| CliError::Message(
        format!("layer '{layer_id}' is not tddd.enabled in architecture-rules.json")
    ))
}
```

This is a thin wrapper, not a separate design. The core logic stays in `resolve_layers`.

#### Decision 2: Fate of `skipped_enabled_layers` logic

**Decision: DELETE entirely in T003 and T004.**

The `skipped_enabled_layers` / partial-failure exit code logic was introduced in Codex rounds 6+10 as a compromise: "warn about non-domain enabled layers but don't fail, yet signal incomplete processing via exit code 1." After generalization, this compromise is no longer needed:

- If `--layer <id>` is specified: only that layer is processed. No skipping.
- If no `--layer`: all enabled layers are processed in the loop. No skipping.
- If a layer is listed as enabled but has no catalogue file: that is a user error, caught immediately by `execute_type_signals_for_layer` → `NotFound` → `Err`.

The only scenario where "skipping" would occur is if an enabled layer has `targets = ["nonexistent-crate"]` — this fails at `exporter.export()` step, not silently. There is no valid scenario post-generalization where a layer should be silently skipped with a warning.

After T005 flips `usecase.tddd.enabled = true`, both layers are always processed. The `skipped_enabled_layers` tracking was a Phase 1 safety valve that became dead code the moment multi-layer support was complete.

**Deleting this code also simplifies `baseline.rs` and `signals.rs` significantly** — the Phase 1 comments (`// T007: Phase 1...`), the `non_domain_enabled` collection, the `skipped_enabled_layers` Vec, and the `if !skipped.is_empty()` exit-code-downgrade blocks all go away.

#### Decision 3: Multi-target exporter merge strategy (future-proofing)

For the current track (single `["usecase"]` target), no merge is needed. For future tracks with multiple targets:

**Strategy: sequential export + vector concatenation with collision detection**

```rust
// pseudo-code for future multi-target merge
let mut merged_types = Vec::new();
let mut merged_traits = Vec::new();
// ... per field
let mut seen_type_names: HashSet<String> = HashSet::new();

for target_crate in binding.targets() {
    let schema = exporter.export(target_crate)?;
    for t in schema.types() {
        if !seen_type_names.insert(t.name().to_owned()) {
            eprintln!("[WARN] same-name type collision '{}' across targets; skipping duplicate (tddd-03)", t.name());
            continue;
        }
        merged_types.push(t.clone());
    }
    merged_traits.extend_from_slice(schema.traits());
    // ...
}
let merged_schema = SchemaExport::new(layer_id.to_owned(), merged_types, merged_functions, merged_traits, merged_impls);
```

The collision handling (warn + skip) is a placeholder until tddd-03 implements Red signal promotion for collisions. For Phase 2 of tddd-02: assert `binding.targets().len() == 1` at the call site and return `Err` if violated, since no multi-target usecase configuration exists in the current project.

#### Decision 4: UseCase struct kind in catalogue

**Critical correction to the briefing:** The briefing mentions `use_case` as a catalogue kind. This kind does NOT exist in `TypeDefinitionKind`. The valid kinds are `value_object`, `enum`, `typestate`, `error_type`, `trait_port`.

UseCase structs (`SaveTrackUseCase<W>`, etc.) are generic structs. The forward check for `ValueObject` simply confirms the type exists in the TypeGraph as a struct/type. Cataloguing them as `value_object` with `action: "reference"` (since they pre-exist) is the correct approach for the initial seed. However, for the minimum viable seed that proves end-to-end wiring, **UseCase structs should be omitted from the initial seed**. Only the 5 TraitPort entries are strictly needed to validate the wiring. Adding error_type entries for the 4 review_v2 error enums is also valuable (they have known variants that can be declared). UseCase structs add noise (generic type params are stripped by rustdoc formatting) without proving new capability.

**Recommendation for T006 seed scope:**
- **5 TraitPort entries** (mandatory — prove forward check for usecase layer)
- **4 error_type entries** from `review_v2/error.rs` (variants are stable and known)
- **5 value_object entries** for the 5 UseCase structs in `lib.rs` (action=reference since they pre-exist)
- Total: 14 entries — small enough to be manageable, large enough to exercise all kind paths

---

### 5. Error Type Placement

No new error enum variants are introduced by this track. All error surfaces go to the existing `CliError::Message(String)`.

| New error condition | Error enum | Layer | Rationale |
|---|---|---|---|
| `targets.is_empty()` guard for multi-target | `CliError::Message` | cli | User-facing configuration error; string message sufficient |
| Layer not found / not enabled (existing) | `CliError::Message` | cli | Already emitted by `resolve_layers`; no new variant |
| `--layer` baseline-capture rejection removed | n/a (deletion) | cli | The error is removed, not replaced |

No new domain or infrastructure error variant is needed. The `TdddLayerParseError` in `libs/infrastructure/src/verify/tddd_layers.rs` already covers parse failures and is re-wrapped to `CliError::Message` at the CLI boundary. No change to that error type.

---

### 6. Task Ordering (TDD-style, T001–T012)

#### T001: Confirm domain is already layer-agnostic — no change

**Scope:** Read-only verification.
**Work:** `libs/domain/src/tddd/` — `consistency.rs`, `signals.rs`, `baseline.rs`, `catalogue.rs`. Confirm that `check_consistency`, `check_type_signals`, `undeclared_to_signals`, `build_baseline` have no references to `"domain"` as a string literal.
**Expected result:** Zero changes. If a hardcode is found, add a task before T003 to fix it.
**Acceptance criteria:** `cargo make ci` passes unchanged; grep for `"domain"` in domain crate tddd module returns zero hits in logic code (only in tests/docs).

#### T002: Infrastructure exporter — confirm `export("usecase")` works; add test

**Scope:** `libs/infrastructure/src/schema_export.rs`.
**Work:** `run_rustdoc` is fully parametric on `crate_name` (`schema_export.rs:57`). `SchemaExportError::CrateNotFound` is returned if `cargo rustdoc -p usecase` fails because the crate doesn't exist or compilation fails. Add a new unit test `test_crate_not_found_returns_crate_not_found_error` that mocks a non-existent crate name. Confirm `export("usecase")` would work by checking that the `usecase` crate is in the workspace Cargo.toml.
**Key verification:** The artifact path logic at `schema_export.rs:86-87`: `let artifact_name = crate_name.replace('-', "_");` → for `"usecase"` → `"usecase.json"`. This is correct since `usecase` contains no hyphens.
**Acceptance criteria:** No code change to `schema_export.rs` unless a new test for `CrateNotFound` is worth adding (< 50 lines). Test passes.

#### T003: CLI `signals.rs` generalization — remove domain hardcode, rename function

**Scope:** `apps/cli/src/commands/track/tddd/signals.rs`.
**Changes:**
1. Delete lines 102-136 (the `non_domain_enabled` + `skipped_enabled_layers` + `if filter != "domain"` block).
2. Delete lines 141-148 (the domain-specific `let Some(domain_binding) = ...` find).
3. Delete lines 157-164 (the `skipped_enabled_layers` partial-failure exit block).
4. Replace with a loop: `for binding in &bindings { let exit = execute_type_signals_for_layer(..., binding)?; }` (exit code from last layer, or propagate first error).
5. Rename `execute_type_signals_single` → `execute_type_signals_for_layer` (doc update too).
6. In `execute_type_signals_for_layer`: replace `exporter.export("domain")` at line 201 with `exporter.export(binding.targets().first().ok_or_else(|| CliError::Message(...))?)`.
7. Update the error message in the `NotFound` baseline arm to include `--layer {layer_id}` in the suggested command.
8. Remove the `// T007: Phase 1...` and `// Phase 1...` comment blocks.
**Estimated diff:** ~80 lines deleted, ~20 lines added = ~100 line diff.
**Acceptance criteria:** `cargo test -p cli` passes. `execute_type_signals` with `--layer domain` still returns correct behavior (loop of 1 binding). New test: `test_execute_type_signals_with_unknown_layer_returns_error` (already covered by `resolve_layers` tests; confirm no regression).

#### T004: CLI `baseline.rs` generalization — remove domain hardcode, rename function

**Scope:** `apps/cli/src/commands/track/tddd/baseline.rs`.
**Changes:**
1. Delete lines 43-49 (`if let Some(ref layer_id) = layer { if layer_id != "domain" { return Err(...) } }`).
2. Replace `enforce_domain_tddd_enabled` call at lines 57-58 with `resolve_layer_binding_for_capture(&workspace_root, layer.as_deref())` which:
   - If `layer = Some(id)`: calls `resolve_layers(&workspace_root, Some(id))?` → returns the single binding or error.
   - If `layer = None`: returns error `"--layer is required for baseline-capture"` (or defaults to iterating all enabled layers — see decision below).
3. **Decision for `--layer` omitted case in `baseline-capture`:** Unlike `type-signals` which can process all layers in one run, `baseline-capture` writes a single baseline file. If `--layer` is omitted, iterate all enabled layers and capture each. This matches the `type-signals` behavior and removes the special-case. The existing loop structure for `signals.rs` serves as a template.
4. Delete the `enforce_domain_tddd_enabled` function entirely (lines 191-235).
5. Delete the `synthetic_domain_binding` function (lines 239-255) or fold it into `resolve_layers` legacy fallback.
6. Delete the `skipped_enabled_layers` partial-failure logic (lines 162-169).
7. Remove the `// T007: Phase 1...` comment block (lines 38-50).
8. Replace `exporter.export("domain")` at line 130 with `exporter.export(binding.targets().first().ok_or_else(|| CliError::Message(...))?)`.
9. Update doc comment of `execute_baseline_capture` to remove domain-only references.
**Estimated diff:** ~120 lines deleted, ~40 lines added = ~160 line diff.
**Acceptance criteria:** `cargo test -p cli` passes. Existing tests `test_baseline_capture_skips_when_baseline_exists` and `test_baseline_capture_force_flag_bypasses_skip` still pass (they use `layer = None`). New test: `test_baseline_capture_with_usecase_layer_reads_usecase_binding` (unit test asserting correct binding resolution with a mock `architecture-rules.json`).

#### T005: `architecture-rules.json` flip — enable usecase TDDD

**Scope:** `architecture-rules.json` (root).
**Changes:** Update the `usecase` layer's `tddd` block from `{ "enabled": false }` to the full block (see Canonical Blocks section). Confirm JSON validity with `jq`.
**Estimated diff:** ~5 lines changed.
**Acceptance criteria:** `parse_tddd_layers` on the updated file returns 2 bindings (domain + usecase). `cargo make ci` still passes (TDDD gate for domain still passes; usecase gate skipped since `usecase-types.json` not yet on the branch). Note: T005 must land before T007 since T007 runs `baseline-capture --layer usecase` which requires `usecase.tddd.enabled = true`.

#### T006: Seed `usecase-types.json`

**Scope:** New file `track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.json`.
**Content:** See Canonical Blocks section for the complete JSON.
**Work:** Author the 14-entry seed (5 TraitPort + 4 ErrorType + 5 ValueObject). Verify that all method signatures in `expected_methods` exactly match the actual rustdoc-exported signatures by consulting the source files directly (confirmed during planning, see source files read above).
**Key method signatures to verify:**
- `TrackBlobReader::read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument>` — `merge_gate.rs:57`
- `TrackBlobReader::read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<TypeCatalogueDocument>` — `merge_gate.rs:65-71`
- `TrackBlobReader::read_track_metadata(&self, branch: &str, track_id: &str) -> BlobFetchResult<TrackMetadata>` — `merge_gate.rs:78`
- `TrackBlobReader::read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>` — `merge_gate.rs:87` (has default impl; still a trait method)
- `HookHandler::handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError>` — `hook.rs:31`
- `Reviewer::review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError>` — `ports.rs:12`
- `Reviewer::fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError>` — `ports.rs:20`
- `DiffGetter::list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError>` — `ports.rs:34`
- `ReviewHasher::calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError>` — `ports.rs:47`

**Note on `HookHandler`:** `HookError` is defined in `domain::hook::error` but is the return type of the `HookHandler` port method. It belongs in the usecase catalogue as `error_type` (it is the error surface of the `HookHandler` port). The catalogue entry should note in `description` that it is defined in the `domain` crate but catalogued here as the port's error type. The forward check for `ErrorType` checks for the type's existence in the usecase rustdoc export — but `HookError` is in the `domain` crate, not `usecase`. **This means `HookError` will NOT appear in `target/doc/usecase.json` as a top-level type.** It is a cross-crate reference. The forward check for this entry will produce Yellow (not found in usecase export). **Resolution: do NOT include `HookError` in `usecase-types.json`. Catalogue it in a future `domain-types.json` update or in the usecase catalogue with `action: "reference"` acknowledging that it lives in domain.** For the minimum seed, omit `HookError` and all other domain-owned error types from the usecase catalogue seed.

Revised scope for T006: 5 TraitPort + 4 ErrorType (review_v2 only, all defined in usecase) + 2 ValueObject (`SaveTrackUseCase<W>`, `LoadTrackUseCase<R>` as representatives) = **11 entries**.

**Estimated diff:** New file ~200 lines.
**Acceptance criteria:** File is valid JSON per `catalogue_codec::decode`. `approved: true` on all entries. All method signatures in `expected_methods` match the actual source code exactly.

#### T007: Run `baseline-capture --layer usecase --force`; commit baseline

**Scope:** New file `track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types-baseline.json`.
**Work:** Requires T003, T004, T005, T006 complete. Run `cargo run -p cli -- track baseline-capture tddd-02-usecase-wiring-2026-04-14 --layer usecase --force` (or via `cargo make track-baseline-capture`). Commit the resulting `usecase-types-baseline.json`.
**Acceptance criteria:** `usecase-types-baseline.json` exists and is valid JSON per `baseline_codec::decode`. Type count and trait count printed to stdout match the expected counts from the usecase crate's pub API.

#### T008: Run `type-signals --layer usecase`; iterate until signals stabilize

**Scope:** Updated `track/items/tddd-02-usecase-wiring-2026-04-14/usecase-types.json` and new `usecase-types.md`.
**Work:** Run `cargo run -p cli -- track type-signals tddd-02-usecase-wiring-2026-04-14 --layer usecase`. Inspect signals. Expected: 5 TraitPort entries → Blue (if method signatures match exactly) or Yellow (if any mismatch). 4 ErrorType entries → Blue (if variants match). 2 ValueObject entries → Blue. If Yellow/Red on TraitPort entries, diagnose by comparing `expected_methods` in the seed against the actual rustdoc output. Fix `usecase-types.json` accordingly.
**Acceptance criteria:** All catalogue entries are Blue or Yellow (no Red except from undeclared types absorbed by baseline). Print shows `red=0` for declared entries. `usecase-types.md` is generated.

#### T009: Rewrite `.claude/commands/track/design.md` for multi-layer loop

**Scope:** `.claude/commands/track/design.md`. No SKILL.md exists for `/track:design` (confirmed).
**Changes:**
1. Replace "Phase 1: domain only" references with multi-layer loop description.
2. Step 1 ("Gather context"): replace `domain-types.json` with `{layer}-types.json` loop. Read all enabled layers from `architecture-rules.json`.
3. Step 3 ("Write domain-types.json"): rename to "Write `{layer}-types.json`". Show example for both domain and usecase.
4. Step 4 ("Capture baseline"): replace domain-only baseline with `--layer <layer_id>` per layer. Remove "Phase 1 always captures domain only" text.
5. Add a "Multi-layer mode" section: "When `--layer` is not specified, `/track:design` iterates every `tddd.enabled` layer in `architecture-rules.json` order."
6. Remove all "Phase 1", "Phase 2 will wire additional layers", "only `domain` is wired" text.
7. Update Step 2 (`run /track:design`) verb from "domain types" to "layer types".
8. Section 5 guidance: update "Commit `domain-types.json`" to "Commit `{layer}-types.json` and `{layer}-types-baseline.json` for each processed layer."
**Estimated diff:** ~80 lines changed in the 163-line file.
**Acceptance criteria:** No references to "Phase 1 only: domain" remain. `--layer <id>` is described as an optional flag. The multi-layer loop is described clearly. The file is syntactically valid YAML front-matter + Markdown.

#### T010: Merge gate tests for usecase enablement (new U27-U30)

**Scope:** `libs/usecase/src/merge_gate.rs` (test module).
**New tests:**
- `test_u27_usecase_blue_domain_notfound_passes`: enabled_layers=["domain","usecase"], domain=NotFound, usecase=all_blue → passes
- `test_u28_usecase_blue_domain_blue_passes`: enabled_layers=["domain","usecase"], domain=all_blue, usecase=all_blue → passes
- `test_u29_usecase_yellow_domain_blue_blocks_strict`: enabled_layers=["domain","usecase"], domain=all_blue, usecase=dt_with_yellow → blocks (strict=true)
- `test_u30_empty_enabled_layers_from_real_config_blocks`: Confirm that an `architecture-rules.json` with both layers `enabled=false` would return an empty layers list and the gate fails-closed (already covered by existing logic; test documents the specific architecture-rules.json shape).

Additionally: update `merge_gate_adapter.rs` tests if any exist to confirm `read_enabled_layers` returns `["domain", "usecase"]` when both are enabled.
**Estimated diff:** ~120 lines added.
**Acceptance criteria:** All new tests pass. `cargo test -p usecase` passes. U19-U26 still pass.

#### T011: ADR 0002 amendment — B/C/E deferral rationale

**Scope:** `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md`.
**Changes:** Add a new subsection under `## Implementation Phases`:

```markdown
### Phase 2 Amendment: tddd-02-usecase-wiring (2026-04-14)

Status: Accepted (implemented in track tddd-02-usecase-wiring-2026-04-14, 2026-04-14)

The following sub-items from Phase 1's backlog were re-evaluated during Phase 2 planning:

**B. Finding collision → Red signal (deferred to tddd-03)**
Rationale: Implementing Red promotion before usecase TDDD is active means the mechanism
cannot dogfood itself. After usecase is wired, the Finding collision in domain and any
future usecase same-name collisions are caught by the same mechanism. Deferred.

**C. async-trait `is_async` detection (deferred — no async traits in usecase)**
Rationale: Current usecase traits are all sync (confirmed via source survey 2026-04-14).
ADR C2 constraint applies: catalogue authors must use `"async": false` for methods
implemented via `async-trait`. Heuristic detection deferred until a real async trait
enters the codebase.

**E. CI rustdoc cache strategy (deferred — independent track)**
Rationale: Entirely orthogonal to usecase wiring. Two `cargo +nightly rustdoc` invocations
(domain + usecase) add linear build time but are otherwise independent. Cache strategy
to be tracked separately.
```

**Estimated diff:** ~30 lines added.
**Acceptance criteria:** ADR file is valid Markdown. The three deferral rationales are present and match the briefing §Out of scope.

#### T012: Full `cargo make ci` run + fix regressions

**Scope:** Entire workspace.
**Work:** Run `cargo make ci` (fmt, clippy -D warnings, test, deny, check-layers, verify-spec-states, verify-arch-docs). Fix any regressions introduced by T003-T011. Expected regressions: possible clippy warnings from renamed functions (dead_code if tests still reference old name), possible `verify-spec-states` failure if `usecase-types.json` has Red signals before baseline is properly established.
**Acceptance criteria:** `cargo make ci` exits 0. All tests pass including the new U27-U30 and any new T003/T004 tests.

---

### 7. Canonical Blocks

The blocks below are verbatim-safe for copy-paste into `plan.md` and `knowledge/DESIGN.md`.

**Block 1: `architecture-rules.json` usecase.tddd block**

```json
{
  "crate": "usecase",
  "path": "libs/usecase",
  "may_depend_on": [
    "domain"
  ],
  "deny_reason": "Usecase crate must only be consumed by the cli layer.",
  "tddd": {
    "enabled": true,
    "catalogue_file": "usecase-types.json",
    "schema_export": {
      "method": "rustdoc",
      "targets": ["usecase"]
    }
  }
}
```

**Block 2: `usecase-types.json` initial seed (11 entries)**

```json
{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "TrackBlobReader",
      "description": "Usecase port for reading track-level domain documents from an external source (git ref, filesystem, etc). Returns BlobFetchResult<T> for each read operation.",
      "approved": true,
      "kind": "trait_port",
      "expected_methods": [
        {
          "name": "read_spec_document",
          "receiver": "&self",
          "params": [
            { "name": "branch", "ty": "&str" },
            { "name": "track_id", "ty": "&str" }
          ],
          "returns": "BlobFetchResult<SpecDocument>",
          "async": false
        },
        {
          "name": "read_type_catalogue",
          "receiver": "&self",
          "params": [
            { "name": "branch", "ty": "&str" },
            { "name": "track_id", "ty": "&str" },
            { "name": "layer_id", "ty": "&str" }
          ],
          "returns": "BlobFetchResult<TypeCatalogueDocument>",
          "async": false
        },
        {
          "name": "read_track_metadata",
          "receiver": "&self",
          "params": [
            { "name": "branch", "ty": "&str" },
            { "name": "track_id", "ty": "&str" }
          ],
          "returns": "BlobFetchResult<TrackMetadata>",
          "async": false
        },
        {
          "name": "read_enabled_layers",
          "receiver": "&self",
          "params": [
            { "name": "_branch", "ty": "&str" }
          ],
          "returns": "BlobFetchResult<Vec<String>>",
          "async": false
        }
      ]
    },
    {
      "name": "HookHandler",
      "description": "Usecase port for individual hook logic. Receives framework-free HookInput and returns a HookVerdict.",
      "approved": true,
      "kind": "trait_port",
      "expected_methods": [
        {
          "name": "handle",
          "receiver": "&self",
          "params": [
            { "name": "ctx", "ty": "&HookContext" },
            { "name": "input", "ty": "&HookInput" }
          ],
          "returns": "Result<HookVerdict, HookError>",
          "async": false
        }
      ]
    },
    {
      "name": "Reviewer",
      "description": "Usecase port for the external reviewer (e.g., Codex). Performs final and fast advisory reviews.",
      "approved": true,
      "kind": "trait_port",
      "expected_methods": [
        {
          "name": "review",
          "receiver": "&self",
          "params": [
            { "name": "target", "ty": "&ReviewTarget" }
          ],
          "returns": "Result<(Verdict, LogInfo), ReviewerError>",
          "async": false
        },
        {
          "name": "fast_review",
          "receiver": "&self",
          "params": [
            { "name": "target", "ty": "&ReviewTarget" }
          ],
          "returns": "Result<(FastVerdict, LogInfo), ReviewerError>",
          "async": false
        }
      ]
    },
    {
      "name": "DiffGetter",
      "description": "Usecase port for obtaining the list of changed files (diff) relative to a base commit.",
      "approved": true,
      "kind": "trait_port",
      "expected_methods": [
        {
          "name": "list_diff_files",
          "receiver": "&self",
          "params": [
            { "name": "base", "ty": "&CommitHash" }
          ],
          "returns": "Result<Vec<FilePath>, DiffGetError>",
          "async": false
        }
      ]
    },
    {
      "name": "ReviewHasher",
      "description": "Usecase port for computing review hashes from file contents. Empty targets return ReviewHash::Empty.",
      "approved": true,
      "kind": "trait_port",
      "expected_methods": [
        {
          "name": "calc",
          "receiver": "&self",
          "params": [
            { "name": "target", "ty": "&ReviewTarget" }
          ],
          "returns": "Result<ReviewHash, ReviewHasherError>",
          "async": false
        }
      ]
    },
    {
      "name": "ReviewerError",
      "description": "Errors from the Reviewer usecase port (abort, timeout, illegal verdict format, unexpected).",
      "approved": true,
      "action": "reference",
      "kind": "error_type",
      "expected_variants": [
        "UserAbort",
        "ReviewerAbort",
        "Timeout",
        "IllegalVerdict",
        "Unexpected"
      ]
    },
    {
      "name": "DiffGetError",
      "description": "Errors from the DiffGetter usecase port.",
      "approved": true,
      "action": "reference",
      "kind": "error_type",
      "expected_variants": [
        "Failed"
      ]
    },
    {
      "name": "ReviewHasherError",
      "description": "Errors from the ReviewHasher usecase port.",
      "approved": true,
      "action": "reference",
      "kind": "error_type",
      "expected_variants": [
        "Failed"
      ]
    },
    {
      "name": "ReviewCycleError",
      "description": "Errors from the ReviewCycle orchestrator (unknown scope, file changed during review, propagated port errors).",
      "approved": true,
      "action": "reference",
      "kind": "error_type",
      "expected_variants": [
        "UnknownScope",
        "FileChangedDuringReview",
        "Diff",
        "Hash",
        "Reviewer",
        "Reader"
      ]
    },
    {
      "name": "SaveTrackUseCase",
      "description": "Use case that persists a track aggregate via the TrackWriter port.",
      "approved": true,
      "action": "reference",
      "kind": "value_object"
    },
    {
      "name": "LoadTrackUseCase",
      "description": "Use case that loads a track aggregate by ID via the TrackReader port.",
      "approved": true,
      "action": "reference",
      "kind": "value_object"
    }
  ]
}
```

**Block 3: CLI function signatures after generalization**

```rust
// apps/cli/src/commands/track/tddd/signals.rs

/// Evaluate type signals for all enabled TDDD layers (or a single layer if
/// `--layer` is specified) and write back the updated catalogue files.
pub fn execute_type_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;
    let mut exit = ExitCode::SUCCESS;
    for binding in &bindings {
        let layer_exit = execute_type_signals_for_layer(&items_dir, &track_id, &workspace_root, binding)?;
        if layer_exit != ExitCode::SUCCESS {
            exit = layer_exit;
        }
    }
    Ok(exit)
}

/// Evaluate type signals for a single TDDD layer given its resolved binding.
fn execute_type_signals_for_layer(
    items_dir: &std::path::Path,
    track_id: &str,
    workspace_root: &std::path::Path,
    binding: &TdddLayerBinding,
) -> Result<ExitCode, CliError> {
    // ... (unchanged body, except exporter.export("domain") → exporter.export(target_crate))
    let target_crate = binding.targets().first()
        .ok_or_else(|| CliError::Message(format!(
            "schema_export.targets is empty for layer '{}'; check architecture-rules.json",
            binding.layer_id()
        )))?;
    let schema = exporter.export(target_crate).map_err(|e| { ... })?;
    // ...
}
```

```rust
// apps/cli/src/commands/track/tddd/baseline.rs

/// Capture the current TypeGraph as a baseline snapshot for the specified layer.
pub fn execute_baseline_capture(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    force: bool,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;
    // symlink guard for items_dir (unchanged)
    for binding in &bindings {
        capture_baseline_for_layer(&items_dir, &track_id, &workspace_root, force, binding)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Captures a baseline for a single TDDD layer given its resolved binding.
fn capture_baseline_for_layer(
    items_dir: &std::path::Path,
    track_id: &str,
    workspace_root: &std::path::Path,
    force: bool,
    binding: &TdddLayerBinding,
) -> Result<(), CliError> {
    // ... (current execute_baseline_capture body, generalized)
    let target_crate = binding.targets().first()
        .ok_or_else(|| CliError::Message(format!(
            "schema_export.targets is empty for layer '{}'",
            binding.layer_id()
        )))?;
    let schema = exporter.export(target_crate).map_err(|e| { ... })?;
    // ...
}
```

**Block 4: `/track:design` multi-layer loop pseudo-code**

```
## Step 0: Resolve track and layer scope

- Extract track ID from current branch.
- Read `architecture-rules.json` to find all `tddd.enabled` layers.
- If `--layer <id>` argument provided: validate that `<id>` is tddd.enabled; process only that layer.
- If no `--layer`: process all enabled layers in `layers[]` order (default: domain first, then usecase, etc.).
- For each layer to process: derive `catalogue_file` from binding (e.g. `domain-types.json`, `usecase-types.json`).

## Step 1: Gather context

For each layer in the processing scope:
- Read `track/items/<id>/{layer}-types.json` if it exists.
- Read the layer's source code for types relevant to the plan.

## Step 4: Capture baseline and validate

For each layer in the processing scope:
1. Run `sotp track baseline-capture <id> --layer <layer_id> [--force]`
   to snapshot `{catalogue_stem}-baseline.json`.
2. Run `sotp track type-signals <id> --layer <layer_id>`
   to evaluate signals for that layer.
3. Print per-layer summary: Blue/Yellow/Red counts.

Run `sotp verify spec-states` (aggregates all enabled layers).
Run `cargo make ci`.
```

---

### 8. Risks & Open Questions

#### R1: Reviewer loop vibration risk (Phase 1 PR #95 had 11 rounds)

The primary vibration source in PR #95 was the `skipped_enabled_layers` / partial-failure exit code design: Codex alternated between "fail-closed" and "warn-and-skip" through rounds 4, 6, 10. **This track eliminates that design entirely** — there is no "skipped enabled layers" concept after generalization. This eliminates the main oscillation source.

New vibration risk: **`binding.targets()[0]` vs full multi-target iteration.** A reviewer could argue for implementing the full multi-target merge now. **Counter-argument:** the ADR's D6 decision says single-element `targets` for Phase 2; the project has no multi-crate layers. Preemptively implement the `targets.first()` path and document the deferred merge in a comment.

#### R2: `HookError` cross-crate forward check failure

The `HookHandler` TraitPort entry in `usecase-types.json` declares `"returns": "Result<HookVerdict, HookError>"`. The forward check will find `HookHandler` in the usecase crate (it's defined there), but `HookError` is in the domain crate. The rustdoc export of usecase will render the return type as `"Result<HookVerdict, HookError>"` because `format_type` strips module paths. String match will succeed. **There is no actual problem here** — `format_type` strips module paths for all types including cross-crate references. Confirmed by ADR §D5 analysis and `format_type` implementation at `schema_export.rs:431-437`.

#### R3: Scope creep — B (Finding collision → Red) pulled into this track

A reviewer may observe that the `Finding` same-name collision is now blocking `domain-types.json` with a `reference` entry and suggest fixing it here. **Decision:** The briefing explicitly defers B to tddd-03. Resist this. The T011 ADR amendment documents the deferral rationale. If a Codex reviewer flags it as a "should fix," respond: "deferred per ADR 0002 Phase 2 Amendment; creating tddd-03 follow-up."

#### R4: Scope creep — C (async-trait `is_async`) pulled into this track

Current usecase traits are all sync (confirmed: none of the 5 trait methods use `async fn`). If during T008 a reviewer notices a Yellow on an `is_async` mismatch, this is a cataloguing error (wrong `"async"` value in the seed), not a C-scope issue. Fix the seed (T006 iteration). Document in T011 ADR.

#### R5: `read_enabled_layers` default impl returns `["domain"]` only

`merge_gate.rs:87-89` has a default implementation of `read_enabled_layers` that returns `["domain"]`. This means any mock that doesn't override `read_enabled_layers` will only test domain-layer gating. The existing tests U1-U18 use `MockTrackBlobReader` which does not override `read_enabled_layers`. After T005 enables usecase, these tests continue to pass (domain-only gate). But the real `GitShowTrackBlobReader` will return both layers. T010 adds tests using `MultiLayerMock` which does override `read_enabled_layers` to confirm usecase gating. The default-impl gap is an existing test coverage limitation, not a regression.

#### R6: `verify-spec-states` CI check — usecase-types.json must be present on PR branch

After T005 enables usecase TDDD, `verify spec-states` on any PR branch that does NOT include `usecase-types.json` will see `BlobFetchResult::NotFound` for the usecase catalogue → gate passes (NotFound = TDDD opt-out per `merge_gate.rs:211-213`). **This is the intended behavior.** Old tracks that predated usecase TDDD are unaffected. Only tddd-02's own PR branch (and future tracks that include `usecase-types.json`) will be gated.

#### R7: Open question — `read_enabled_layers` reads PR branch blob, not local workspace

The `read_enabled_layers` adapter reads `architecture-rules.json` from the PR branch via `git show origin/<branch>:architecture-rules.json`. This means a PR that modifies `architecture-rules.json` (e.g., T005) gates against the PR's own version of the rules — intentional per ADR §D1. There is a potential timing issue: if T005 (the `architecture-rules.json` flip) is committed before T006 (the seed), the CI `verify-spec-states` check on the partial PR (T005 committed, T006 not yet) would look for `usecase-types.json` on the PR branch and get `NotFound` → pass. This is safe. The concern would be if T006 is committed before T005 — then `usecase-types.json` exists but `usecase.tddd.enabled = false`, so `read_enabled_layers` would not include `usecase` → `usecase-types.json` is never read by the gate → safe. **No ordering constraint on T005 vs T006 from the gate's perspective.**

#### R8: `catalogue_codec` decoder — `use_case` kind string not registered

**Critical finding confirmed:** There is no `use_case` `kind_tag` in `TypeDefinitionKind`. The briefing's mention of a `use_case` kind is an error. The correct kind for UseCase structs in the catalogue is `value_object`. If the catalogue seed (T006) is authored with `"kind": "use_case"`, `catalogue_codec::decode` will fail at JSON deserialization. The Canonical Block (Block 2) above uses `"kind": "value_object"` for `SaveTrackUseCase` and `LoadTrackUseCase`. **Flag to implementer: do not use `use_case` as a kind string.**

---

### Critical Files for Implementation

- `/home/flip451/individual/t-rust/templates/SoTOHE-core/apps/cli/src/commands/track/tddd/signals.rs`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/apps/cli/src/commands/track/tddd/baseline.rs`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/architecture-rules.json`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/.claude/commands/track/design.md`
- `/home/flip451/individual/t-rust/templates/SoTOHE-core/libs/infrastructure/src/verify/tddd_layers.rs`