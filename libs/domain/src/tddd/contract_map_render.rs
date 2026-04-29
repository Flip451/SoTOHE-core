//! Contract Map renderer — pure function that converts per-layer type
//! catalogues into a single-file mermaid flowchart (ADR 2026-04-17-1528 §D1).
//!
//! Placement rationale: the function is I/O-free and is called directly
//! from the usecase interactor. Per ADR §D1 this belongs in the domain
//! layer — rendering the catalogue is a pure transformation, not an
//! infrastructure concern.
//!
//! Layer-agnostic invariant (ADR §4.5): the renderer never hard-codes
//! layer names. Every subgraph label comes from the `LayerId` supplied in
//! `layer_order`, and every edge is derived from the contents of
//! `catalogues`. The function therefore works identically for 2-layer,
//! 3-layer, or custom-layer architectures (verified by multilayer fixtures).
//!
//! The module intentionally does **not** import `infrastructure::…` — a
//! duplicate of `extract_type_names` lives here so the dependency
//! direction `domain → infrastructure` is never formed. See
//! `knowledge/conventions/hexagonal-architecture.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::tddd::LayerId;
use crate::tddd::catalogue::{
    MethodDeclaration, TraitImplDecl, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
};
use crate::tddd::contract_map_content::ContractMapContent;
use crate::tddd::contract_map_options::ContractMapRenderOptions;

/// Render the full contract map for the given per-layer catalogues.
///
/// Returns a [`ContractMapContent`] containing a markdown block with an
/// embedded mermaid `flowchart LR` diagram. One subgraph per surviving
/// layer (after `opts.layers` filtering), containing one node per surviving
/// entry (after `opts.kind_filter` filtering), plus:
///
/// * method-call edges (solid) when a method's `returns` string references
///   another entry that survived both filters;
/// * trait-impl edges (dashed) from `SecondaryAdapter` entries to the
///   `SecondaryPort` entries they declare to implement.
///
/// When `opts.kind_filter = Some(vec![])` is supplied, every entry is
/// filtered out and an empty-subgraph diagram is returned (not an error).
/// This lets CLI callers surface a warning without failing the pipeline.
#[must_use]
pub fn render_contract_map(
    catalogues: &BTreeMap<LayerId, TypeCatalogueDocument>,
    layer_order: &[LayerId],
    opts: &ContractMapRenderOptions,
) -> ContractMapContent {
    // 1. Layer filter — preserve topological order from `layer_order`.
    let active_layers: Vec<&LayerId> = if opts.layers.is_empty() {
        layer_order.iter().collect()
    } else {
        let allowed: BTreeSet<&LayerId> = opts.layers.iter().collect();
        layer_order.iter().filter(|l| allowed.contains(*l)).collect()
    };

    // 2. Entry filter — apply kind_filter and collect (layer, entry) pairs
    //    in active_layers order.
    let filter_tags: Option<BTreeSet<&str>> = opts
        .kind_filter
        .as_ref()
        .map(|kinds| kinds.iter().map(TypeDefinitionKind::kind_tag).collect());
    let entries: Vec<(&LayerId, &TypeCatalogueEntry)> = active_layers
        .iter()
        .copied()
        .flat_map(|layer| {
            catalogues
                .get(layer)
                .map(|doc| doc.entries().iter().map(move |entry| (layer, entry)))
                .into_iter()
                .flatten()
        })
        .filter(|(_layer, entry)| match &filter_tags {
            Some(tags) => tags.contains(entry.kind().kind_tag()),
            None => true,
        })
        .collect();

    // 3. Build lookup: type name → (layer, node_id) so edges can resolve
    //    targets by last-segment type name.
    //
    //    `type_index`              — all surviving entries (used for method-call
    //                                edges and FreeFunction param/return edges).
    //    `port_index`              — only `SecondaryPort` entries (used for
    //                                trait-impl edges so that `-.impl.->` never
    //                                accidentally targets a same-named
    //                                DTO/value-object).
    //    `application_service_index` — only `ApplicationService` entries (used
    //                                for Interactor `-.impl.->` edges so that the
    //                                dashed arrow never accidentally targets a
    //                                same-named non-service entry).
    //
    //    Per-layer catalogues may legitimately declare the same short name in
    //    different layers (e.g. `Error` or `Command` as layer-local types).
    //    Index values are therefore `Vec` so every matching declaration
    //    participates in edge resolution; a method whose signature references
    //    the shared short name fans out to all matching targets, keeping the
    //    ambiguity visible rather than silently dropping shadowed
    //    declarations.
    let mut type_index: BTreeMap<String, Vec<(LayerId, String)>> = BTreeMap::new();
    let mut port_index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut application_service_index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (layer, entry) in &entries {
        let id = node_id(layer, entry.name());
        type_index.entry(entry.name().to_owned()).or_default().push(((*layer).clone(), id.clone()));
        if matches!(entry.kind(), TypeDefinitionKind::SecondaryPort { .. }) {
            port_index.entry(entry.name().to_owned()).or_default().push(id.clone());
        }
        if matches!(entry.kind(), TypeDefinitionKind::ApplicationService { .. }) {
            application_service_index.entry(entry.name().to_owned()).or_default().push(id);
        }
    }

    // 4. Emit markdown.
    //
    // CN-08: a machine-readable marker is prepended so that tooling and
    // humans can recognise the file as generated and avoid manual edits
    // (ADR 2026-04-28-0135 §CN-08).
    let mut out = String::new();
    out.push_str("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->\n");
    out.push_str("```mermaid\n");
    out.push_str("flowchart LR\n");
    out.push_str("    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4\n");
    out.push_str("    classDef command fill:#e3f2fd,stroke:#1976d2\n");
    out.push_str("    classDef query fill:#f3e5f5,stroke:#8e24aa\n");
    out.push_str("    classDef factory fill:#fff8e1,stroke:#f9a825\n");
    out.push_str("    classDef free_function fill:#f1f8e9,stroke:#558b2f\n");
    out.push_str("    classDef domain_service fill:#fce4ec,stroke:#c62828\n");

    // 4a. Subgraphs.
    for layer in &active_layers {
        let label = sanitize_id(layer.as_ref());
        let _ = writeln!(out, "    subgraph {label} [{raw}]", raw = layer.as_ref());
        for (entry_layer, entry) in &entries {
            if entry_layer == layer {
                let _ = writeln!(out, "        {}", node_shape(layer, entry));
            }
        }
        out.push_str("    end\n");
    }

    // 4b. Edges — collected into a BTreeSet so duplicates are dropped and
    //     output order is deterministic.
    //
    //     Per ADR 2026-04-17-1528 §D4 (1) (Phase 1.5 extension), method-call
    //     edges are derived from BOTH `method.returns()` AND
    //     `method.params()`. Returns-derived edges keep the simple
    //     `method` label; params-derived edges use `method(arg_name)` so
    //     the Contract Map reader can distinguish them and see which
    //     parameter introduced the dependency. Only declared types
    //     (present in `type_index`) become edge targets — references to
    //     external types (e.g. `String`, `Result`) are ignored.
    let mut edges: BTreeSet<String> = BTreeSet::new();
    for (src_layer, entry) in &entries {
        let src_id = node_id(src_layer, entry.name());

        for method in methods_of(entry.kind()) {
            // Returns-derived edges (Phase 1 baseline).
            for token in extract_type_names(method.returns()) {
                if let Some(dsts) = type_index.get(token) {
                    for (_dst_layer, dst_id) in dsts {
                        if dst_id == &src_id {
                            continue;
                        }
                        edges.insert(format!(
                            "    {src_id} -->|{label}| {dst_id}",
                            label = escape_edge_label(method.name()),
                        ));
                    }
                }
            }

            // Params-derived edges (Phase 1.5 extension, ADR §D4 (1)).
            for param in method.params() {
                for token in extract_type_names(param.ty()) {
                    if let Some(dsts) = type_index.get(token) {
                        for (_dst_layer, dst_id) in dsts {
                            if dst_id == &src_id {
                                continue;
                            }
                            edges.insert(format!(
                                "    {src_id} -->|{label}| {dst_id}",
                                label = escape_edge_label(&format!(
                                    "{}({})",
                                    method.name(),
                                    param.name()
                                )),
                            ));
                        }
                    }
                }
            }
        }

        if let TypeDefinitionKind::SecondaryAdapter { implements, .. } = entry.kind() {
            for impl_decl in implements {
                // Use `port_index` (SecondaryPort entries only) so that
                // `-.impl.->` is never drawn to a same-named non-port entry
                // (ADR §D4: trait-impl edge targets secondary_port nodes only).
                if let Some(port_ids) = port_index.get(impl_decl.trait_name()) {
                    for port_id in port_ids {
                        edges.insert(format!("    {src_id} -.impl.-> {port_id}"));
                    }
                }
            }
        }

        // Interactor → ApplicationService trait-impl edges (ADR 2026-04-17-1528 §L3).
        //
        // Use `application_service_index` (ApplicationService entries only) so
        // that `-.impl.->` is never accidentally drawn to a same-named
        // non-service entry. Missing names (ApplicationService not in catalogue)
        // are silently skipped — no broken edge is emitted (CN-08 compliant:
        // no layer names hardcoded).
        if let TypeDefinitionKind::Interactor { declares_application_service, .. } = entry.kind() {
            for svc_name in declares_application_service {
                if let Some(svc_ids) = application_service_index.get(svc_name.as_str()) {
                    for svc_id in svc_ids {
                        edges.insert(format!("    {src_id} -.impl.-> {svc_id}"));
                    }
                }
            }
        }

        // FreeFunction param/return edges (ADR 2026-04-17-1528 §L2 / §L4).
        //
        // For each `expected_params[].ty` token that resolves in `type_index`,
        // draw an edge labelled with the parameter name.  For each token in
        // `expected_returns` that resolves in `type_index`, draw an edge
        // labelled `"returns"`.  Only declared types (present in `type_index`)
        // become edge targets — external types (e.g. `String`, `Result`) are
        // silently ignored.  Self-loops are suppressed.
        if let TypeDefinitionKind::FreeFunction { expected_params, expected_returns, .. } =
            entry.kind()
        {
            for param in expected_params {
                for token in extract_type_names(param.ty()) {
                    if let Some(dsts) = type_index.get(token) {
                        for (_dst_layer, dst_id) in dsts {
                            if dst_id == &src_id {
                                continue;
                            }
                            edges.insert(format!(
                                "    {src_id} -->|{label}| {dst_id}",
                                label = escape_edge_label(param.name()),
                            ));
                        }
                    }
                }
            }
            for ret_ty in expected_returns {
                for token in extract_type_names(ret_ty.as_str()) {
                    if let Some(dsts) = type_index.get(token) {
                        for (_dst_layer, dst_id) in dsts {
                            if dst_id == &src_id {
                                continue;
                            }
                            edges.insert(format!("    {src_id} -->|\"returns\"| {dst_id}"));
                        }
                    }
                }
            }
        }
    }

    for edge in &edges {
        out.push_str(edge);
        out.push('\n');
    }

    out.push_str("```\n");

    // Renderer never produces an empty string — even an empty catalogue
    // yields the `flowchart LR` scaffold. `ContractMapContent::new` is
    // validation-free, so the call is infallible and panic-free.
    ContractMapContent::new(out)
}

/// Rewrite an arbitrary string into a mermaid-safe, **injective**
/// identifier.
///
/// Mermaid node / subgraph identifiers must be `[A-Za-z0-9_]+`. To stay
/// injective (so that distinct inputs always map to distinct IDs — a
/// requirement for edge resolution to never alias unrelated nodes), the
/// encoding is:
///
/// - ASCII alphanumerics pass through verbatim.
/// - `_` is escaped as `__` (double underscore).
/// - Any other code point is escaped as `_<hex>_` where `<hex>` is the
///   lowercase hexadecimal representation of the Unicode scalar value.
///
/// The scheme is a bijection from `String` onto a subset of
/// `[A-Za-z0-9_]+`: the `_` prefix followed by either another `_` (for the
/// underscore escape) or a hex digit terminated by `_` (for any other
/// character) disambiguates every escape from every literal alnum run.
/// Empty input maps to `_` (a valid mermaid identifier that cannot be
/// produced by the encoding rules above, keeping injectivity intact even
/// for the empty string).
fn sanitize_id(raw: &str) -> String {
    if raw.is_empty() {
        return "_".to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch == '_' {
            out.push_str("__");
        } else {
            let _ = write!(out, "_{:x}_", ch as u32);
        }
    }
    out
}

/// Node identifier used in mermaid. Format: `L<layer_sanitized_len>_<sanitized_layer>_<sanitized_name>`.
///
/// A bare `<layer>_<name>` concatenation is not injective even with an
/// injective [`sanitize_id`], because escaped components can start or end
/// with `_`. For example `layer = "a_"` + `name = "b"` and
/// `layer = "a"` + `name = "_b"` both collapse to `a___b` with a plain
/// `_` separator. Length-prefixing the layer component makes the split
/// unambiguous: the first `<layer_sanitized_len>` characters after the
/// `L<N>_` prefix form the sanitized layer, and everything after the
/// trailing `_` is the sanitized name.
fn node_id(layer: &LayerId, name: &str) -> String {
    let l = sanitize_id(layer.as_ref());
    let n = sanitize_id(name);
    format!("L{}_{}_{}", l.len(), l, n)
}

/// Wrap an edge label in double quotes so mermaid does not misinterpret
/// shape-delimiter characters (`(`, `)`, `[`, `]`, `{`, `}`) that may
/// appear inside a method-name or param-name fragment. Double quotes and
/// pipes inside the label are escaped so the wrapping remains balanced
/// and the label cannot terminate its own `|...|` scope.
///
/// Mermaid treats a label bracketed by `"..."` as a literal string
/// (see <https://mermaid.js.org/syntax/flowchart.html#styling-a-node>),
/// so quoting is the safest way to carry `method(arg)` labels introduced
/// by the Phase 1.5 params-edge extension (ADR 2026-04-17-1528 §D4 (1)).
fn escape_edge_label(raw: &str) -> String {
    let escaped = raw.replace('"', "&quot;").replace('|', "&#124;");
    format!("\"{escaped}\"")
}

/// Sanitize a catalogue entry name for safe embedding inside any mermaid
/// node-shape label.
///
/// Shape delimiters (`]`, `)`, `}`, `/`, `\`) and the mermaid quoting
/// character (`"`) are replaced with `_`. All other characters — including
/// every character valid in a Rust type identifier — pass through unchanged,
/// so for well-formed catalogues this function is always a no-op.
///
/// A Rust struct/enum name is `[A-Za-z][A-Za-z0-9_]*`, which cannot
/// contain any of the replaced characters, so in normal usage the output
/// equals the input. The sanitisation acts as a defensive guard against
/// manually-crafted catalogue JSON that contains non-identifier names.
fn sanitize_node_label(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            ']' | ')' | '}' | '/' | '\\' | '"' => '_',
            other => other,
        })
        .collect()
}

/// Render the mermaid shape for an entry. Each variant of
/// [`TypeDefinitionKind`] maps to one of the shapes defined in ADR
/// 2026-04-17-1528 §D3.
fn node_shape(layer: &LayerId, entry: &TypeCatalogueEntry) -> String {
    let id = node_id(layer, entry.name());
    let name = sanitize_node_label(entry.name());
    match entry.kind() {
        TypeDefinitionKind::Typestate { .. } => format!("{id}([{name}])"),
        TypeDefinitionKind::Enum { .. } => format!("{id}{{{{{name}}}}}"),
        TypeDefinitionKind::ValueObject { .. } => format!("{id}({name})"),
        TypeDefinitionKind::ErrorType { .. } => format!("{id}>{name}]"),
        TypeDefinitionKind::SecondaryPort { .. } => format!("{id}[[{name}]]"),
        TypeDefinitionKind::SecondaryAdapter { .. } => {
            format!("{id}[{name}]:::secondary_adapter")
        }
        TypeDefinitionKind::ApplicationService { .. } => format!("{id}[/{name}\\]"),
        TypeDefinitionKind::UseCase { .. } => format!("{id}[/{name}/]"),
        TypeDefinitionKind::Interactor { .. } => format!("{id}[\\{name}/]"),
        TypeDefinitionKind::Dto { .. } => format!("{id}[{name}]"),
        TypeDefinitionKind::Command { .. } => format!("{id}[{name}]:::command"),
        TypeDefinitionKind::Query { .. } => format!("{id}[{name}]:::query"),
        TypeDefinitionKind::Factory { .. } => format!("{id}[{name}]:::factory"),
        TypeDefinitionKind::DomainService { .. } => format!("{id}[{name}]:::domain_service"),
        TypeDefinitionKind::FreeFunction { .. } => format!("{id}[{name}]:::free_function"),
    }
}

/// Returns the method declarations associated with an entry kind (empty
/// for kinds that carry none).
///
/// Three buckets:
/// 1. **Struct-based kinds with a single `expected_methods` source**
///    (Typestate / ValueObject / UseCase / Interactor / Dto / Command /
///    Query / Factory / DomainService / SecondaryPort / ApplicationService):
///    returns `expected_methods` from the top-level field directly.
/// 2. **`SecondaryAdapter`**: merges the top-level `expected_methods`
///    (direct struct methods) with each `implements[].expected_methods`
///    (trait impl methods). Both sources contribute to contract-map edges.
/// 3. **No-method kinds** (`Enum` / `ErrorType` / `FreeFunction`): returns
///    an empty `Vec`.
fn methods_of(kind: &TypeDefinitionKind) -> Vec<&MethodDeclaration> {
    match kind {
        // Unified arm: all struct-based kinds that carry a single top-level
        // `expected_methods` field (M1 / S1 uniformization, ADR 2026-04-28-0135).
        TypeDefinitionKind::Typestate { expected_methods, .. }
        | TypeDefinitionKind::ValueObject { expected_methods, .. }
        | TypeDefinitionKind::UseCase { expected_methods, .. }
        | TypeDefinitionKind::Interactor { expected_methods, .. }
        | TypeDefinitionKind::Dto { expected_methods, .. }
        | TypeDefinitionKind::Command { expected_methods, .. }
        | TypeDefinitionKind::Query { expected_methods, .. }
        | TypeDefinitionKind::Factory { expected_methods, .. }
        | TypeDefinitionKind::DomainService { expected_methods, .. }
        | TypeDefinitionKind::SecondaryPort { expected_methods }
        | TypeDefinitionKind::ApplicationService { expected_methods } => {
            expected_methods.iter().collect()
        }
        // Two-source merge: top-level struct methods + trait impl methods.
        TypeDefinitionKind::SecondaryAdapter { expected_methods, implements, .. } => {
            expected_methods
                .iter()
                .chain(implements.iter().flat_map(TraitImplDecl::expected_methods))
                .collect()
        }
        // No-method kinds.
        TypeDefinitionKind::Enum { .. }
        | TypeDefinitionKind::ErrorType { .. }
        | TypeDefinitionKind::FreeFunction { .. } => Vec::new(),
    }
}

/// Extract PascalCase type-name tokens from a type-string. This is the
/// domain-side twin of `infrastructure::tddd::type_graph_render::
/// extract_type_names` — keeping an independent copy here preserves the
/// `domain → infrastructure` no-dependency rule.
fn extract_type_names(ty: &str) -> Vec<&str> {
    ty.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .filter(|s| s.starts_with(char::is_uppercase))
        .collect()
}

#[cfg(test)]
#[path = "contract_map_render_tests.rs"]
mod tests;
