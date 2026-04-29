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
    let mut out = String::new();
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
fn methods_of(kind: &TypeDefinitionKind) -> Vec<&MethodDeclaration> {
    match kind {
        TypeDefinitionKind::SecondaryPort { expected_methods }
        | TypeDefinitionKind::ApplicationService { expected_methods } => {
            expected_methods.iter().collect()
        }
        TypeDefinitionKind::SecondaryAdapter { implements, .. } => {
            implements.iter().flat_map(TraitImplDecl::expected_methods).collect()
        }
        TypeDefinitionKind::Typestate { .. }
        | TypeDefinitionKind::Enum { .. }
        | TypeDefinitionKind::ValueObject { .. }
        | TypeDefinitionKind::ErrorType { .. }
        | TypeDefinitionKind::UseCase { .. }
        | TypeDefinitionKind::Interactor { .. }
        | TypeDefinitionKind::Dto { .. }
        | TypeDefinitionKind::Command { .. }
        | TypeDefinitionKind::Query { .. }
        | TypeDefinitionKind::Factory { .. }
        | TypeDefinitionKind::DomainService { .. }
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
        MethodDeclaration, ParamDeclaration, TraitImplDecl, TypeAction, TypeCatalogueDocument,
        TypeCatalogueEntry, TypeDefinitionKind, TypestateTransitions,
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
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry("DomainError", TypeDefinitionKind::ErrorType { expected_variants: vec![] }),
        ]);
        let usecase_doc = doc(vec![
            entry(
                "RegisterUser",
                TypeDefinitionKind::ApplicationService { expected_methods: register_user_methods },
            ),
            entry(
                "RegisterUserCommand",
                TypeDefinitionKind::Command {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
        ]);
        let infra_doc = doc(vec![entry(
            "PostgresUserRepository",
            TypeDefinitionKind::SecondaryAdapter {
                implements: vec![postgres_impl],
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
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
    fn test_render_contract_map_emits_15_shape_variants_correctly() {
        let l = layer("sample");
        let entries = vec![
            entry(
                "TState",
                TypeDefinitionKind::Typestate {
                    transitions: TypestateTransitions::Terminal,
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry("EKind", TypeDefinitionKind::Enum { expected_variants: vec![] }),
            entry(
                "Vo",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry("Err", TypeDefinitionKind::ErrorType { expected_variants: vec![] }),
            entry("SPort", TypeDefinitionKind::SecondaryPort { expected_methods: vec![] }),
            entry(
                "SAdap",
                TypeDefinitionKind::SecondaryAdapter {
                    implements: vec![],
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry("AppSvc", TypeDefinitionKind::ApplicationService { expected_methods: vec![] }),
            entry(
                "UCase",
                TypeDefinitionKind::UseCase {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "Intc",
                TypeDefinitionKind::Interactor {
                    expected_members: Vec::new(),
                    declares_application_service: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "DtoK",
                TypeDefinitionKind::Dto {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "CmdK",
                TypeDefinitionKind::Command {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "QryK",
                TypeDefinitionKind::Query {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "FactK",
                TypeDefinitionKind::Factory {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "DsvcK",
                TypeDefinitionKind::DomainService {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
            entry(
                "FFn",
                TypeDefinitionKind::FreeFunction {
                    module_path: None,
                    expected_params: Vec::new(),
                    expected_returns: Vec::new(),
                    expected_is_async: false,
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
            text.contains("L6_sample_DsvcK[DsvcK]:::domain_service"),
            "domain_service rect + classDef"
        );
        assert!(
            text.contains("classDef domain_service"),
            "domain_service classDef must be declared in diagram header"
        );
        assert!(
            text.contains("L6_sample_FFn[FFn]:::free_function"),
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
    fn test_render_contract_map_phase_2_3_stub_fields_do_not_alter_output() {
        let (catalogues, order) = simple_3layer_catalogues();
        let baseline = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let overlays_on = ContractMapRenderOptions {
            signal_overlay: true,
            action_overlay: true,
            include_spec_source_edges: true,
            ..ContractMapRenderOptions::default()
        };
        let with_overlays = render_contract_map(&catalogues, &order, &overlays_on);
        assert_eq!(
            baseline.as_ref(),
            with_overlays.as_ref(),
            "Phase 1 must treat the 3 overlay flags as inert stubs"
        );
    }

    #[test]
    fn test_render_contract_map_hyphenated_layer_id_sanitized_in_ids() {
        // layer-id with hyphen ("my-gateway") must render into mermaid IDs
        // that are distinct from an identically-spelled underscore variant
        // ("my_gateway"). The injective `sanitize_id` encodes `-` (U+002D)
        // as `_2d_` and `_` (U+005F) as `__`, so the two inputs are
        // guaranteed to yield different node prefixes.
        let gateway = layer("my-gateway");
        let d = doc(vec![entry(
            "Foo",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        )]);
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
        let d_hyphen = doc(vec![entry(
            "Foo",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        )]);
        let d_underscore = doc(vec![entry(
            "Bar",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        )]);

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

        let domain_doc = doc(vec![entry(
            "Subject",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        )]);
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
            entry(
                "Settings",
                TypeDefinitionKind::ValueObject {
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
            ),
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
                TypeDefinitionKind::SecondaryAdapter {
                    implements: vec![adapter_impl],
                    expected_members: Vec::new(),
                    expected_methods: Vec::new(),
                },
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

    // --- T009: Interactor → ApplicationService -.impl.-> edge (AC-10) ---

    #[test]
    fn test_render_contract_map_draws_interactor_to_application_service_impl_edge() {
        // AC-10: Interactor with `declares_application_service: ["UserManagement"]`
        // and an ApplicationService entry "UserManagement" → output contains the
        // dashed impl edge (ADR 2026-04-17-1528 §L3 resolved).
        let usecase = layer("usecase");

        let interactor_entry = entry(
            "RegisterUserInteractor",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["UserManagement".to_owned()],
                expected_methods: Vec::new(),
            },
        );
        let svc_entry = entry(
            "UserManagement",
            TypeDefinitionKind::ApplicationService { expected_methods: vec![] },
        );

        let usecase_doc = doc(vec![interactor_entry, svc_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(usecase.clone(), usecase_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&usecase),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            text.contains("L7_usecase_RegisterUserInteractor -.impl.-> L7_usecase_UserManagement"),
            "Interactor → ApplicationService impl edge must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_interactor_to_application_service_edge_cross_layer() {
        // AC-10 cross-layer variant: Interactor in usecase layer declares
        // ApplicationService that lives in a different layer — edge must
        // still be drawn using the index, not hardcoded layer names (CN-08).
        let app = layer("app");
        let core = layer("core");

        let interactor_entry = entry(
            "DoThingInteractor",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["DoThing".to_owned()],
                expected_methods: Vec::new(),
            },
        );
        let svc_entry =
            entry("DoThing", TypeDefinitionKind::ApplicationService { expected_methods: vec![] });

        let app_doc = doc(vec![interactor_entry]);
        let core_doc = doc(vec![svc_entry]);

        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(app.clone(), app_doc);
        catalogues.insert(core.clone(), core_doc);
        let order = vec![core, app];

        let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
        let text = content.as_ref();
        assert!(
            text.contains("L3_app_DoThingInteractor -.impl.-> L4_core_DoThing"),
            "cross-layer interactor → application_service edge must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_interactor_missing_application_service_emits_no_broken_edge() {
        // Edge case: Interactor declares_application_service references a name
        // that has no matching ApplicationService entry in the catalogue.
        // The renderer must skip gracefully — no broken edge, no panic.
        let usecase = layer("usecase");

        let interactor_entry = entry(
            "OrphanInteractor",
            TypeDefinitionKind::Interactor {
                expected_members: Vec::new(),
                declares_application_service: vec!["MissingService".to_owned()],
                expected_methods: Vec::new(),
            },
        );

        let usecase_doc = doc(vec![interactor_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(usecase.clone(), usecase_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&usecase),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // No edge of any kind should reference the missing service.
        assert!(
            !text.contains("MissingService"),
            "missing ApplicationService must not produce a broken edge; output was:\n{text}"
        );
        // Node itself must be present (the interactor should still render).
        assert!(
            text.contains("L7_usecase_OrphanInteractor"),
            "OrphanInteractor node must still be rendered; output was:\n{text}"
        );
    }

    // --- T009: FreeFunction param / return edges (AC-09) ---

    #[test]
    fn test_render_contract_map_free_function_param_edge_to_declared_type() {
        // AC-09a: FreeFunction with expected_params containing a type
        // that is declared as a ValueObject → edge from FreeFunction node to
        // that ValueObject node must appear.
        let domain = layer("domain");

        let fn_entry = entry(
            "find_user",
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: vec![ParamDeclaration::new("id", "UserId")],
                expected_returns: Vec::new(),
                expected_is_async: false,
            },
        );
        let vo_entry = entry(
            "UserId",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );

        let domain_doc = doc(vec![fn_entry, vo_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // `find_user` → sanitize_id encodes `_` as `__`, so the node id is
        // `find__user`.
        assert!(
            text.contains("L6_domain_find__user -->|\"id\"| L6_domain_UserId"),
            "FreeFunction param edge to UserId must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_free_function_return_edge_to_declared_type() {
        // AC-09b: FreeFunction with expected_returns containing a type that
        // is declared in the catalogue → edge from FreeFunction node to that
        // type node must appear, labelled "returns".
        let domain = layer("domain");

        let fn_entry = entry(
            "make_result",
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: Vec::new(),
                expected_returns: vec!["MyResult".to_owned()],
                expected_is_async: false,
            },
        );
        let result_entry = entry(
            "MyResult",
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
        );

        let domain_doc = doc(vec![fn_entry, result_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // `make_result` → sanitize_id encodes `_` as `__`, so the node id is
        // `make__result`.
        assert!(
            text.contains("L6_domain_make__result -->|\"returns\"| L6_domain_MyResult"),
            "FreeFunction returns edge to MyResult must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_free_function_both_param_and_return_edges() {
        // AC-09 combined: FreeFunction with both params and returns referencing
        // declared types → both edges appear.
        let domain = layer("domain");

        let fn_entry = entry(
            "transform",
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: vec![ParamDeclaration::new("input", "InputDto")],
                expected_returns: vec!["OutputDto".to_owned()],
                expected_is_async: false,
            },
        );
        let in_entry = entry(
            "InputDto",
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
        );
        let out_entry = entry(
            "OutputDto",
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
        );

        let domain_doc = doc(vec![fn_entry, in_entry, out_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            text.contains("L6_domain_transform -->|\"input\"| L6_domain_InputDto"),
            "FreeFunction param edge to InputDto must appear; output was:\n{text}"
        );
        assert!(
            text.contains("L6_domain_transform -->|\"returns\"| L6_domain_OutputDto"),
            "FreeFunction returns edge to OutputDto must appear; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_free_function_undeclared_param_type_emits_no_edge() {
        // AC-09 edge case: FreeFunction param type that is not in the catalogue
        // must NOT produce an edge (external/undeclared types are ignored).
        let domain = layer("domain");

        let fn_entry = entry(
            "read_file",
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: vec![ParamDeclaration::new("path", "std::path::PathBuf")],
                expected_returns: Vec::new(),
                expected_is_async: false,
            },
        );

        let domain_doc = doc(vec![fn_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        assert!(
            !text.contains("-->"),
            "no edge should be emitted for undeclared param type; output was:\n{text}"
        );
    }

    #[test]
    fn test_render_contract_map_free_function_node_is_rendered_in_subgraph() {
        // AC-09: FreeFunction node appears in the subgraph with the
        // free_function classDef shape (verified by node_shape test above
        // but also confirmed here in the full render flow).
        let domain = layer("domain");

        let fn_entry = entry(
            "helper_fn",
            TypeDefinitionKind::FreeFunction {
                module_path: None,
                expected_params: Vec::new(),
                expected_returns: Vec::new(),
                expected_is_async: false,
            },
        );

        let domain_doc = doc(vec![fn_entry]);
        let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
        catalogues.insert(domain.clone(), domain_doc);

        let content = render_contract_map(
            &catalogues,
            std::slice::from_ref(&domain),
            &ContractMapRenderOptions::empty(),
        );
        let text = content.as_ref();
        // `helper_fn` → sanitize_id encodes `_` as `__`, so the node id is
        // `helper__fn`.
        assert!(
            text.contains("L6_domain_helper__fn[helper_fn]:::free_function"),
            "FreeFunction node must appear with free_function classDef; output was:\n{text}"
        );
        // classDef must be emitted.
        assert!(
            text.contains("classDef free_function"),
            "free_function classDef must be declared in diagram header; output was:\n{text}"
        );
    }
}
