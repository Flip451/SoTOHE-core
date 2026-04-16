# Planner Output — TDDD Type Graph View Phase 2 (cluster + drift + bug fix)

**Date**: 2026-04-17 (UTC 15:50)
**Capability**: planner (Claude Opus)
**Track**: `tddd-type-graph-cluster-2026-04-17`
**Status**: Authoritative task breakdown + canonical blocks produced
**Briefing**: `tmp/planner-briefing-tddd-type-graph-cluster-2026-04-17.md`
**ADR (SSoT)**: `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md`

---

## Summary

Phase 2 adopts a layered build-up strategy: bug fix + TDDD-Q01 land first (T001-T002) to ensure `check_type_signals` carries the correct file name before any new code calls it; then the `ClusterPlan` module (T003) and directory-layout writer (T004) give the clustering primitive; then edges expansion + entry-point detection (T005-T006) sits on top; and DRIFT-01 + orphan detection (T007) consumes the already-built `ClusterPlan`; finally auto-render integration (T008) closes the loop in `sotp track type-signals`. Each task is self-contained, compilable, and under 500 LOC diff. The `ClusterPlan` struct lives in its own `type_graph_cluster.rs` module (option C) because both the mermaid renderer and the drift checker consume it independently, and co-locating them with the renderer would create a bidirectional dependency.

---

## Answers to Q1–Q8

### Q1 — Task decomposition / ordering

**Answer: Option (a) with intra-bug-fix split.** Bug fix first, then clustering, then edges, then DRIFT-01, then auto-integration.

Rationale: Option (b) (clustering first) would require calling `check_type_signals` indirectly via `type-signals` auto-render integration while the bug is still live — meaning Phase 2 commits would produce misleading error messages in CI. Landing TDDD-BUG-02 (T001) before any code that relies on the new signature prevents that; it also isolates a risky 3-caller ripple into its own small diff.

Ordered tasks (8 total):

| id | one-line description | deps | expected LOC |
|---|---|---|---|
| T001 | TDDD-BUG-02 — `check_type_signals(doc, strict, catalogue_file: &str)` | none | ~80 |
| T002 | TDDD-Q01 — `test_sections_covers_all_kind_tags` | none | ~25 |
| T003 | `ClusterPlan` module (`type_graph_cluster.rs`) + `classify_types` | none | ~150 |
| T004 | Cluster directory layout — `write_type_graph_dir` + stale-file cleanup + `cluster_depth` option | T003 | ~250 |
| T005 | Edges expansion — `EdgeSet::Fields` + `EdgeSet::Impls` implementation | T004 | ~180 |
| T006 | Entry-point detection — shallow-unwrap predicate + `classDef entry` marking | T005 | ~130 |
| T007 | DRIFT-01 + orphan — `DriftReport`, `check_drift`, `--check-drift` CLI flag | T003, T004 | ~280 |
| T008 | Auto-render integration — `evaluate_and_write_signals` calls `write_type_graph_dir` fail-closed | T004, T007 | ~120 |

### Q2 — Cluster classifier design: Option (c)

New module `libs/infrastructure/src/tddd/type_graph_cluster.rs` with `ClusterPlan` struct. Both the mermaid renderer and the drift checker import unidirectionally from this module, avoiding bidirectional coupling. Convention reference: `.claude/rules/04-coding-principles.md` §"1モジュールに1つの責務".

### Q3 — Directory layout migration: Option (b) with double symlink guard

Delete the "other" mode's output on write entry, only when present, with symlink check first.
1. Before deleting `domain-graph.md` (entering cluster mode): `reject_symlinks_below` first; if symlink, fail-closed `InvalidInput`.
2. Before `remove_dir_all` on `domain-graph/`: same check.
3. TOCTOU: Developer-owned track dir under `items_dir`; worst case `remove_file` on symlink only removes the link, not the target. Acceptable for developer tooling.

Convention reference: `knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters`.

### Q4 — Entry point detection for wrappers: Option (b) shallow unwrap

Strip one layer of `Arc` / `Rc` / `Box` / `Result` / `Option` / `Vec`, then check tokens against primitive/external allowlist. The check is applied to parameter types (not return types) per ADR §D3. This correctly classifies `fn new(name: &str) -> Result<TrackId, DomainError>` as an entry point (both params are primitive), while excluding methods with workspace types in their arguments.

### Q5 — DRIFT-01 output: Option (a) always write

The visualized violations ARE the value; CI exit code is the gate. `[FAIL] N drift violation(s) in <layer>` stderr line format for actionable CI logs. Non-zero exit on any violation across all layers.

### Q6 — Error types: Option (c) `DriftReport` as value, `std::io::Error` for I/O, `CliError::Message` at CLI boundary

`DriftReport` is a *value* (domain-level observation), not an error. `check_drift` returns `DriftReport`. CLI maps `has_violations()` to `ExitCode::FAILURE`. Layout/write keep `std::io::Error`. `architecture-rules.json` parse errors use existing `LoadTdddLayersError` via `load_tddd_layers_from_path` (reusable).

### Q7 — TDDD-BUG-02: Option (a) clean breaking change

Per `feedback_no_backward_compat.md` memory. 3 crates + 8 test sites updated in one diff. Mechanical update: `merge_gate.rs:220` has `layer_id` in scope (format `{layer_id}-types.json`); `spec_states.rs:231` has `binding.catalogue_file()` already in scope.

### Q8 — Auto-render integration

- **Fail-closed: yes** — broken renderer should not produce green signal
- **After catalogue write: yes** — reuse the already-built `TypeGraph`; catalogue write is atomic and completes first
- **Default cluster_depth: 2** — Phase 1 empirical data (110 types, 50 connected unreadable flat) confirms
- **Default edge_set: `EdgeSet::Methods`** — simplest/readable for daily use; `--edges all` for deeper views

Recovery path error message: `"type-graph auto-render failed: {e}. Catalogue was written. Re-run `sotp track type-graph` after fixing the render issue."`

---

## Task Breakdown (authoritative)

### T001 — TDDD-BUG-02: add `catalogue_file` parameter to `check_type_signals`

**Description.**
(a) **New types/errors**: none.
(b) **Files touched**:
- `libs/domain/src/tddd/consistency.rs` — signature change + 8 test call-sites
- `libs/domain/src/lib.rs` — re-export signature update (if re-exported with explicit types)
- `libs/usecase/src/merge_gate.rs:220` — pass `format!("{layer_id}-types.json")`
- `libs/infrastructure/src/verify/spec_states.rs:231` — pass `binding.catalogue_file()`
(c) **Tests added**: update 8 existing tests + add 2 new tests asserting `catalogue_file` appears in error messages.
(d) **Convention docs**: `.claude/rules/04-coding-principles.md` §"No Panics"; `.claude/rules/05-testing.md` §TDD.

**TDD signal.** Red: every existing call-site fails to compile. Green: after signature change all tests pass with `catalogue_file` in error strings.

**LOC.** ~80. **Deps.** None.

---

### T002 — TDDD-Q01: `SECTIONS` exhaustive coverage test

**Description.**
(a) **New types**: none.
(b) **Files touched**: `libs/infrastructure/src/type_catalogue_render.rs` — add `#[cfg(test)] mod tests` block.
(c) **Tests added**: `test_sections_covers_all_kind_tags` — builds `all_tags: HashSet<&str>` from 13 `TypeDefinitionKind` variants' `kind_tag()`; compares against `SECTIONS.iter().map(|s| s.kind_tag)`; fails if new variant added without SECTIONS update.
(d) **Convention docs**: `.claude/rules/05-testing.md` §"カバレッジ目標"; ADR §D9 TDDD-Q01.

**TDD signal.** Red: test fails if SECTIONS missing any variant. Green: 13-element sets match.

**LOC.** ~25. **Deps.** None (parallel with T001).

---

### T003 — `ClusterPlan` module: `type_graph_cluster.rs`

**Description.**
(a) **New types**:
- `ClusterKey = String` (type alias)
- `ClusterPlan { depth: usize, assignments: HashMap<ClusterKey, Vec<String>>, cross_edges: Vec<CrossEdge> }`
- `CrossEdge { source_type, source_cluster, target_type, target_cluster, label, edge_kind }`
- `UNRESOLVED_CLUSTER: &str = "unresolved"` (const)
- `classify_types(graph, depth, edges) -> ClusterPlan` (pure)
(b) **Files touched**:
- `libs/infrastructure/src/tddd/type_graph_cluster.rs` (new, ~150 lines)
- `libs/infrastructure/src/tddd/mod.rs` — add `pub mod type_graph_cluster;`
(c) **Tests added**: 6 pure unit tests:
- `test_classify_types_depth_zero_returns_single_cluster`
- `test_classify_types_depth_one_groups_by_top_segment`
- `test_classify_types_depth_two_groups_by_two_segments`
- `test_classify_types_none_module_path_goes_to_unresolved`
- `test_cross_edges_detected_for_inter_cluster_method_edges`
- `test_cross_edges_empty_when_all_types_in_same_cluster`
(d) **Convention docs**: `.claude/rules/04-coding-principles.md` §"enum-first" (pure function, no mutable state).

**TDD signal.** Red: module does not exist. Green: all 6 unit tests pass with no I/O.

**LOC.** ~150. **Deps.** None.

---

### T004 — Cluster directory layout: `write_type_graph_dir` + `TypeGraphRenderOptions::cluster_depth`

**Description.**
(a) **New types**:
- `TypeGraphRenderOptions::cluster_depth: usize` field (Default = 2)
- `render_type_graph_clustered(graph, cluster_key, plan, opts) -> String`
- `render_type_graph_overview(graph, plan, opts) -> String`
- `write_type_graph_dir(graph, layer_id, track_dir, trusted_root, opts) -> Result<Vec<String>, io::Error>`
- Private: `remove_stale_flat_file`, `remove_stale_cluster_dir`
(b) **Files touched**:
- `libs/infrastructure/src/tddd/type_graph_render.rs` — cluster rendering + update `Default`
- `libs/infrastructure/src/tddd/type_graph_cluster.rs` — re-used by renderer
- `apps/cli/src/commands/track/tddd/graph.rs` — `--cluster-depth N` arg; dispatch flat vs dir
(c) **Tests added**:
- `test_write_type_graph_dir_creates_index_and_cluster_files`
- `test_write_type_graph_dir_rejects_symlink_track_dir`
- `test_write_type_graph_dir_removes_stale_flat_file_on_cluster_mode`
- `test_write_type_graph_file_removes_stale_cluster_dir_on_flat_mode`
- `test_render_type_graph_overview_contains_cluster_nodes`
- `test_render_type_graph_clustered_contains_only_intra_cluster_edges`
(d) **Convention docs**: `.claude/rules/04-coding-principles.md` §"No Panics"; symlink safety analysis per Q3; `.claude/rules/10-guardrails.md` §"Small task commits".

**TDD signal.** Red: `write_type_graph_dir` does not exist. Green: directory with `index.md` + cluster files; stale cleanup works.

**LOC.** ~250. **Deps.** T003.

---

### T005 — Edges expansion: `EdgeSet::Fields` and `EdgeSet::Impls`

**Description.**
(a) **New types**: none (enum variants already exist as stubs).
(b) **Files touched**:
- `libs/infrastructure/src/tddd/type_graph_render.rs` — implement Fields (`A --- B`) and Impls (dashed `A -.-> Trait`) edges; add `classDef traitNode` and stadium shape for traits
- `apps/cli/src/commands/track/tddd/graph.rs` — `--edges methods|fields|impls|all` flag
(c) **Tests added**:
- `test_render_field_edges_for_struct_members`
- `test_render_trait_impl_dashed_edges`
- `test_render_all_edge_set_includes_methods_fields_impls`
- `test_trait_node_rendered_with_stadium_shape`
(d) **Convention docs**: ADR §D2 edge semantics; `.claude/rules/04-coding-principles.md` §"enum-first".

**TDD signal.** Red: `EdgeSet::Fields` produces no edges (Phase 1 stub). Green: `FsReviewStore -.->|impl| ReviewReader` appears in output.

**LOC.** ~180. **Deps.** T004.

---

### T006 — Entry-point detection: `is_entry_point` + `classDef entry`

**Description.**
(a) **New types**:
- `EntryPointAnalysis { entry_types: HashSet<String> }`
- `PRIMITIVE_ALLOWLIST: &[&str]`
- `detect_entry_points(graph, workspace_type_names) -> EntryPointAnalysis`
- `is_entry_point_param(ty, workspace) -> bool` (shallow unwrap)
- `TypeGraphRenderOptions::entry_point_marking: bool` (Default = true)
(b) **Files touched**: `libs/infrastructure/src/tddd/type_graph_render.rs`
(c) **Tests added**:
- `test_detect_entry_points_with_all_primitive_params`
- `test_detect_entry_points_skips_methods_with_workspace_params`
- `test_detect_entry_points_shallow_unwraps_result_outer`
- `test_detect_entry_points_excludes_self_receiver`
- `test_render_marks_entry_type_with_entry_class`
(d) **Convention docs**: ADR §D3 entry-point definition; Q4 answer.

**TDD signal.** Red: `detect_entry_points` does not exist. Green: types with all-primitive params gain `:::entry` class.

**LOC.** ~130. **Deps.** T005.

---

### T007 — DRIFT-01 + orphan detection: `DriftReport` + `--check-drift`

**Description.**
(a) **New types**:
- `DriftViolation { source_cluster, target_cluster, source_type, target_type, label }`
- `DriftReport { violations: Vec<DriftViolation>, orphan_types: Vec<String> }` with `has_violations()`, `violation_count()`, `orphan_count()`
- `check_drift(graph, plan, bindings, all_edges) -> DriftReport` (pure, uses depth=1 clusters internally regardless of render depth per R3)
- `detect_orphans(graph, edges) -> Vec<String>`
(b) **Files touched**:
- `libs/infrastructure/src/tddd/type_graph_cluster.rs` — add `check_drift` + `detect_orphans` + types
- `libs/infrastructure/src/tddd/type_graph_render.rs` — `:::violation` Red style on cross-edges; `:::orphan` on orphans
- `apps/cli/src/commands/track/tddd/graph.rs` — `--check-drift` bool; emit `[FAIL]` stderr; `ExitCode::FAILURE` when `has_violations()`. Orphans `[WARN]` only (see OQ3)
(c) **Tests added**:
- `test_check_drift_with_allowed_cross_edge_returns_no_violation`
- `test_check_drift_with_forbidden_cross_edge_returns_violation`
- `test_check_drift_with_empty_plan_returns_empty_report`
- `test_detect_orphans_returns_types_with_zero_edges`
- `test_detect_orphans_returns_empty_when_all_connected`
- `test_drift_report_has_violations_false_when_empty`
- `test_execute_type_graph_with_check_drift_flag_exits_nonzero_on_violation` (integration)
(d) **Convention docs**: ADR §D8; Q5/Q6; `.claude/rules/04-coding-principles.md` §"enum-first".

**TDD signal.** Red: `DriftReport` does not exist. Green: `--check-drift` returns non-zero on forbidden cross-cluster edges.

**LOC.** ~280. **Deps.** T003, T004.

---

### T008 — Auto-render integration in `sotp track type-signals`

**Description.**
(a) **New types**: none.
(b) **Files touched**:
- `apps/cli/src/commands/track/tddd/signals.rs` — in `evaluate_and_write_signals`: after catalogue `atomic_write_file`, call `write_type_graph_dir` with `opts = TypeGraphRenderOptions::default()` (cluster_depth=2, EdgeSet::Methods, entry_point_marking=true). Add `auto_render_type_graph(profile, layer_id, track_dir, trusted_root) -> Result<(), CliError>`. Map `io::Error` to `CliError::Message` (fail-closed).
(c) **Tests added**:
- `test_evaluate_and_write_signals_triggers_graph_render_after_catalogue_write`
- `test_evaluate_and_write_signals_fails_closed_when_render_fails`
- Update existing `test_evaluate_and_write_signals_with_clean_report_returns_success_and_writes_files` to assert `<layer>-graph/index.md` is written
(d) **Convention docs**: ADR §D6 auto-render integration; Q8; `.claude/rules/05-testing.md` §"外部依存はモックされている".

**TDD signal.** Red: no graph file written post `evaluate_and_write_signals`. Green: `domain-graph/index.md` appears alongside `domain-types.json`.

**LOC.** ~120. **Deps.** T004, T007 (soft — T004 alone sufficient if T007 later).

---

## Plan Sections

**S001 — Bug fixes and invariant tests (T001, T002)**
Clean the two known debt items before Phase 2 code is written. T001 fixes the hardcoded filename bug; T002 closes the SECTIONS coverage gap. Foundation cleanup.

**S002 — Cluster primitive and directory layout (T003, T004)**
Introduce `ClusterPlan` as shared data structure, then implement the directory-layout writer. Most architecturally significant step — changes output shape from one file to directory.

**S003 — Edges expansion and entry-point detection (T005, T006)**
Fill in Phase 2 edge types (fields, trait impls) and add entry-point detection.

**S004 — DRIFT-01 foundation and orphan detection (T007)**
Introduce `DriftReport`, `check_drift`, orphan detection, `--check-drift` CLI flag. CI gate task.

**S005 — Auto-render integration (T008)**
Wire `write_type_graph_dir` into `evaluate_and_write_signals`. Final acceptance: `sotp track type-signals` produces both catalogue and graph.

---

## Risks + Mitigations

**R1 — Mermaid subgraph + `classDef` interaction**
Mermaid `subgraph` blocks may not apply `classDef` to nodes inside subgraphs consistently across renderers. Mitigation: Implement and inspect real output early in T004. Move `classDef` outside subgraph if needed.

**R2 — `write_type_graph_dir` + `remove_dir_all` is irreversible**
Data-loss risk if symlink guard has gap. Mitigation: Double-guard — (a) validate `track_dir` under `trusted_root`; (b) `reject_symlinks_below` on `<layer>-graph/` before `remove_dir_all`; (c) unit test injecting symlink verifies `remove_dir_all` NOT called.

**R3 — `check_drift` cluster key vs crate name alignment**
`module_path` uses `domain::review_v2`; `architecture-rules.json::layers[].crate` uses `domain`. Cluster key at depth=2 (`domain::review_v2`) won't match `may_depend_on` strings. Mitigation: `check_drift` uses **fixed depth=1 reclassification** of same `TypeGraph` internally, independent of render depth. Document in `type_graph_cluster.rs`.

**R4 — Auto-render fail-closed breaks `sotp track type-signals` on render errors**
Mitigation: Catalogue write atomic and completes first. Users recover by fixing render and re-running. Error message: `"type-graph auto-render failed: {e}. Catalogue was written. Re-run `sotp track type-graph` after fixing the render issue."`

**R5 — T001 3-caller ripple across crate boundaries**
Mitigation: Update domain → compile → usecase → compile → infrastructure → compile. `cargo make ci` as T001 acceptance.

---

## Open Questions (need empirical input from early Phase 2 commits)

**OQ1 — `TypeGraph::outgoing` deduplication with method edges** (ADR §4)
Typestate `outgoing` + method edges produce duplicate `Draft → Published`. Recommendation: `TypeGraphRenderOptions::deduplicate_typestate_edges: bool` (default true) in T004. Inspect real domain graph to confirm.

**OQ2 — Entry-point mermaid shape** (ADR §2)
Plan uses color-only (`classDef entry fill:#fff9c4,...`). If insufficient at cluster_depth=2, add stadium shape change as sub-task of T006.

**OQ3 — Orphan CI semantics**
Hard-fail vs warning-only depends on domain orphan count. Recommendation: `[WARN]` only + optional `--fail-on-orphans` flag.

**OQ4 — Auto-render default edge_set**
`EdgeSet::Methods` proposed. After T005, decide if `EdgeSet::All` overloads at cluster_depth=2. Decide after T005 manual inspection.

---

## Canonical Blocks

### `ClusterPlan` (libs/infrastructure/src/tddd/type_graph_cluster.rs)

```rust
pub type ClusterKey = String;

pub const UNRESOLVED_CLUSTER: &str = "unresolved";

/// Assignment of all types in a TypeGraph to named clusters.
///
/// `assignments`: maps each ClusterKey to the type names (short names, no `::`)
/// that belong to that cluster.
///
/// `cross_edges`: all edges (from any EdgeSet) whose source and target belong
/// to different clusters. Used by both the mermaid overview renderer and the
/// drift checker.
#[derive(Debug, Clone)]
pub struct ClusterPlan {
    pub depth: usize,
    pub assignments: HashMap<ClusterKey, Vec<String>>,
    pub cross_edges: Vec<CrossEdge>,
}

/// A directed edge that crosses cluster boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossEdge {
    pub source_type: String,
    pub source_cluster: ClusterKey,
    pub target_type: String,
    pub target_cluster: ClusterKey,
    /// Edge label (method name, field name, or trait name).
    pub label: String,
    /// Edge kind: "method" | "field" | "impl"
    pub edge_kind: &'static str,
}

/// Assigns every type in `graph` to a cluster based on `module_path` prefix up
/// to `depth` segments. Types with `module_path = None` go to UNRESOLVED_CLUSTER.
/// Also collects `cross_edges` from `edges`.
#[must_use]
pub fn classify_types(
    graph: &TypeGraph,
    depth: usize,
    edges: &[(String, String, String, &'static str)],
) -> ClusterPlan {
    // ...
}
```

### `DriftReport` + `check_drift`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftViolation {
    pub source_cluster: String,
    pub target_cluster: String,
    pub source_type: String,
    pub target_type: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct DriftReport {
    pub violations: Vec<DriftViolation>,
    pub orphan_types: Vec<String>,
}

impl DriftReport {
    #[must_use]
    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }

    #[must_use]
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    #[must_use]
    pub fn orphan_count(&self) -> usize {
        self.orphan_types.len()
    }
}

/// Always uses depth=1 cluster keys internally for comparison against
/// TdddLayerBinding::layer_id, regardless of render depth.
#[must_use]
pub fn check_drift(
    graph: &TypeGraph,
    plan: &ClusterPlan,
    bindings: &[TdddLayerBinding],
    all_edges: &[(String, String)],
) -> DriftReport {
    // ...
}
```

### `TypeGraphRenderOptions` (Phase 2 shape)

```rust
#[derive(Debug, Clone)]
pub struct TypeGraphRenderOptions {
    pub edge_set: EdgeSet,
    pub max_nodes_per_diagram: usize,
    /// `0` = flat (single file); `1` = top-level module; `2` = sub-module (default).
    pub cluster_depth: usize,
    pub entry_point_marking: bool,
    pub deduplicate_typestate_edges: bool,
}

impl Default for TypeGraphRenderOptions {
    fn default() -> Self {
        Self {
            edge_set: EdgeSet::Methods,
            max_nodes_per_diagram: 50,
            cluster_depth: 2,
            entry_point_marking: true,
            deduplicate_typestate_edges: true,
        }
    }
}
```

### `write_type_graph_dir`

```rust
/// Renders a clustered mermaid type graph and writes it to
/// `<layer_id>-graph/index.md` + `<layer_id>-graph/<cluster>.md` under `track_dir`.
///
/// When switching from flat mode (if `<layer_id>-graph.md` exists as a regular
/// file and is not a symlink), the stale flat file is deleted before writing.
///
/// # Errors
///
/// Returns `std::io::Error` if:
/// - `layer_id` contains unsafe path characters,
/// - the symlink guard rejects any output path,
/// - a stale-file removal fails, or
/// - any atomic write fails.
///
/// Returns a `Vec<String>` of the written filenames relative to `track_dir`.
pub fn write_type_graph_dir(
    graph: &TypeGraph,
    layer_id: &str,
    track_dir: &Path,
    trusted_root: &Path,
    opts: &TypeGraphRenderOptions,
) -> Result<Vec<String>, std::io::Error> {
    // ...
}
```

### `check_type_signals` new signature (TDDD-BUG-02)

```rust
// libs/domain/src/tddd/consistency.rs

/// Evaluates the Stage 2 signal gate for a type catalogue document.
///
/// # Arguments
/// * `doc` — the decoded `TypeCatalogueDocument`
/// * `strict` — when `true`, Yellow signals are treated as errors
/// * `catalogue_file` — the catalogue filename for error messages
///   (e.g. `"domain-types.json"`, `"usecase-types.json"`)
#[must_use]
pub fn check_type_signals(
    doc: &TypeCatalogueDocument,
    strict: bool,
    catalogue_file: &str,
) -> VerifyOutcome {
    // ...
}
```

### `EntryPointAnalysis` + `detect_entry_points`

```rust
const PRIMITIVE_ALLOWLIST: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize",
    "i8", "i16", "i32", "i64", "i128", "isize",
    "f32", "f64",
    "bool", "char", "str", "String",
    "()",
    // Wrapper types for shallow-unwrap
    "Arc", "Rc", "Box", "Result", "Option", "Vec",
];

#[derive(Debug, Clone, Default)]
pub struct EntryPointAnalysis {
    pub entry_types: std::collections::HashSet<String>,
}

#[must_use]
pub fn detect_entry_points(
    graph: &TypeGraph,
    workspace_type_names: &std::collections::HashSet<&str>,
) -> EntryPointAnalysis {
    // ...
}

/// Shallow unwrap: strip a single layer of Arc/Rc/Box/Result/Option/Vec,
/// then check the inner token(s) against workspace_type_names.
fn is_entry_point_param(
    ty: &str,
    workspace_type_names: &std::collections::HashSet<&str>,
) -> bool {
    // ...
}
```

### Mermaid style classes (canonical)

```mermaid
flowchart LR
    classDef structNode fill:#f3e5f5,stroke:#7b1fa2
    classDef enumNode fill:#e1f5fe,stroke:#0288d1
    classDef traitNode fill:#e8f5e9,stroke:#388e3c
    classDef entry fill:#fff9c4,stroke:#f9a825,stroke-width:3px
    classDef violation fill:#ffebee,stroke:#d32f2f,stroke-width:3px,color:#b71c1c
    classDef orphan fill:#f5f5f5,stroke:#9e9e9e,stroke-dasharray:5 5
```
