//! Unit tests for `contract_map_render`. Extracted to a separate file to keep
//! `contract_map_render.rs` within the `module_limits.max_lines = 700` bound
//! defined in `architecture-rules.json`.
//!
//! This file is compiled only under `#[cfg(test)]` via the `#[path]` attribute
//! in the parent module:
//! ```text
//! #[cfg(test)]
//! #[path = "contract_map_render_tests.rs"]
//! mod tests;
//! ```
//! All items in this file are part of the `tests` module and can reference
//! private helpers in `contract_map_render` via `use super::*`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use super::*;
use crate::tddd::LayerId;
use crate::tddd::catalogue::{
    MethodDeclaration, ParamDeclaration, TraitImplDecl, TypeAction, TypeCatalogueDocument,
    TypeCatalogueEntry, TypeDefinitionKind, TypestateTransitions,
};
use crate::tddd::contract_map_options::ContractMapRenderOptions;

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
    // CN-08: output starts with the generated marker comment.
    assert!(
        text.starts_with("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->"),
        "output must start with generated marker; got:\n{text}"
    );
    assert!(text.contains("```mermaid\n"), "output must contain mermaid fence");
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
            TypeDefinitionKind::Dto { expected_members: Vec::new(), expected_methods: Vec::new() },
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
    assert!(text.contains("L6_sample_FFn[FFn]:::free_function"), "free_function rect + classDef");
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

    let domain_doc =
        doc(vec![entry("Port", TypeDefinitionKind::SecondaryPort { expected_methods: vec![] })]);
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
    // Use `-->|` (mermaid labelled-edge syntax) rather than bare `-->` to
    // avoid false positives from the `-->` in the generated-marker HTML
    // comment that is now prepended to every render output (CN-08).
    assert!(
        !text.contains("-->|"),
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

// --- T003: methods_of() unification (AC-02 / IN-03 / CN-05 / CN-08) ---

/// Builds a `MethodDeclaration` with a single param for test fixtures.
fn method_with_return(name: &str, returns: &str) -> MethodDeclaration {
    MethodDeclaration::new(name, Some("&self".to_owned()), Vec::new(), returns, false)
}

// Helper: assert `methods_of` returns exactly the supplied method names.
fn assert_methods_of(kind: &TypeDefinitionKind, expected_names: &[&str]) {
    let methods = methods_of(kind);
    let got: Vec<&str> = methods.iter().map(|m| m.name()).collect();
    assert_eq!(got, expected_names, "methods_of({}) returned unexpected methods", kind.kind_tag());
}

#[test]
fn test_methods_of_typestate_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::Typestate {
        transitions: TypestateTransitions::Terminal,
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("transition", "NextState")],
    };
    assert_methods_of(&kind, &["transition"]);
}

#[test]
fn test_methods_of_value_object_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::ValueObject {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("value", "String")],
    };
    assert_methods_of(&kind, &["value"]);
}

#[test]
fn test_methods_of_use_case_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::UseCase {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("execute", "Result<(), Error>")],
    };
    assert_methods_of(&kind, &["execute"]);
}

#[test]
fn test_methods_of_interactor_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::Interactor {
        expected_members: Vec::new(),
        declares_application_service: Vec::new(),
        expected_methods: vec![method_with_return("new", "Self")],
    };
    assert_methods_of(&kind, &["new"]);
}

#[test]
fn test_methods_of_dto_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::Dto {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("to_domain", "User")],
    };
    assert_methods_of(&kind, &["to_domain"]);
}

#[test]
fn test_methods_of_command_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::Command {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("validate", "Result<(), Error>")],
    };
    assert_methods_of(&kind, &["validate"]);
}

#[test]
fn test_methods_of_query_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::Query {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("filter", "Filter")],
    };
    assert_methods_of(&kind, &["filter"]);
}

#[test]
fn test_methods_of_factory_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::Factory {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("create", "Result<User, Error>")],
    };
    assert_methods_of(&kind, &["create"]);
}

#[test]
fn test_methods_of_domain_service_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::DomainService {
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("apply", "Result<(), Error>")],
    };
    assert_methods_of(&kind, &["apply"]);
}

#[test]
fn test_methods_of_secondary_port_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::SecondaryPort {
        expected_methods: vec![method_with_return("save", "()")],
    };
    assert_methods_of(&kind, &["save"]);
}

#[test]
fn test_methods_of_application_service_returns_top_level_expected_methods() {
    let kind = TypeDefinitionKind::ApplicationService {
        expected_methods: vec![method_with_return("execute", "Result<(), Error>")],
    };
    assert_methods_of(&kind, &["execute"]);
}

#[test]
fn test_methods_of_secondary_adapter_merges_top_level_and_implements_methods() {
    // Top-level method (direct struct method on the adapter).
    let direct = method_with_return("new", "Self");
    // Trait impl method declared under `implements`.
    let trait_method = method_with_return("save", "()");
    let impl_decl = TraitImplDecl::new("UserRepository", vec![trait_method]);

    let kind = TypeDefinitionKind::SecondaryAdapter {
        implements: vec![impl_decl],
        expected_members: Vec::new(),
        expected_methods: vec![direct],
    };

    let methods = methods_of(&kind);
    let names: Vec<&str> = methods.iter().map(|m| m.name()).collect();
    // Top-level first, then implements (chain order).
    assert_eq!(names, vec!["new", "save"], "SecondaryAdapter must merge both sources in order");
}

#[test]
fn test_methods_of_secondary_adapter_with_empty_top_level_returns_implements_only() {
    let trait_method = method_with_return("find", "Option<User>");
    let impl_decl = TraitImplDecl::new("UserRepository", vec![trait_method]);

    let kind = TypeDefinitionKind::SecondaryAdapter {
        implements: vec![impl_decl],
        expected_members: Vec::new(),
        expected_methods: Vec::new(),
    };
    assert_methods_of(&kind, &["find"]);
}

#[test]
fn test_methods_of_secondary_adapter_with_empty_implements_returns_top_level_only() {
    let kind = TypeDefinitionKind::SecondaryAdapter {
        implements: vec![TraitImplDecl::new("SomeTrait", Vec::new())],
        expected_members: Vec::new(),
        expected_methods: vec![method_with_return("helper", "()")],
    };
    assert_methods_of(&kind, &["helper"]);
}

#[test]
fn test_methods_of_enum_returns_empty_vec() {
    let kind = TypeDefinitionKind::Enum { expected_variants: vec!["A".to_owned()] };
    assert_methods_of(&kind, &[]);
}

#[test]
fn test_methods_of_error_type_returns_empty_vec() {
    let kind = TypeDefinitionKind::ErrorType { expected_variants: vec!["NotFound".to_owned()] };
    assert_methods_of(&kind, &[]);
}

#[test]
fn test_methods_of_free_function_returns_empty_vec() {
    let kind = TypeDefinitionKind::FreeFunction {
        module_path: None,
        expected_params: Vec::new(),
        expected_returns: Vec::new(),
        expected_is_async: false,
    };
    assert_methods_of(&kind, &[]);
}

// --- T003: CN-08 — generated marker in render_contract_map output ---

#[test]
fn test_render_contract_map_output_contains_generated_marker() {
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(
        text.contains("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->"),
        "rendered output must contain the generated marker (CN-08); output was:\n{text}"
    );
}

// --- T003: integration — struct kind with methods produces method edge ---

#[test]
fn test_render_contract_map_struct_kind_with_methods_produces_method_edge() {
    // Regression / integration: a Typestate (struct kind) with top-level
    // `expected_methods` should now produce method-call edges just like
    // SecondaryPort / ApplicationService did before the unification.
    let domain = layer("domain");

    let target = entry(
        "NextState",
        TypeDefinitionKind::Typestate {
            transitions: TypestateTransitions::Terminal,
            expected_members: Vec::new(),
            expected_methods: Vec::new(),
        },
    );
    let source = entry(
        "InitialState",
        TypeDefinitionKind::Typestate {
            transitions: TypestateTransitions::To(vec!["NextState".to_owned()]),
            expected_members: Vec::new(),
            expected_methods: vec![MethodDeclaration::new(
                "advance",
                Some("&self".to_owned()),
                Vec::new(),
                "NextState",
                false,
            )],
        },
    );

    let domain_doc = doc(vec![source, target]);
    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    catalogues.insert(domain.clone(), domain_doc);

    let content = render_contract_map(
        &catalogues,
        std::slice::from_ref(&domain),
        &ContractMapRenderOptions::empty(),
    );
    let text = content.as_ref();
    assert!(
        text.contains("L6_domain_InitialState -->|\"advance\"| L6_domain_NextState"),
        "Typestate method edge must appear after unification; output was:\n{text}"
    );
}

#[test]
fn test_render_contract_map_domain_service_with_methods_produces_method_edge() {
    // New DomainService variant (S1): top-level expected_methods must
    // produce edges after the methods_of() unification.
    let domain = layer("domain");

    let target = entry(
        "Order",
        TypeDefinitionKind::ValueObject {
            expected_members: Vec::new(),
            expected_methods: Vec::new(),
        },
    );
    let svc = entry(
        "PricingService",
        TypeDefinitionKind::DomainService {
            expected_members: Vec::new(),
            expected_methods: vec![MethodDeclaration::new(
                "calculate",
                Some("&self".to_owned()),
                Vec::new(),
                "Order",
                false,
            )],
        },
    );

    let domain_doc = doc(vec![svc, target]);
    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    catalogues.insert(domain.clone(), domain_doc);

    let content = render_contract_map(
        &catalogues,
        std::slice::from_ref(&domain),
        &ContractMapRenderOptions::empty(),
    );
    let text = content.as_ref();
    assert!(
        text.contains("L6_domain_PricingService -->|\"calculate\"| L6_domain_Order"),
        "DomainService method edge must appear; output was:\n{text}"
    );
}

#[test]
fn test_render_contract_map_secondary_adapter_top_level_method_produces_edge() {
    // T003 S2 integration: SecondaryAdapter top-level `expected_methods`
    // (new M1 field) now contributes edges via the 2-source merge.
    let infra = layer("infrastructure");
    let domain = layer("domain");

    let target = entry(
        "User",
        TypeDefinitionKind::ValueObject {
            expected_members: Vec::new(),
            expected_methods: Vec::new(),
        },
    );
    let adapter = entry(
        "PostgresUserRepo",
        TypeDefinitionKind::SecondaryAdapter {
            implements: Vec::new(),
            expected_members: Vec::new(),
            // Direct struct method on the adapter (not a trait impl method).
            expected_methods: vec![MethodDeclaration::new(
                "build",
                None,
                Vec::new(),
                "User",
                false,
            )],
        },
    );

    let domain_doc = doc(vec![target]);
    let infra_doc = doc(vec![adapter]);
    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    catalogues.insert(domain.clone(), domain_doc);
    catalogues.insert(infra.clone(), infra_doc);
    let order = vec![domain, infra];

    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(
        text.contains("L14_infrastructure_PostgresUserRepo -->|\"build\"| L6_domain_User"),
        "SecondaryAdapter top-level method edge must appear; output was:\n{text}"
    );
}
