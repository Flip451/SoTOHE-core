//! TypeRef → `rustdoc_types::Type` conversion using the `syn` crate.
//!
//! Converts a `domain::tddd::catalogue_v2::TypeRef` string (e.g.
//! `"Result<Option<User>, DomainError>"`) into the equivalent
//! `rustdoc_types::Type` representation.
//!
//! ## Responsibilities
//!
//! * Parse the string via `syn::parse_str::<syn::Type>()`.
//! * Walk the `syn::Type` AST recursively and produce `rustdoc_types::Type`.
//! * Resolve each identifier against:
//!   1. Rust primitive names → `Type::Primitive`.
//!   2. The `Self` keyword → `Type::ResolvedPath` with sentinel `Id(0)`.
//!   3. std prelude allowlist → `Type::ResolvedPath`.
//!   4. Known identifiers with a crate prefix (e.g. `"domain_core::UserId"`) → external crate.
//!   5. Identifiers declared in the current catalogue (looked up via a closure).
//!   6. Anything else → an "unresolved marker" using sentinel crate_id `u32::MAX`.
//!
//! ## Unresolved marker
//!
//! Per ADR 2 D10, the A codec is open-world: identifiers that are not known at
//! codec time are recorded as unresolved markers rather than rejected.
//! Closed-world validation occurs in Phase 1 (Signal evaluator).
//!
//! (CN-08 / spec.json IN-09 / ADR 2 D9 / D10 / D11)

use domain::tddd::catalogue_linter::{
    ExtractedTypeRefPath, TypeRefPathExtractionError, TypeRefPathExtractorPort,
};
use domain::tddd::catalogue_v2::identifiers::TypeRef;
use quote::ToTokens;
use syn::visit::{self, Visit};

mod canonical_render;
mod closed_grammar;
mod constants;
mod dummy_context;
mod generic_tokens;
mod helpers;
mod non_trait_paths;
mod parse_ctx;
mod parse_fns;
mod precise_capture;

// ---------------------------------------------------------------------------
// Re-exports — public surface of this module
// ---------------------------------------------------------------------------

pub(crate) use canonical_render::{render_bound, render_type};
pub(crate) use constants::{STD_PRELUDE_TYPES, UNRESOLVED_CRATE_ID};
pub(crate) use generic_tokens::is_plain_generic_param_name;
pub(crate) use helpers::{core_canonical_path, std_canonical_path};
pub(crate) use parse_fns::{
    parse_generic_bound_with_generics, parse_generic_bound_with_generics_preserving_spelling,
    parse_syn_type, parse_syn_type_param_bound, parse_type_ref, parse_type_ref_with_generics,
    parse_type_ref_with_generics_preserving_spelling, validate_legacy_type_ref,
    validate_lexical_alias_target, validate_lexical_generic_bound, validate_lexical_type_ref,
    validate_maybe_const_bound,
};

/// Syn-backed adapter for [`TypeRefPathExtractorPort`].
pub struct SynTypeRefPathExtractorAdapter;

impl TypeRefPathExtractorPort for SynTypeRefPathExtractorAdapter {
    fn extract(
        &self,
        type_ref: &TypeRef,
    ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
        let syntax = parse_syn_type(type_ref.as_str())
            .map_err(|_| TypeRefPathExtractionError::InvalidTypeRef(type_ref.clone()))?;
        let mut visitor = TypeRefPathVisitor { paths: Vec::new(), invalid: false };
        visitor.visit_type(&syntax);
        if visitor.invalid {
            return Err(TypeRefPathExtractionError::InvalidTypeRef(type_ref.clone()));
        }
        Ok(visitor.paths)
    }
}

struct TypeRefPathVisitor {
    paths: Vec<ExtractedTypeRefPath>,
    invalid: bool,
}

impl TypeRefPathVisitor {
    fn record_path(&mut self, path: &syn::Path) {
        let path_text = path_head(path);
        let is_generic = path
            .segments
            .iter()
            .any(|segment| !matches!(&segment.arguments, syn::PathArguments::None));
        match TypeRef::new(path_text) {
            Ok(type_ref) => {
                if is_generic {
                    self.paths.push(ExtractedTypeRefPath::GenericConstructor(type_ref));
                } else {
                    self.paths.push(ExtractedTypeRefPath::Reference(type_ref));
                }
            }
            Err(_) => self.invalid = true,
        }
    }
}

impl<'ast> Visit<'ast> for TypeRefPathVisitor {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        let qself_position = node.qself.as_ref().map(|qself| qself.position);
        if let Some(position) = qself_position {
            visit::visit_type_path(self, node);
            if position > 0 {
                let trait_path = syn::Path {
                    leading_colon: node.path.leading_colon,
                    segments: node.path.segments.iter().take(position).cloned().collect(),
                };
                self.record_path(&trait_path);
            }
        } else {
            self.record_path(&node.path);
            visit::visit_type_path(self, node);
        }
    }

    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        self.record_path(&node.path);
        visit::visit_trait_bound(self, node);
    }
}

fn path_head(path: &syn::Path) -> String {
    let mut rendered = String::new();
    if path.leading_colon.is_some() {
        rendered.push_str("::");
    }
    for (index, segment) in path.segments.iter().enumerate() {
        if index > 0 {
            rendered.push_str("::");
        }
        let ident = segment.ident.to_token_stream().to_string();
        rendered.push_str(ident.strip_prefix("r#").unwrap_or(&ident));
    }
    rendered
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
#[path = "../type_ref_parser_tests.rs"]
mod tests;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod path_extractor_tests {
    use super::*;
    use domain::tddd::catalogue_linter::{
        CatalogueLinterRule, CatalogueLinterRuleKind, ExtractedTypeRefPath, RoleKind,
        RolePayloadField, RuleTarget, evaluate_catalogue_lint,
    };
    use domain::tddd::catalogue_v2::identifiers::{
        CrateName, FullyQualifiedItemPath, Identifier, ModulePath,
    };

    #[test]
    fn test_syn_type_ref_path_extractor_skips_reference_syntax_tokens() {
        let adapter = SynTypeRefPathExtractorAdapter;
        for source in ["&mut OrderPlaced", "*const OrderPlaced", "&'static OrderPlaced"] {
            let type_ref = TypeRef::new(source.to_owned()).expect("non-empty TypeRef");
            let paths = adapter.extract(&type_ref).expect("syn extraction succeeds");
            assert_eq!(
                paths,
                vec![ExtractedTypeRefPath::Reference(
                    TypeRef::new("OrderPlaced".to_owned()).expect("path")
                )]
            );
        }
    }

    #[test]
    fn test_syn_type_ref_path_extractor_classifies_external_constructor_and_inner_path() {
        let adapter = SynTypeRefPathExtractorAdapter;
        let type_ref = TypeRef::new("std::cell::Cell<OrderPlaced>".to_owned()).expect("type ref");

        let paths = adapter.extract(&type_ref).expect("syn extraction succeeds");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::GenericConstructor(
                    TypeRef::new("std::cell::Cell".to_owned()).expect("constructor path")
                ),
                ExtractedTypeRefPath::Reference(
                    TypeRef::new("OrderPlaced".to_owned()).expect("inner path")
                )
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_collects_dyn_and_impl_trait_bounds() {
        let adapter = SynTypeRefPathExtractorAdapter;
        for source in ["dyn domain::ports::Port", "impl domain::ports::Port"] {
            let type_ref = TypeRef::new(source.to_owned()).expect("type ref");
            let paths = adapter.extract(&type_ref).expect("syn extraction succeeds");

            assert_eq!(
                paths,
                vec![ExtractedTypeRefPath::Reference(
                    TypeRef::new("domain::ports::Port".to_owned()).expect("trait path")
                )],
                "trait-bound path was not extracted from `{source}`"
            );
        }
    }

    #[test]
    fn test_syn_type_ref_path_extractor_splits_qself_into_self_and_trait_paths() {
        let adapter = SynTypeRefPathExtractorAdapter;
        let type_ref =
            TypeRef::new("<domain::alpha::Wrapper as domain::ports::Port>::Output".to_owned())
                .expect("qualified path is a valid TypeRef");

        let paths = adapter.extract(&type_ref).expect("syn extraction succeeds");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::Reference(
                    TypeRef::new("domain::alpha::Wrapper".to_owned()).expect("self path"),
                ),
                ExtractedTypeRefPath::Reference(
                    TypeRef::new("domain::ports::Port".to_owned()).expect("trait path"),
                ),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_resolves_inner_catalogue_identity_across_valid_wrappers() {
        let adapter = SynTypeRefPathExtractorAdapter;
        let catalogue_crate = CrateName::new("domain").expect("valid crate");
        let expected = FullyQualifiedItemPath::new(
            catalogue_crate.clone(),
            ModulePath::from_segments(vec!["orders".to_owned()]).expect("valid module path"),
            Identifier::new("OrderPlaced").expect("valid identifier"),
        );
        let universe = std::collections::BTreeSet::from([expected.clone()]);

        for source in [
            "&mut OrderPlaced",
            "*const OrderPlaced",
            "&'static OrderPlaced",
            "std::cell::Cell<OrderPlaced>",
        ] {
            let type_ref = TypeRef::new(source.to_owned()).expect("non-empty TypeRef");
            let extracted = adapter.extract(&type_ref).expect("valid Rust type extracts");
            let inner = extracted
                .iter()
                .find_map(|path| match path {
                    ExtractedTypeRefPath::Reference(path) => Some(path.clone()),
                    ExtractedTypeRefPath::GenericConstructor(_) => None,
                })
                .expect("every wrapper contains the catalogue reference");
            let resolved =
                domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity(
                    &inner,
                    &catalogue_crate,
                    &universe,
                )
                .expect("inner catalogue identity resolves");

            assert_eq!(resolved, expected, "source `{source}` resolved incorrectly");
            assert!(extracted.iter().all(|path| match path {
                ExtractedTypeRefPath::Reference(path) => path.as_str() == "OrderPlaced",
                ExtractedTypeRefPath::GenericConstructor(path) => {
                    path.as_str() == "std::cell::Cell"
                }
            }));
        }
    }

    #[test]
    fn test_catalogue_lint_accepts_syntactically_valid_unknown_external_wrapper() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::document::CatalogueSchemaVersion;
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction, NonEmptyVec};
        use domain::tddd::layer_id::LayerId;
        use domain::tddd::primitive_occurrence_scanner::{
            PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceReport,
            PrimitiveOccurrenceScanError, PrimitiveOccurrenceScanner,
        };
        use domain::tddd::semantic_verify::CatalogueEntryKey;

        struct NoopScanner;

        impl PrimitiveOccurrenceScanner for NoopScanner {
            fn scan(
                &self,
                _type_ref: TypeRef,
                _primitives: NonEmptyVec<PrimitiveName>,
                _position: PrimitiveOccurrencePosition,
            ) -> Result<PrimitiveOccurrenceReport, PrimitiveOccurrenceScanError> {
                Ok(PrimitiveOccurrenceReport::new(std::collections::BTreeMap::new()))
            }
        }

        let layer = LayerId::try_new("domain".to_owned()).expect("valid layer");
        let module = ModulePath::from_segments(vec!["alpha".to_owned()]).expect("valid module");
        let mut catalogue = domain::tddd::catalogue_v2::CatalogueDocument::new(
            CatalogueSchemaVersion::new(3),
            CrateName::new("domain").expect("valid crate"),
            layer.clone(),
        );
        let unit_kind = TypeKindV2::Struct(StructKind::new(StructShape::Unit, None));
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::Event".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::DomainEvent,
                unit_kind.clone(),
                vec![],
                vec![],
                vec![],
                module.clone(),
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::UseCase".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::UseCase {
                    handles: vec![
                        TypeRef::new(
                            "std::not_a_real_wrapper<&'static domain::alpha::Event>".to_owned(),
                        )
                        .unwrap(),
                    ],
                },
                unit_kind,
                vec![],
                vec![],
                vec![],
                module,
                None,
                vec![],
                vec![],
            ),
        );

        let mut all_catalogues = std::collections::BTreeMap::new();
        all_catalogues.insert(layer.clone(), catalogue);
        let rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: RolePayloadField::Handles,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .expect("valid identity rule");
        let violations = evaluate_catalogue_lint(
            &[rule],
            &all_catalogues,
            &layer,
            &NoopScanner,
            &SynTypeRefPathExtractorAdapter,
        )
        .expect("unknown external wrapper spelling is outside catalogue-lint scope");

        assert!(
            violations.is_empty(),
            "the inner domain event should resolve despite the unknown wrapper: {violations:?}"
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_rejects_invalid_type_ref() {
        let adapter = SynTypeRefPathExtractorAdapter;
        let type_ref = TypeRef::new("(".to_owned()).expect("non-empty TypeRef");

        assert!(matches!(
            adapter.extract(&type_ref),
            Err(TypeRefPathExtractionError::InvalidTypeRef(invalid))
                if invalid == type_ref
        ));
    }
}
