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

use crate::ConfidenceSignal;
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
    //    `type_index`  — all surviving entries (used for method-call edges).
    //    `port_index`  — only `SecondaryPort` entries (used for trait-impl
    //                    edges so that `-.impl.->` never accidentally targets a
    //                    same-named DTO/value-object).
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
    // `application_service_index` — only `ApplicationService` entries
    // (used for Interactor `-.impl.->` edges introduced in T003 / IN-02 /
    // ADR §D4 (2)). Same shadowing semantics as `port_index`: multiple
    // declarations of the same short name fan out to all matching targets.
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
    let mut out = String::new();
    out.push_str("```mermaid\n");
    out.push_str("flowchart LR\n");
    out.push_str("    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4\n");
    out.push_str("    classDef command fill:#e3f2fd,stroke:#1976d2\n");
    out.push_str("    classDef query fill:#f3e5f5,stroke:#8e24aa\n");
    out.push_str("    classDef factory fill:#fff8e1,stroke:#f9a825\n");
    out.push_str("    classDef free_function fill:#e8eaf6,stroke:#3949ab\n");
    // T005 / IN-03 / IN-04: dashed-border classDefs for the L1 / L2
    // categories. `unused_reference` flags `action=reference` entries that
    // never appear as an edge source or target (forward-reference
    // placeholders); `declaration_only` flags `action=modify` entries whose
    // expected_methods list is empty (declarative-only modifications).
    out.push_str("    classDef unused_reference stroke-dasharray: 4 4\n");
    out.push_str("    classDef declaration_only stroke-dasharray: 4 4\n");
    // T007 / IN-06 / ADR §D5: action overlay classDefs. Only emitted when
    // `opts.action_overlay = true`. `add_action` is the default and carries
    // no classDef (preserves existing visual baseline for unmarked entries).
    if opts.action_overlay {
        out.push_str("    classDef modify_action stroke-dasharray: 4 4\n");
        out.push_str(
            "    classDef delete_action fill:#f5f5f5,stroke:#9e9e9e,color:#9e9e9e,stroke-dasharray: 6 2\n",
        );
        out.push_str("    classDef reference_action stroke-dasharray: 2 4\n");
    }
    // T008 / IN-07 / ADR §D5: signal overlay classDefs. Only emitted when
    // `opts.signal_overlay = true` AND the per-layer `signals()` payload
    // contains at least one Yellow or Red signal. `Blue` keeps the default
    // colour. When a layer's `signals()` is `None`, that layer contributes
    // nothing to either the header gate or the per-node annotations
    // (complete silence — not "treat as Blue with classDef").
    let has_signal_to_show = opts.signal_overlay
        && entries.iter().any(|(layer, entry)| {
            catalogues
                .get(*layer)
                .and_then(TypeCatalogueDocument::signals)
                .map(|sigs| {
                    sigs.iter().any(|s| {
                        s.type_name() == entry.name()
                            && s.kind_tag() == entry.kind().kind_tag()
                            && matches!(
                                s.signal(),
                                ConfidenceSignal::Yellow | ConfidenceSignal::Red
                            )
                    })
                })
                .unwrap_or(false)
        });
    if has_signal_to_show {
        out.push_str("    classDef yellow_signal fill:#fff3e0\n");
        out.push_str("    classDef red_signal fill:#ffebee\n");
    }

    // 4a. Subgraphs.
    for layer in &active_layers {
        let label = sanitize_id(layer.as_ref());
        let _ = writeln!(out, "    subgraph {label} [{raw}]", raw = layer.as_ref());
        for (entry_layer, entry) in &entries {
            if entry_layer == layer {
                let mut shape = node_shape(layer, entry, opts.action_overlay);
                // T008: append `:::yellow_signal` / `:::red_signal` for entries
                // whose looked-up signal is Yellow / Red. Only when overlay is
                // enabled AND the layer's `signals()` payload exists.
                if opts.signal_overlay {
                    if let Some(sigs) =
                        catalogues.get(*layer).and_then(TypeCatalogueDocument::signals)
                    {
                        // Lookup by (type_name, kind_tag) — name-only match
                        // would mis-select a signal record in a delete+add
                        // migration pair where the same name is declared with
                        // two different kinds (PR #115 P1 finding).
                        if let Some(s) = sigs.iter().find(|s| {
                            s.type_name() == entry.name() && s.kind_tag() == entry.kind().kind_tag()
                        }) {
                            match s.signal() {
                                ConfidenceSignal::Yellow => shape.push_str(":::yellow_signal"),
                                ConfidenceSignal::Red => shape.push_str(":::red_signal"),
                                _ => {}
                            }
                        }
                    }
                }
                let _ = writeln!(out, "        {shape}");
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
    // T005 / IN-03: track every node id that appears as an edge source or
    // target so the post-loop classDef pass can flag `action=reference`
    // entries that never participate in any edge as `unused_reference`.
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
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
                        used_ids.insert(src_id.clone());
                        used_ids.insert(dst_id.clone());
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
                            used_ids.insert(src_id.clone());
                            used_ids.insert(dst_id.clone());
                        }
                    }
                }
            }
        }

        if let TypeDefinitionKind::SecondaryAdapter { implements } = entry.kind() {
            for impl_decl in implements {
                // Use `port_index` (SecondaryPort entries only) so that
                // `-.impl.->` is never drawn to a same-named non-port entry
                // (ADR §D4: trait-impl edge targets secondary_port nodes only).
                if let Some(port_ids) = port_index.get(impl_decl.trait_name()) {
                    for port_id in port_ids {
                        edges.insert(format!("    {src_id} -.impl.-> {port_id}"));
                        used_ids.insert(src_id.clone());
                        used_ids.insert(port_id.clone());
                    }
                }
            }
        }

        // T003 / IN-02 / ADR §D4 (2): Interactor → ApplicationService
        // impl edge. Only emitted when (1) the entry kind is `Interactor`,
        // (2) `declares_application_service` is `Some(name)`, and (3) `name`
        // resolves in `application_service_index` (same fan-out semantics
        // as `port_index` so shadowed declarations across layers don't drop).
        if let TypeDefinitionKind::Interactor { declares_application_service: Some(name) } =
            entry.kind()
        {
            if let Some(svc_ids) = application_service_index.get(name) {
                for svc_id in svc_ids {
                    edges.insert(format!("    {src_id} -.impl.-> {svc_id}"));
                    used_ids.insert(src_id.clone());
                    used_ids.insert(svc_id.clone());
                }
            }
        }

        // T006 / IN-05 / IN-08 / CN-05: field edges from `expected_members`.
        // Only emit for the four field-bearing kinds (Dto / Command / Query /
        // ValueObject) per CN-05. The constructor-side gate
        // (`TypeCatalogueEntry::with_members`) already keeps `expected_members`
        // empty for other kinds, so `field_members()` is structurally empty on
        // them; the explicit kind match here is a defence-in-depth read-side
        // filter against externally-constructed entries.
        if matches!(
            entry.kind(),
            TypeDefinitionKind::Dto
                | TypeDefinitionKind::Command
                | TypeDefinitionKind::Query
                | TypeDefinitionKind::ValueObject
        ) {
            for (field_name, field_ty) in entry.field_members() {
                for token in extract_type_names(field_ty) {
                    if let Some(dsts) = type_index.get(token) {
                        for (_dst_layer, dst_id) in dsts {
                            if dst_id == &src_id {
                                continue;
                            }
                            edges.insert(format!(
                                "    {src_id} -->|{label}| {dst_id}",
                                label = escape_edge_label(&format!(".{field_name}")),
                            ));
                            used_ids.insert(src_id.clone());
                            used_ids.insert(dst_id.clone());
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

    // T005 / IN-03 / IN-04: classDef applications for L1 / L2 categories.
    //
    // - `unused_reference` (IN-03 / L1): `action=reference` AND the node id
    //   never appears in any edge → render with dashed border to mark the
    //   entry as an intentional forward-reference placeholder.
    // - `declaration_only` (IN-04 / L2): `action=modify` AND `methods_of(kind)`
    //   is empty → render with dashed border to mark the entry as a
    //   declarative-only modification (no method-level changes), regardless
    //   of whether other edges reach the node.
    let mut class_lines: BTreeSet<String> = BTreeSet::new();
    for (layer, entry) in &entries {
        let id = node_id(layer, entry.name());
        if entry.action() == crate::tddd::catalogue::TypeAction::Reference
            && !used_ids.contains(&id)
        {
            class_lines.insert(format!("    class {id} unused_reference"));
        }
        // declaration_only gates on `is_method_bearing_kind` (not just
        // `methods_of(...).is_empty()`) — non-method-bearing kinds (Dto,
        // Enum, etc.) can have other genuine structural deltas like
        // `expected_members` or variant changes, so they must NOT be
        // marked declaration-only just because their methods list is
        // structurally absent (PR #115 P1 finding).
        if entry.action() == crate::tddd::catalogue::TypeAction::Modify
            && crate::tddd::catalogue::is_method_bearing_kind(entry.kind())
            && methods_of(entry.kind()).is_empty()
        {
            class_lines.insert(format!("    class {id} declaration_only"));
        }
    }
    for line in &class_lines {
        out.push_str(line);
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
/// [`TypeDefinitionKind`] maps to one of 14 shapes defined in ADR
/// 2026-04-17-1528 §D3. When `action_overlay=true`, an action-specific
/// classDef (`:::modify_action` / `:::delete_action` / `:::reference_action`)
/// is appended for non-`Add` actions per ADR §D5 / IN-06.
fn node_shape(layer: &LayerId, entry: &TypeCatalogueEntry, action_overlay: bool) -> String {
    let id = node_id(layer, entry.name());
    let name = sanitize_node_label(entry.name());
    let base = match entry.kind() {
        TypeDefinitionKind::Typestate { .. } => format!("{id}([{name}])"),
        TypeDefinitionKind::Enum { .. } => format!("{id}{{{{{name}}}}}"),
        TypeDefinitionKind::ValueObject => format!("{id}({name})"),
        TypeDefinitionKind::ErrorType { .. } => format!("{id}>{name}]"),
        TypeDefinitionKind::SecondaryPort { .. } => format!("{id}[[{name}]]"),
        TypeDefinitionKind::SecondaryAdapter { .. } => {
            format!("{id}[{name}]:::secondary_adapter")
        }
        TypeDefinitionKind::ApplicationService { .. } => format!("{id}[/{name}\\]"),
        TypeDefinitionKind::UseCase => format!("{id}[/{name}/]"),
        TypeDefinitionKind::Interactor { .. } => format!("{id}[\\{name}/]"),
        TypeDefinitionKind::Dto => format!("{id}[{name}]"),
        TypeDefinitionKind::Command => format!("{id}[{name}]:::command"),
        TypeDefinitionKind::Query => format!("{id}[{name}]:::query"),
        TypeDefinitionKind::Factory => format!("{id}[{name}]:::factory"),
        TypeDefinitionKind::FreeFunction { .. } => format!("{id}[{name}]:::free_function"),
    };
    if action_overlay {
        match entry.action() {
            crate::tddd::catalogue::TypeAction::Add => base,
            crate::tddd::catalogue::TypeAction::Modify => format!("{base}:::modify_action"),
            crate::tddd::catalogue::TypeAction::Delete => format!("{base}:::delete_action"),
            crate::tddd::catalogue::TypeAction::Reference => format!("{base}:::reference_action"),
        }
    } else {
        base
    }
}

/// Returns the method declarations associated with an entry kind (empty
/// for kinds that carry none).
fn methods_of(kind: &TypeDefinitionKind) -> Vec<&MethodDeclaration> {
    match kind {
        TypeDefinitionKind::SecondaryPort { expected_methods }
        | TypeDefinitionKind::ApplicationService { expected_methods } => {
            expected_methods.iter().collect()
        }
        TypeDefinitionKind::SecondaryAdapter { implements } => {
            implements.iter().flat_map(TraitImplDecl::expected_methods).collect()
        }
        TypeDefinitionKind::Typestate { .. }
        | TypeDefinitionKind::Enum { .. }
        | TypeDefinitionKind::ValueObject
        | TypeDefinitionKind::ErrorType { .. }
        | TypeDefinitionKind::UseCase
        | TypeDefinitionKind::Interactor { .. }
        | TypeDefinitionKind::Dto
        | TypeDefinitionKind::Command
        | TypeDefinitionKind::Query
        | TypeDefinitionKind::Factory
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::catalogue::{
        MemberDeclaration, MethodDeclaration, ParamDeclaration, TraitImplDecl, TypeAction,
        TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind, TypeSignal,
        TypestateTransitions,
    };

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    fn entry(name: &str, kind: TypeDefinitionKind) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(name, format!("{name} description"), kind, TypeAction::Add, true)
            .unwrap()
    }

    fn doc(entries: Vec<TypeCatalogueEntry>) -> TypeCatalogueDocument {
        TypeCatalogueDocument::new(2, entries)
    }

    fn simple_3layer_catalogues() -> (BTreeMap<LayerId, TypeCatalogueDocument>, Vec<LayerId>) {
        let domain = layer("domain");
        let usecase = layer("usecase");
        let infra = layer("infrastructure");

        let user_repository_methods = vec![MethodDeclaration::new(
            "save",
            Some("&self".to_owned()),
            vec![ParamDeclaration::new("user", "User")],
            "Result<(), DomainError>",
            false,
        )];

        let register_user_methods = vec![MethodDeclaration::new(
            "execute",
            Some("&self".to_owned()),
            vec![],
            "Result<User, DomainError>",
            false,
        )];

        let postgres_impl = TraitImplDecl::new("UserRepository", Vec::new());

        let domain_doc = doc(vec![
            entry(
                "UserRepository",
                TypeDefinitionKind::SecondaryPort { expected_methods: user_repository_methods },
            ),
            entry(
                "User",
                TypeDefinitionKind::Typestate {
                    transitions: TypestateTransitions::To(vec!["VerifiedUser".to_owned()]),
                },
            ),
            entry("DomainError", TypeDefinitionKind::ErrorType { expected_variants: vec![] }),
        ]);
        let usecase_doc = doc(vec![
            entry(
                "RegisterUser",
                TypeDefinitionKind::ApplicationService { expected_methods: register_user_methods },
            ),
            entry("RegisterUserCommand", TypeDefinitionKind::Command),
        ]);
        let infra_doc = doc(vec![entry(
            "PostgresUserRepository",
            TypeDefinitionKind::SecondaryAdapter { implements: vec![postgres_impl] },
        )]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        catalogues.insert(usecase.clone(), usecase_doc);
        catalogues.insert(infra.clone(), infra_doc);
        let layer_order = vec![domain, usecase, infra];
        (catalogues, layer_order)
    }

    #[test]
    fn test_render_contract_map_emits_fenced_mermaid_block() {
        let (catalogues, order) = simple_3layer_catalogues();
        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        assert!(text.starts_with("```mermaid\n"), "output must start with mermaid fence");
        assert!(text.trim_end().ends_with("```"), "output must end with closing fence");
        assert!(text.contains("flowchart LR"));
    }

    #[test]
    fn test_render_contract_map_produces_subgraph_per_layer_in_order() {
        let (catalogues, order) = simple_3layer_catalogues();
        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        let domain_pos = text.find("subgraph domain [domain]").unwrap();
        let usecase_pos = text.find("subgraph usecase [usecase]").unwrap();
        let infra_pos = text.find("subgraph infrastructure [infrastructure]").unwrap();
        assert!(domain_pos < usecase_pos);
        assert!(usecase_pos < infra_pos);
    }

    #[test]
    fn test_render_contract_map_emits_14_shape_variants_correctly() {
        let l = layer("sample");
        let entries = vec![
            entry(
                "TState",
                TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal },
            ),
            entry("EKind", TypeDefinitionKind::Enum { expected_variants: vec![] }),
            entry("Vo", TypeDefinitionKind::ValueObject),
            entry("Err", TypeDefinitionKind::ErrorType { expected_variants: vec![] }),
            entry("SPort", TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }),
            entry("SAdap", TypeDefinitionKind::SecondaryAdapter { implements: vec![] }),
            entry("AppSvc", TypeDefinitionKind::ApplicationService { expected_methods: vec![] }),
            entry("UCase", TypeDefinitionKind::UseCase),
            entry("Intc", TypeDefinitionKind::Interactor { declares_application_service: None }),
            entry("DtoK", TypeDefinitionKind::Dto),
            entry("CmdK", TypeDefinitionKind::Command),
            entry("QryK", TypeDefinitionKind::Query),
            entry("FactK", TypeDefinitionKind::Factory),
            entry(
                "FreeFn",
                TypeDefinitionKind::FreeFunction {
                    expected_params: vec![],
                    expected_returns: vec![],
                },
            ),
        ];

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(l.clone(), doc(entries));

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&l),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();

        assert!(text.contains("L6_sample_TState([TState])"), "typestate stadium shape");
        assert!(text.contains("L6_sample_EKind{{EKind}}"), "enum hexagon shape");
        assert!(text.contains("L6_sample_Vo(Vo)"), "value_object round shape");
        assert!(text.contains("L6_sample_Err>Err]"), "error_type flag shape");
        assert!(text.contains("L6_sample_SPort[[SPort]]"), "secondary_port subroutine shape");
        assert!(
            text.contains("L6_sample_SAdap[SAdap]:::secondary_adapter"),
            "secondary_adapter rect + classDef"
        );
        assert!(text.contains("L6_sample_AppSvc[/AppSvc\\]"), "application_service parallelogram");
        assert!(text.contains("L6_sample_UCase[/UCase/]"), "use_case parallelogram-alt");
        assert!(text.contains("L6_sample_Intc[\\Intc/]"), "interactor trapezoid-alt");
        assert!(text.contains("L6_sample_DtoK[DtoK]"), "dto rect");
        assert!(text.contains("L6_sample_CmdK[CmdK]:::command"), "command rect + classDef");
        assert!(text.contains("L6_sample_QryK[QryK]:::query"), "query rect + classDef");
        assert!(text.contains("L6_sample_FactK[FactK]:::factory"), "factory rect + classDef");
        assert!(
            text.contains("L6_sample_FreeFn[FreeFn]:::free_function"),
            "free_function rect + classDef"
        );
    }

    #[test]
    fn test_render_contract_map_draws_method_call_edges_across_layers() {
        let (catalogues, order) = simple_3layer_catalogues();
        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        // `UserRepository.save(&self, user: User) -> Result<(), DomainError>`
        // should yield edges to `DomainError` (User excluded because it is
        // the same layer receiver — the extractor keeps it, but the edge
        // still goes through: assert on DomainError only, which is
        // unambiguous.)
        assert!(
            text.contains("L6_domain_UserRepository -->|\"save\"| L6_domain_DomainError"),
            "method edge to DomainError must appear; output was:\n{text}"
        );
        // `RegisterUser.execute() -> Result<User, DomainError>` spans
        // usecase → domain.
        assert!(
            text.contains("L7_usecase_RegisterUser -->|\"execute\"| L6_domain_User"),
            "cross-layer method edge to User must appear; output was:\n{text}"
        );
        assert!(
            text.contains("L7_usecase_RegisterUser -->|\"execute\"| L6_domain_DomainError"),
            "cross-layer method edge to DomainError must appear"
        );
    }

    #[test]
    fn test_render_contract_map_draws_trait_impl_edges_as_dashed() {
        let (catalogues, order) = simple_3layer_catalogues();
        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        assert!(
            text.contains(
                "L14_infrastructure_PostgresUserRepository -.impl.-> L6_domain_UserRepository"
            ),
            "trait impl edge must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_respects_kind_filter() {
        let (catalogues, order) = simple_3layer_catalogues();
        let opts = ContractMapRenderOptions {
            kind_filter: Some(vec![TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }]),
            ..ContractMapRenderOptions::default()
        };
        let content = render_contract_map(&catalogues, &order, &opts);
        let text = content.as_ref();
        assert!(text.contains("L6_domain_UserRepository[[UserRepository]]"));
        // Use shape-specific substrings so `L6_domain_UserRepository` does not
        // accidentally satisfy a `L6_domain_User` prefix match.
        assert!(!text.contains("L6_domain_User([User])"), "User should be filtered out");
        assert!(
            !text.contains("L6_domain_DomainError>DomainError]"),
            "DomainError should be filtered out"
        );
        assert!(
            !text.contains("L7_usecase_RegisterUser[/RegisterUser\\]"),
            "RegisterUser should be filtered out"
        );
    }

    #[test]
    fn test_render_contract_map_kind_filter_empty_vec_returns_empty_subgraphs() {
        let (catalogues, order) = simple_3layer_catalogues();
        let opts = ContractMapRenderOptions {
            kind_filter: Some(Vec::new()),
            ..ContractMapRenderOptions::default()
        };
        let content = render_contract_map(&catalogues, &order, &opts);
        let text = content.as_ref();
        // Subgraphs are still emitted (one per layer) but carry no nodes.
        assert!(text.contains("subgraph domain [domain]"));
        assert!(text.contains("    end"));
        assert!(!text.contains("UserRepository"), "no entries should be rendered");
    }

    #[test]
    fn test_render_contract_map_respects_layers_subset() {
        let (catalogues, order) = simple_3layer_catalogues();
        let opts = ContractMapRenderOptions {
            layers: vec![layer("domain")],
            ..ContractMapRenderOptions::default()
        };
        let content = render_contract_map(&catalogues, &order, &opts);
        let text = content.as_ref();
        assert!(text.contains("subgraph domain [domain]"));
        assert!(!text.contains("subgraph usecase"));
        assert!(!text.contains("subgraph infrastructure"));
    }

    #[test]
    fn test_render_contract_map_action_overlay_all_add_entries_only_adds_classdefs() {
        // `simple_3layer_catalogues` entries are all `TypeAction::Add`.
        // When action_overlay=true the 3 action classDef header lines
        // (modify_action / delete_action / reference_action) are added, but
        // no node annotations change because Add is the default and carries no
        // extra suffix. The two outputs are therefore NOT equal — they differ
        // by exactly those 3 classDef lines.
        let (catalogues, order) = simple_3layer_catalogues();
        let baseline = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let overlay_on = ContractMapRenderOptions {
            action_overlay: true,
            ..ContractMapRenderOptions::default()
        };
        let with_overlay = render_contract_map(&catalogues, &order, &overlay_on);
        // The header section differs.
        assert_ne!(
            baseline.as_ref(),
            with_overlay.as_ref(),
            "action_overlay=true must add 3 classDef lines to the header"
        );
        // The 3 action classDefs are present in the overlay output.
        assert!(with_overlay.as_ref().contains("classDef modify_action"));
        assert!(with_overlay.as_ref().contains("classDef delete_action"));
        assert!(with_overlay.as_ref().contains("classDef reference_action"));
        // No node carries an action annotation (all entries are Add).
        assert!(!with_overlay.as_ref().contains(":::modify_action"));
        assert!(!with_overlay.as_ref().contains(":::delete_action"));
        assert!(!with_overlay.as_ref().contains(":::reference_action"));
    }

    #[test]
    fn test_render_contract_map_hyphenated_layer_id_sanitized_in_ids() {
        // layer-id with hyphen ("my-gateway") must render into mermaid IDs
        // that are distinct from an identically-spelled underscore variant
        // ("my_gateway"). The injective `sanitize_id` encodes `-` (U+002D)
        // as `_2d_` and `_` (U+005F) as `__`, so the two inputs are
        // guaranteed to yield different node prefixes.
        let gateway = layer("my-gateway");
        let d = doc(vec![entry("Foo", TypeDefinitionKind::ValueObject)]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(gateway.clone(), d);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&gateway),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // Label preserves original hyphen; id encodes `-` as `_2d_`.
        assert!(text.contains("subgraph my_2d_gateway [my-gateway]"));
        assert!(text.contains("L13_my_2d_gateway_Foo(Foo)"));
    }

    #[test]
    fn test_render_contract_map_sanitize_id_is_injective_for_hyphen_vs_underscore() {
        // Regression: before the injective encoding, layer ids `my-gateway`
        // and `my_gateway` both collapsed to `my_gateway` and produced
        // undistinguishable subgraphs. After the fix the hyphen form
        // becomes `my_2d_gateway` and the underscore form becomes
        // `my__gateway`, so the two render targets can never alias.
        let hyphen = layer("my-gateway");
        let underscore = layer("my_gateway");
        let d_hyphen = doc(vec![entry("Foo", TypeDefinitionKind::ValueObject)]);
        let d_underscore = doc(vec![entry("Bar", TypeDefinitionKind::ValueObject)]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(hyphen.clone(), d_hyphen);
        catalogues.insert(underscore.clone(), d_underscore);
        let order = vec![hyphen, underscore];

        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        assert!(
            text.contains("subgraph my_2d_gateway [my-gateway]"),
            "hyphen layer subgraph id must be `my_2d_gateway`; output was:\n{text}"
        );
        assert!(
            text.contains("subgraph my__gateway [my_gateway]"),
            "underscore layer subgraph id must be `my__gateway`; output was:\n{text}"
        );
        assert!(
            text.contains("L13_my_2d_gateway_Foo(Foo)"),
            "Foo node id must be prefixed with hyphen-encoded layer id"
        );
        assert!(
            text.contains("L11_my__gateway_Bar(Bar)"),
            "Bar node id must be prefixed with underscore-encoded layer id"
        );
    }

    #[test]
    fn test_render_contract_map_is_pure_and_deterministic() {
        let (catalogues, order) = simple_3layer_catalogues();
        let a = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let b = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        assert_eq!(a.as_ref(), b.as_ref(), "render must be deterministic");
    }

    // --- Phase 1.5: param-derived method-edges (ADR §D4 (1) extended) ---

    #[test]
    fn test_render_contract_map_emits_param_edge_within_layer() {
        // `UserRepository.save(&self, user: User) -> Result<(), DomainError>`
        // must emit a same-layer edge to `User` labelled `save(user)` in
        // addition to the returns-derived edge to `DomainError`.
        let (catalogues, order) = simple_3layer_catalogues();
        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        assert!(
            text.contains("L6_domain_UserRepository -->|\"save(user)\"| L6_domain_User"),
            "param edge to User must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_emits_param_edge_across_layers() {
        // Add a usecase-layer application_service whose execute method
        // takes a domain value-object parameter. The edge must span
        // usecase → domain.
        let domain = layer("domain");
        let usecase = layer("usecase");

        let exec_method = vec![MethodDeclaration::new(
            "execute",
            Some("&self".to_owned()),
            vec![ParamDeclaration::new("subject", "Subject")],
            "()",
            false,
        )];

        let domain_doc = doc(vec![entry("Subject", TypeDefinitionKind::ValueObject)]);
        let usecase_doc = doc(vec![entry(
            "Greeter",
            TypeDefinitionKind::ApplicationService { expected_methods: exec_method },
        )]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        catalogues.insert(usecase.clone(), usecase_doc);
        let order = vec![domain, usecase];

        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        assert!(
            text.contains("L7_usecase_Greeter -->|\"execute(subject)\"| L6_domain_Subject"),
            "cross-layer param edge to Subject must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_ignores_param_referencing_undeclared_type() {
        // Param ty that references an external / undeclared type must NOT
        // produce an edge (external type is absent from the type_index).
        let domain = layer("domain");
        let exec_method = vec![MethodDeclaration::new(
            "take",
            Some("&self".to_owned()),
            vec![ParamDeclaration::new("path", "std::path::PathBuf")],
            "()",
            false,
        )];
        let domain_doc = doc(vec![entry(
            "Service",
            TypeDefinitionKind::ApplicationService { expected_methods: exec_method },
        )]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            !text.contains("-->|take(path)|"),
            "no edge should be emitted for undeclared type 'PathBuf'; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_param_edge_label_format_is_method_arg() {
        // Label format must be exactly `method(arg_name)` — verifies we
        // do not accidentally emit `method` (collision with returns edge)
        // or `method(Type)` (leaking the param type) for params.
        let domain = layer("domain");

        let ctor = vec![MethodDeclaration::new(
            "configure",
            Some("&self".to_owned()),
            vec![ParamDeclaration::new("settings", "Settings")],
            "()",
            false,
        )];
        let domain_doc = doc(vec![
            entry("App", TypeDefinitionKind::ApplicationService { expected_methods: ctor }),
            entry("Settings", TypeDefinitionKind::ValueObject),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // Present: exact `configure(settings)` label, wrapped in the
        // double-quote fence that isolates shape-delimiter characters
        // from mermaid's flowchart parser.
        assert!(
            text.contains("L6_domain_App -->|\"configure(settings)\"| L6_domain_Settings"),
            "label must be quoted 'configure(settings)'; output was:\n{text}"
        );
        // Absent: bare `configure` (would indicate edge was keyed from
        // returns, not params).
        assert!(
            !text.contains("L6_domain_App -->|\"configure\"| L6_domain_Settings"),
            "bare label must not appear for params-only edge; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_fans_out_edges_when_short_name_shadowed_across_layers() {
        // Regression: earlier the `type_index` was keyed by short name with
        // first-wins semantics, so a method returning `Error` (when both
        // `domain::Error` and `infrastructure::Error` were declared) only
        // reached the first-declared layer and silently dropped the other.
        // After the fix, both declarations participate in edge resolution.
        let domain = layer("domain");
        let infra = layer("infrastructure");

        let caller_methods = vec![MethodDeclaration::new(
            "run",
            Some("&self".to_owned()),
            vec![],
            "Result<(), Error>",
            false,
        )];

        let domain_doc = doc(vec![
            entry(
                "Caller",
                TypeDefinitionKind::ApplicationService { expected_methods: caller_methods },
            ),
            entry("Error", TypeDefinitionKind::ErrorType { expected_variants: vec![] }),
        ]);
        let infra_doc =
            doc(vec![entry("Error", TypeDefinitionKind::ErrorType { expected_variants: vec![] })]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        catalogues.insert(infra.clone(), infra_doc);
        let order = vec![domain, infra];

        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();

        assert!(
            text.contains("L6_domain_Caller -->|\"run\"| L6_domain_Error"),
            "edge to L6_domain_Error must appear; output was:\n{text}"
        );
        assert!(
            text.contains("L6_domain_Caller -->|\"run\"| L14_infrastructure_Error"),
            "edge to L14_infrastructure_Error must appear (shadowing must not drop declarations); output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_fans_out_trait_impl_when_port_name_shadowed() {
        // Same shadowing concern for `port_index`: if two layers declare a
        // `SecondaryPort` with the same short name, a `SecondaryAdapter`
        // whose `implements[].trait_name` matches must generate an
        // `-.impl.->` edge to each declaration.
        let domain = layer("domain");
        let infra = layer("infrastructure");

        let adapter_impl = TraitImplDecl::new("Port", Vec::new());

        let domain_doc = doc(vec![entry(
            "Port",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] },
        )]);
        let infra_doc = doc(vec![
            entry("Port", TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }),
            entry(
                "Adapter",
                TypeDefinitionKind::SecondaryAdapter { implements: vec![adapter_impl] },
            ),
        ]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        catalogues.insert(infra.clone(), infra_doc);
        let order = vec![domain, infra];

        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();

        assert!(
            text.contains("L14_infrastructure_Adapter -.impl.-> L6_domain_Port"),
            "trait-impl edge to L6_domain_Port must appear; output was:\n{text}"
        );
        assert!(
            text.contains("L14_infrastructure_Adapter -.impl.-> L14_infrastructure_Port"),
            "trait-impl edge to L14_infrastructure_Port must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_quotes_labels_for_mermaid_safety() {
        // Regression: before this fix, a `method(arg)` label leaked
        // literal parentheses into the `|...|` label scope, and mermaid
        // interpreted `(` as a node-shape opener, breaking rendering.
        // After the fix, every method edge label is wrapped in `"..."`
        // so shape delimiters never escape into the flowchart grammar.
        let (catalogues, order) = simple_3layer_catalogues();
        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        // The raw unescaped form must NOT appear anywhere (parse error).
        assert!(
            !text.contains("-->|save(user)|"),
            "unquoted `|save(user)|` must not appear (breaks mermaid); output was:\n{text}"
        );
        // The quoted form MUST appear for param edges.
        assert!(
            text.contains("-->|\"save(user)\"|"),
            "quoted `|\"save(user)\"|` must appear; output was:\n{text}"
        );
        // Returns-only edges are also quoted, keeping the emission rule
        // uniform across both code paths.
        assert!(
            text.contains("-->|\"save\"|"),
            "quoted `|\"save\"|` must appear for returns edges; output was:\n{text}"
        );
    }

    // --- T003 / IN-02 / ADR §D4 (2): Interactor → ApplicationService impl edge ---

    #[test]
    fn test_render_contract_map_emits_interactor_application_service_impl_edge() {
        // Cross-layer: usecase Interactor declares ApplicationService it implements;
        // renderer emits `-.impl.->` edge fanning to all matching ApplicationService entries.
        let usecase = layer("usecase");

        let usecase_doc = doc(vec![
            entry(
                "RegisterUser",
                TypeDefinitionKind::ApplicationService { expected_methods: vec![] },
            ),
            entry(
                "RegisterUserInteractor",
                TypeDefinitionKind::Interactor {
                    declares_application_service: Some("RegisterUser".to_owned()),
                },
            ),
        ]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(usecase.clone(), usecase_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&usecase),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();

        assert!(
            text.contains("L7_usecase_RegisterUserInteractor -.impl.-> L7_usecase_RegisterUser"),
            "Interactor → ApplicationService impl edge must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_no_impl_edge_when_application_service_unknown() {
        // declares_application_service points at a name not in any
        // ApplicationService entry → no edge emitted (not even to a same-named
        // non-ApplicationService entry).
        let usecase = layer("usecase");

        let usecase_doc = doc(vec![
            // `RegisterUser` is declared as a `Dto`, NOT as an ApplicationService.
            entry("RegisterUser", TypeDefinitionKind::Dto),
            entry(
                "RegisterUserInteractor",
                TypeDefinitionKind::Interactor {
                    declares_application_service: Some("RegisterUser".to_owned()),
                },
            ),
        ]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(usecase.clone(), usecase_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&usecase),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();

        assert!(
            !text.contains(" -.impl.-> "),
            "no impl edge expected when target is not an ApplicationService; output was:\n{text}"
        );
    }

    // --- T008 / IN-07 / ADR §D5: signal overlay ---

    fn doc_with_signals(
        entries: Vec<TypeCatalogueEntry>,
        signals: Vec<TypeSignal>,
    ) -> TypeCatalogueDocument {
        let mut d = doc(entries);
        d.set_signals(signals);
        d
    }

    #[test]
    fn test_render_contract_map_signal_overlay_emits_classdefs_and_annotations() {
        // signals() with Yellow + Red present → both classDefs in header, both
        // node annotations applied. Blue → no annotation.
        let domain = layer("domain");
        let entries = vec![
            entry("BlueOne", TypeDefinitionKind::ValueObject),
            entry("YellowOne", TypeDefinitionKind::ValueObject),
            entry("RedOne", TypeDefinitionKind::ValueObject),
        ];
        let signals = vec![
            TypeSignal::new(
                "BlueOne",
                "value_object",
                ConfidenceSignal::Blue,
                true,
                vec![],
                vec![],
                vec![],
            ),
            TypeSignal::new(
                "YellowOne",
                "value_object",
                ConfidenceSignal::Yellow,
                true,
                vec![],
                vec![],
                vec![],
            ),
            TypeSignal::new(
                "RedOne",
                "value_object",
                ConfidenceSignal::Red,
                true,
                vec![],
                vec![],
                vec![],
            ),
        ];
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), doc_with_signals(entries, signals));
        let opts = ContractMapRenderOptions { signal_overlay: true, ..Default::default() };
        let content = render_contract_map(&catalogues, std::slice::from_ref(&domain), &opts);
        let text = content.as_ref();
        assert!(text.contains("classDef yellow_signal"), "yellow_signal classDef must appear");
        assert!(text.contains("classDef red_signal"), "red_signal classDef must appear");
        assert!(
            text.contains("L6_domain_YellowOne(YellowOne):::yellow_signal"),
            "Yellow node must carry :::yellow_signal; output was:\n{text}"
        );
        assert!(
            text.contains("L6_domain_RedOne(RedOne):::red_signal"),
            "Red node must carry :::red_signal; output was:\n{text}"
        );
        assert!(
            !text.contains(":::yellow_signal\n        L6_domain_BlueOne")
                && !text.contains("L6_domain_BlueOne(BlueOne):::yellow_signal")
                && !text.contains("L6_domain_BlueOne(BlueOne):::red_signal"),
            "Blue node must NOT carry any signal classDef; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_signal_overlay_no_classdefs_when_all_blue() {
        // signal_overlay=true but all entries are Blue → no signal classDef
        // header lines (the gate fires only on at least one Yellow or Red).
        let domain = layer("domain");
        let entries = vec![entry("OnlyBlue", TypeDefinitionKind::ValueObject)];
        let signals = vec![TypeSignal::new(
            "OnlyBlue",
            "value_object",
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )];
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), doc_with_signals(entries, signals));
        let opts = ContractMapRenderOptions { signal_overlay: true, ..Default::default() };
        let content = render_contract_map(&catalogues, std::slice::from_ref(&domain), &opts);
        let text = content.as_ref();
        assert!(
            !text.contains("classDef yellow_signal"),
            "yellow_signal classDef must NOT appear when no Yellow/Red signal"
        );
        assert!(
            !text.contains("classDef red_signal"),
            "red_signal classDef must NOT appear when no Yellow/Red signal"
        );
    }

    #[test]
    fn test_render_contract_map_signal_overlay_silent_when_signals_none() {
        // signals() == None → signal_overlay=true output equals signal_overlay=false
        // output (complete silence — not "treat as Blue with classDef").
        let domain = layer("domain");
        let domain_doc = doc(vec![entry("NoSignals", TypeDefinitionKind::ValueObject)]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let opts_overlay = ContractMapRenderOptions { signal_overlay: true, ..Default::default() };
        let opts_off = ContractMapRenderOptions::empty();
        let with_overlay =
            render_contract_map(&catalogues, std::slice::from_ref(&domain), &opts_overlay);
        let without_overlay =
            render_contract_map(&catalogues, std::slice::from_ref(&domain), &opts_off);
        assert_eq!(
            with_overlay.as_ref(),
            without_overlay.as_ref(),
            "signals()=None must produce identical output regardless of signal_overlay flag"
        );
    }

    #[test]
    fn test_render_contract_map_signal_overlay_disabled_omits_classdefs_and_annotations() {
        let domain = layer("domain");
        let entries = vec![entry("YellowOne", TypeDefinitionKind::ValueObject)];
        let signals = vec![TypeSignal::new(
            "YellowOne",
            "value_object",
            ConfidenceSignal::Yellow,
            true,
            vec![],
            vec![],
            vec![],
        )];
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), doc_with_signals(entries, signals));
        let opts = ContractMapRenderOptions::empty();
        let content = render_contract_map(&catalogues, std::slice::from_ref(&domain), &opts);
        let text = content.as_ref();
        assert!(
            !text.contains("classDef yellow_signal"),
            "yellow_signal classDef must be absent when signal_overlay=false"
        );
        assert!(
            !text.contains(":::yellow_signal"),
            "signal annotation must be absent when signal_overlay=false"
        );
    }

    // --- T007 / IN-06 / ADR §D5: action overlay ---

    #[test]
    fn test_render_contract_map_action_overlay_classdefs_and_node_classes() {
        // 4 entries with each TypeAction → action_overlay=true emits 3
        // classDefs (modify/delete/reference; Add is default = no extra)
        // and the matching `:::action_class` annotations on each node.
        let domain = layer("domain");
        let entries = vec![
            TypeCatalogueEntry::new(
                "AddedThing",
                "added",
                TypeDefinitionKind::ValueObject,
                TypeAction::Add,
                true,
            )
            .unwrap(),
            TypeCatalogueEntry::new(
                "ModifiedThing",
                "modified",
                TypeDefinitionKind::ValueObject,
                TypeAction::Modify,
                true,
            )
            .unwrap(),
            TypeCatalogueEntry::new(
                "DeletedThing",
                "deleted",
                TypeDefinitionKind::ValueObject,
                TypeAction::Delete,
                true,
            )
            .unwrap(),
            TypeCatalogueEntry::new(
                "ReferencedThing",
                "reference",
                TypeDefinitionKind::ValueObject,
                TypeAction::Reference,
                true,
            )
            .unwrap(),
        ];
        let domain_doc = doc(entries);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let opts = ContractMapRenderOptions { action_overlay: true, ..Default::default() };
        let content = render_contract_map(&catalogues, std::slice::from_ref(&domain), &opts);
        let text = content.as_ref();

        assert!(text.contains("classDef modify_action"), "modify_action classDef must appear");
        assert!(text.contains("classDef delete_action"), "delete_action classDef must appear");
        assert!(
            text.contains("classDef reference_action"),
            "reference_action classDef must appear"
        );
        // Add → no `:::action_class` annotation
        assert!(
            text.contains("L6_domain_AddedThing(AddedThing)\n"),
            "Add entry must NOT carry an :::action_class annotation; output was:\n{text}"
        );
        // Other 3 → matching annotations
        assert!(
            text.contains("L6_domain_ModifiedThing(ModifiedThing):::modify_action"),
            "Modify entry must carry :::modify_action; output was:\n{text}"
        );
        assert!(
            text.contains("L6_domain_DeletedThing(DeletedThing):::delete_action"),
            "Delete entry must carry :::delete_action; output was:\n{text}"
        );
        assert!(
            text.contains("L6_domain_ReferencedThing(ReferencedThing):::reference_action"),
            "Reference entry must carry :::reference_action; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_action_overlay_disabled_omits_classdefs_and_annotations() {
        // action_overlay=false → no action-overlay classDef header lines and
        // no `:::*_action` annotations. The unrelated dashed-border classDefs
        // (unused_reference / declaration_only from T005) remain present.
        let domain = layer("domain");
        let domain_doc = doc(vec![
            TypeCatalogueEntry::new(
                "ModifiedThing",
                "m",
                TypeDefinitionKind::ValueObject,
                TypeAction::Modify,
                true,
            )
            .unwrap(),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            !text.contains("classDef modify_action"),
            "action overlay classDef must be absent; output was:\n{text}"
        );
        assert!(
            !text.contains(":::modify_action"),
            "action overlay annotation must be absent; output was:\n{text}"
        );
        // Unrelated T005 classDefs remain present unconditionally.
        assert!(
            text.contains("classDef unused_reference"),
            "unused_reference classDef must remain present (T005 is independent of action_overlay flag)"
        );
    }

    // --- T006 / IN-05 / CN-05: field edges from expected_members ---

    #[test]
    fn test_render_contract_map_field_edge_same_layer() {
        // Dto with a Field referencing a same-layer ValueObject → field edge emitted.
        let domain = layer("domain");
        let user_id_entry = entry("UserId", TypeDefinitionKind::ValueObject);
        let user_dto = entry("UserDto", TypeDefinitionKind::Dto)
            .with_members(vec![MemberDeclaration::field("id", "UserId")])
            .unwrap();
        let domain_doc = doc(vec![user_id_entry, user_dto]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            text.contains("L6_domain_UserDto -->|\".id\"| L6_domain_UserId"),
            "same-layer field edge to UserId must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_field_edge_cross_layer() {
        // usecase Command with a Field referencing a domain ValueObject.
        let domain = layer("domain");
        let usecase = layer("usecase");
        let user_id = entry("UserId", TypeDefinitionKind::ValueObject);
        let cmd = entry("CreateUserCommand", TypeDefinitionKind::Command)
            .with_members(vec![MemberDeclaration::field("user_id", "UserId")])
            .unwrap();
        let domain_doc = doc(vec![user_id]);
        let usecase_doc = doc(vec![cmd]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        catalogues.insert(usecase.clone(), usecase_doc);
        let content = render_contract_map(
            &catalogues,
            &[domain, usecase],
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            text.contains("L7_usecase_CreateUserCommand -->|\".user_id\"| L6_domain_UserId"),
            "cross-layer field edge to UserId must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_no_field_edge_for_external_type() {
        // Field referencing an external type (String) absent from type_index → no edge.
        let domain = layer("domain");
        let dto = entry("Vo", TypeDefinitionKind::Dto)
            .with_members(vec![MemberDeclaration::field("name", "String")])
            .unwrap();
        let domain_doc = doc(vec![dto]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            !text.contains("-->|\".name\"|"),
            "no edge for external type 'String'; output was:\n{text}"
        );
    }

    #[test]
    fn test_with_members_rejects_method_bearing_kind() {
        // CN-05: SecondaryPort is method-bearing, not field-bearing → with_members rejects.
        let secondary_port_entry = TypeCatalogueEntry::new(
            "SomePort",
            "secondary port",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] },
            TypeAction::Add,
            true,
        )
        .unwrap();
        let result =
            secondary_port_entry.with_members(vec![MemberDeclaration::field("x", "SomeType")]);
        assert!(result.is_err(), "with_members must reject field on a method-bearing kind");
    }

    // --- T005 / IN-03 / IN-04: dashed-border classDefs ---

    #[test]
    fn test_render_contract_map_unused_reference_classdef_for_orphan_reference_entry() {
        let domain = layer("domain");
        let domain_doc = doc(vec![
            // Orphan reference entry — no edges in or out.
            TypeCatalogueEntry::new(
                "TaskId",
                "Forward-reference placeholder",
                TypeDefinitionKind::ValueObject,
                TypeAction::Reference,
                true,
            )
            .unwrap(),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            text.contains("classDef unused_reference"),
            "unused_reference classDef definition must appear; output was:\n{text}"
        );
        assert!(
            text.contains("class L6_domain_TaskId unused_reference"),
            "unused_reference class application must target TaskId; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_no_unused_reference_when_reference_entry_is_edge_target() {
        // Reference entry that IS an edge target → no unused_reference classDef.
        let domain = layer("domain");
        let exec_method =
            vec![MethodDeclaration::new("find", Some("&self".to_owned()), vec![], "TaskId", false)];
        let domain_doc = doc(vec![
            entry("Repo", TypeDefinitionKind::ApplicationService { expected_methods: exec_method }),
            TypeCatalogueEntry::new(
                "TaskId",
                "Reference but used as method return type",
                TypeDefinitionKind::ValueObject,
                TypeAction::Reference,
                true,
            )
            .unwrap(),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            !text.contains("class L6_domain_TaskId unused_reference"),
            "TaskId is an edge target — must NOT carry unused_reference classDef; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_declaration_only_classdef_for_modify_empty_methods() {
        // action=modify + method-bearing kind + expected_methods empty →
        // declaration_only classDef. The PR #115 fix narrowed the gate to
        // method-bearing kinds (SecondaryPort / ApplicationService);
        // SecondaryAdapter and non-method-bearing kinds (Dto / Enum / etc.)
        // are excluded: SecondaryAdapter carries `implements` + impl edges
        // (not expected_methods), and non-method-bearing kinds can have
        // other genuine deltas like expected_members / variants.
        let domain = layer("domain");
        let domain_doc = doc(vec![
            TypeCatalogueEntry::new(
                "DeclarativeOnlyPort",
                "Modified port without any method-level delta",
                TypeDefinitionKind::SecondaryPort { expected_methods: vec![] },
                TypeAction::Modify,
                true,
            )
            .unwrap(),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            text.contains("classDef declaration_only"),
            "declaration_only classDef definition must appear; output was:\n{text}"
        );
        assert!(
            text.contains("class L6_domain_DeclarativeOnlyPort declaration_only"),
            "declaration_only must apply to a method-bearing modify entry with empty methods; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_no_declaration_only_for_non_method_bearing_modify() {
        // PR #115 P1 fix: a non-method-bearing kind with action=Modify (e.g.,
        // ErrorType with empty variants, or Dto with empty members) MUST NOT
        // receive declaration_only — `methods_of(...)` is trivially empty for
        // these kinds and a real structural delta on `expected_variants` /
        // `expected_members` would make the dashed marker semantically
        // misleading.
        let domain = layer("domain");
        let domain_doc = doc(vec![
            TypeCatalogueEntry::new(
                "AppError",
                "Error type modify without variant declarations",
                TypeDefinitionKind::ErrorType { expected_variants: vec![] },
                TypeAction::Modify,
                true,
            )
            .unwrap(),
            TypeCatalogueEntry::new(
                "WidgetDto",
                "Dto modify without member declarations",
                TypeDefinitionKind::Dto,
                TypeAction::Modify,
                true,
            )
            .unwrap(),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            !text.contains("class L6_domain_AppError declaration_only"),
            "ErrorType (non-method-bearing) must NOT receive declaration_only; output was:\n{text}"
        );
        assert!(
            !text.contains("class L6_domain_WidgetDto declaration_only"),
            "Dto (non-method-bearing) must NOT receive declaration_only; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_no_declaration_only_for_modify_secondary_adapter() {
        // PR #115 r6 P1 fix: SecondaryAdapter with action=Modify and an empty
        // `implements` list must NOT receive declaration_only.  SecondaryAdapter
        // carries `implements` and impl edges — not `expected_methods` — so the
        // IN-04 empty-methods gate does not apply.  The adapter may still carry
        // real `implements` contract deltas that would be hidden by the
        // dashed-border overlay.
        //
        // `node_id` uses `l.len()` as the numeric prefix: "infra" (5 chars)
        // → prefix `L5_infra_`.  The positive assertion on the node shape
        // confirms the prefix is correct so the negative assertion cannot be
        // vacuously true.
        let infra = layer("infra");
        let infra_doc = doc(vec![
            TypeCatalogueEntry::new(
                "PostgresUserRepo",
                "Secondary adapter with no implements listed yet",
                TypeDefinitionKind::SecondaryAdapter { implements: vec![] },
                TypeAction::Modify,
                true,
            )
            .unwrap(),
        ]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(infra.clone(), infra_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&infra),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // Positive guard: the node must actually appear in the output so the
        // negative assertion below cannot be vacuously true from a wrong prefix.
        assert!(
            text.contains("L5_infra_PostgresUserRepo"),
            "node L5_infra_PostgresUserRepo must appear in output (prefix sanity); output was:\n{text}"
        );
        assert!(
            !text.contains("class L5_infra_PostgresUserRepo declaration_only"),
            "SecondaryAdapter (modify, empty implements) must NOT receive declaration_only; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_no_impl_edge_when_interactor_lacks_field() {
        // Interactor with `declares_application_service: None` produces
        // no impl edge (preserves legacy / existence-only behaviour).
        let usecase = layer("usecase");

        let usecase_doc = doc(vec![
            entry(
                "RegisterUser",
                TypeDefinitionKind::ApplicationService { expected_methods: vec![] },
            ),
            entry(
                "RegisterUserInteractor",
                TypeDefinitionKind::Interactor { declares_application_service: None },
            ),
        ]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(usecase.clone(), usecase_doc);
        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&usecase),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();

        assert!(
            !text.contains(" -.impl.-> "),
            "no impl edge expected when Interactor has no declares_application_service; output was:\n{text}"
        );
    }
}
