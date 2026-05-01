<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Reality View renderer の edge カバレッジ拡張 — receiver-less method / trait-method incoming + 起源別視覚区別

## Tasks (2/4 resolved)

### S1 — collect_edges() — receiver-less method coverage + edge_kind split

> The core behavioral change: remove the receiver().is_none() early-continue guard from the Methods branch so that associated functions (constructors, factories, parsers) become edge sources alongside instance methods.
> Simultaneously split the single 'method' edge_kind tag into 'method_return' (return-value origin) and 'method_param' (argument origin). The argument scan (method.params()) is added here for the first time; the existing return scan (method.returns()) is retained but re-tagged.
> Self-loop suppression (target != source_name) is applied on both the return side and the new argument side.
> This task touches only the collect_edges() function body and the doc-comment for the edge_kind values. No change to the public function signature or the EdgeSet enum (CN-01).
> T001 is a prerequisite for T003 because the new kind tags must exist before render_edge_symbol() dispatch can be extended.

- [x] **T001**: Remove receiver-less guard from collect_edges() Methods branch and split edge_kind from 'method' into 'method_return' (return-value origin) and 'method_param' (argument origin): (a) delete the is_none() early-continue so associated functions become edge sources, (b) add method.params() scan alongside the existing method.returns() scan, (c) emit 'method_return' for return-value edges and 'method_param' for argument edges, (d) apply self-loop suppression (target != source_name) on the argument side as well (`1303aa67dd60ae46f5c15e738839dd7dbe351e59`)

### S2 — collect_edges() — trait method scanning

> Add the graph.trait_names() loop to collect_edges() so that trait method return types and argument types also generate edges.
> Edge kinds emitted here are 'trait_method_return' (return-value origin) and 'trait_method_param' (argument origin), consistent with the nomenclature established in T001.
> The EdgeSet::Methods | EdgeSet::All guard ensures this block runs whenever method edges are requested.
> The optional trait-node cluster insertion handles the edge case where EdgeSet::Methods is called without EdgeSet::Impls: if the trait node that becomes an edge source is not already in the cluster (because impl edges were not generated), it is explicitly inserted. This prevents dangling source nodes in the mermaid output.
> This task depends on T001 (kind string convention) but is logically independent of T003 (rendering).

- [x] **T002**: Add trait method scanning block to collect_edges(): (a) add a graph.trait_names() loop inside the EdgeSet::Methods | EdgeSet::All guard, (b) for each trait node iterate trait_node.methods() extracting both returns() and params() via extract_type_names, (c) emit 'trait_method_return' / 'trait_method_param' edges with self-loop suppression (target != trait_name), (d) when EdgeSet::Methods is used without EdgeSet::Impls and a trait node that becomes an edge source is absent from the cluster, explicitly insert it into the cluster plan so the mermaid output remains self-contained

### S3 — render_edge_symbol() dispatch + linkStyle gray coloring

> Extend the mermaid rendering layer to visually distinguish return-value edges (-->) from argument edges (--o).
> render_edge_symbol() gains four new match arms for 'method_return', 'trait_method_return', 'method_param', and 'trait_method_param'. The old catch-all arm that defaulted unknown kinds to '-->' is replaced by the explicit named arms; 'field' and 'impl' arms are unchanged.
> In render_type_graph_clustered and render_type_graph_flat, the edge output loop is instrumented to collect the 0-based output-order indices of argument-derived edges. After all edge lines are emitted, a single 'linkStyle <i1>,<i2>,... stroke:#888;' line is appended when the index list is non-empty.
> render_type_graph_overview is explicitly not modified (OS-03): overview aggregates cross-cluster edges without per-edge indices, so individual coloring is inapplicable.
> This task depends on T001 and T002 for the kind values it dispatches on.

- [ ] **T003**: Extend render_edge_symbol() dispatch to the 4-value edge_kind set and add linkStyle gray coloring for argument-derived edges in clustered and flat renderers: (a) add match arms for 'method_return' / 'trait_method_return' mapping to '-->' and 'method_param' / 'trait_method_param' mapping to '--o', (b) in render_type_graph_clustered and render_type_graph_flat collect the output-order indices of argument-derived edges and append 'linkStyle <i1>,<i2>,... stroke:#888;' to the mermaid block when any such indices exist, (c) confirm render_type_graph_overview is NOT modified (OS-03 exclusion), (d) update the cluster-file leading comment to note '--o = argument-derived edge'

### S4 — Test updates and CI gate

> Update existing tests and add new unit tests to cover all behavioural changes introduced in T001-T003.
> The test_render_skips_associated_functions_without_self test asserts the opposite of the new behaviour (it expected no edge). It must be updated (or split into a new test that verifies edges DO appear) to reflect the T001 change.
> New tests cover: (a) associated-function return-value edge with kind 'method_return', (b) associated-function argument edge with kind 'method_param' and '--o' symbol, (c) trait method return-value edge with kind 'trait_method_return', (d) trait method argument edge with kind 'trait_method_param', (e) self-loop suppression on both sides (associated function returning Self and taking Self-typed param), (f) linkStyle index correctness when argument-derived edges are present, (g) linkStyle absent when no argument-derived edges exist, (h) render_type_graph_overview unchanged (no linkStyle line), (i) EdgeSet::Methods-without-Impls trait-node cluster insertion: when EdgeSet::Methods is used without EdgeSet::Impls, a trait node that becomes an edge source is explicitly inserted into the cluster so the clustered mermaid output has no dangling source nodes.
> All snapshot fixtures that reference the old 'method' kind string or a '-->' -only mermaid output are updated to match the new 4-value kind set.
> The CI gate (cargo make ci) is the final enforcement: fmt-check, clippy, nextest, deny, check-layers, and verify-* must all pass.

- [ ] **T004**: Update snapshot and unit tests to match the new 4-value edge_kind and '--o' / 'linkStyle' output, and run the full CI gate: (a) update or replace the existing test_render_skips_associated_functions_without_self test — it should now assert that associated functions DO create edges, (b) add new unit tests for method_param edge emission, associated-function-to-param and associated-function-to-return coverage, trait method return and param edges, self-loop suppression on both sides, and linkStyle index correctness, (c) update any snapshot fixtures that contain the old 'method' kind or '-->' -only output, (d) verify cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) passes
