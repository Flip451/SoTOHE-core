//! TypeRef syntax parsing and extraction using the `syn` crate.
//!
//! The module also contains the existing TypeRef-to-rustdoc conversion helpers,
//! while [`SynTypeRefPathExtractorAdapter`] is deliberately syntax-only.
//!
//! ## Responsibilities
//!
//! * Parse the string via `syn::Type`, falling back to `syn::TypeParamBound`
//!   for standalone bound spellings such as `?Sized`.
//! * Walk the `syn::Type` AST recursively and produce complete syntactic
//!   occurrences for the domain linter.
//! * Keep catalogue-vs-external membership classification in the domain layer.
//!
//! ## Unresolved marker
//!
//! The existing rustdoc conversion path remains open-world; the linter
//! extraction path fails closed when syntax inspection cannot complete.
//!
//! (CN-08 / spec.json IN-09 / ADR 2 D9 / D10 / D11)

use domain::tddd::catalogue_linter::{
    ExtractedTypeRefPath, TypeRefPathExtractionError, TypeRefPathExtractorPort,
};
use domain::tddd::catalogue_v2::identifiers::{CatalogueItemNamespace, ParamName, TypeRef};
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
        type_parameters: &[ParamName],
        lifetime_parameters: &[ParamName],
        const_parameters: &[ParamName],
    ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
        enum ParsedTypeRef {
            Type(syn::Type),
            Bound(syn::TypeParamBound),
        }

        let syntax = parse_syn_type(type_ref.as_str())
            .map(ParsedTypeRef::Type)
            .or_else(|_| parse_syn_type_param_bound(type_ref.as_str()).map(ParsedTypeRef::Bound))
            .map_err(|_| TypeRefPathExtractionError::UnsupportedSyntax {
                location: type_ref.clone(),
            })?;
        let mut visitor = TypeRefPathVisitor {
            paths: Vec::new(),
            type_parameters,
            lifetime_parameters,
            const_parameters,
            location: type_ref.clone(),
            depth: 0,
            resources: 0,
            error: None,
        };
        match &syntax {
            ParsedTypeRef::Type(syntax) => visitor.visit_type(syntax),
            ParsedTypeRef::Bound(bound) => visitor.visit_type_param_bound(bound),
        }
        visitor.error.map_or(Ok(visitor.paths), Err)
    }
}

const MAX_INSPECTION_DEPTH: usize = 32;
const MAX_INSPECTION_RESOURCES: usize = 512;

struct TypeRefPathVisitor<'a> {
    paths: Vec<ExtractedTypeRefPath>,
    type_parameters: &'a [ParamName],
    lifetime_parameters: &'a [ParamName],
    const_parameters: &'a [ParamName],
    location: TypeRef,
    depth: usize,
    resources: usize,
    error: Option<TypeRefPathExtractionError>,
}

impl TypeRefPathVisitor<'_> {
    fn enter_node(&mut self) -> bool {
        if self.error.is_some() {
            return false;
        }
        self.resources = self.resources.saturating_add(1);
        if self.resources > MAX_INSPECTION_RESOURCES {
            self.error = Some(TypeRefPathExtractionError::ResourceLimitExceeded {
                location: self.location.clone(),
            });
            return false;
        }
        self.depth = self.depth.saturating_add(1);
        if self.depth > MAX_INSPECTION_DEPTH {
            self.error = Some(TypeRefPathExtractionError::DepthLimitExceeded {
                location: self.location.clone(),
            });
            self.depth = self.depth.saturating_sub(1);
            return false;
        }
        true
    }

    fn leave_node(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn unsupported(&mut self) {
        if self.error.is_none() {
            self.error = Some(TypeRefPathExtractionError::UnsupportedSyntax {
                location: self.location.clone(),
            });
        }
    }

    fn push(&mut self, occurrence: ExtractedTypeRefPath) {
        if self.error.is_some() {
            return;
        }
        self.resources = self.resources.saturating_add(1);
        if self.resources > MAX_INSPECTION_RESOURCES {
            self.error = Some(TypeRefPathExtractionError::ResourceLimitExceeded {
                location: self.location.clone(),
            });
            return;
        }
        self.paths.push(occurrence);
    }

    fn push_param(&mut self, occurrence: ExtractedTypeRefPath) {
        self.push(occurrence);
    }

    fn record_path(&mut self, path: &syn::Path, namespace: CatalogueItemNamespace) {
        let first = path.segments.first().map(|segment| ident_text(&segment.ident));
        let rooted_in_type_parameter = first.as_deref().is_some_and(|name| {
            name == "Self" || self.type_parameters.iter().any(|param| param.as_str() == name)
        });
        if rooted_in_type_parameter {
            if let Some(name) = first.and_then(|name| ParamName::new(name).ok()) {
                self.push_param(ExtractedTypeRefPath::TypeParameter(name));
            } else {
                self.unsupported();
                return;
            }
            for segment in path.segments.iter().skip(1) {
                if let Ok(name) = ParamName::new(ident_text(&segment.ident)) {
                    self.push_param(ExtractedTypeRefPath::AssociatedItemLabel(name));
                } else {
                    self.unsupported();
                    return;
                }
            }
            return;
        }

        let path_text = path_head(path);
        match TypeRef::new(path_text) {
            Ok(type_ref) => self.push(ExtractedTypeRefPath::Path { type_ref, namespace }),
            Err(_) => self.unsupported(),
        }
    }

    fn record_declared_const_parameter(&mut self, path: &syn::Path) -> bool {
        if path.segments.len() != 1 {
            return false;
        }
        let Some(segment) = path.segments.first() else {
            self.unsupported();
            return true;
        };
        if !matches!(&segment.arguments, syn::PathArguments::None) {
            return false;
        }
        let name = ident_text(&segment.ident);
        if !self.const_parameters.iter().any(|param| param.as_str() == name) {
            return false;
        }
        match ParamName::new(name) {
            Ok(name) => self.push_param(ExtractedTypeRefPath::ConstParameter(name)),
            Err(_) => self.unsupported(),
        }
        true
    }

    fn record_unbraced_const_argument(&mut self, ty: &syn::Type) -> bool {
        let syn::Type::Path(type_path) = ty else {
            return false;
        };
        if type_path.qself.is_some() {
            return false;
        }
        self.record_declared_const_parameter(&type_path.path)
    }

    fn record_associated_label(&mut self, ident: &syn::Ident) {
        match ParamName::new(ident_text(ident)) {
            Ok(name) => self.push_param(ExtractedTypeRefPath::AssociatedItemLabel(name)),
            Err(_) => self.unsupported(),
        }
    }

    fn record_expr_path(&mut self, path: &syn::Path) {
        // Expression paths name values, not catalogue types.  Keep declared
        // const parameters as their exclusive occurrence, but do not emit
        // imported constants/functions as catalogue identities.  The default
        // visitor still traverses qself and path generic arguments, so nested
        // type occurrences remain visible to the extractor.
        let _ = self.record_declared_const_parameter(path);
    }
}

impl<'ast, 'ctx> Visit<'ast> for TypeRefPathVisitor<'ctx> {
    fn visit_type(&mut self, node: &'ast syn::Type) {
        if !self.enter_node() {
            return;
        }
        match node {
            syn::Type::Macro(_) | syn::Type::Infer(_) | syn::Type::Verbatim(_) => {
                self.unsupported();
            }
            _ => visit::visit_type(self, node),
        }
        self.leave_node();
    }

    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        if !self.enter_node() {
            return;
        }
        match node {
            syn::Expr::Macro(_) | syn::Expr::Infer(_) | syn::Expr::Verbatim(_) => {
                self.unsupported();
            }
            _ => visit::visit_expr(self, node),
        }
        self.leave_node();
    }

    fn visit_type_param_bound(&mut self, node: &'ast syn::TypeParamBound) {
        if !self.enter_node() {
            return;
        }
        match node {
            syn::TypeParamBound::Verbatim(_) => self.unsupported(),
            _ => visit::visit_type_param_bound(self, node),
        }
        self.leave_node();
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if self.error.is_some() {
            return;
        }
        let qself_position = node.qself.as_ref().map(|qself| qself.position);
        if let Some(position) = qself_position {
            visit::visit_type_path(self, node);
            if position > 0 {
                self.record_path_prefix(&node.path, position);
            }
            for segment in node.path.segments.iter().skip(position) {
                self.record_associated_label(&segment.ident);
            }
        } else {
            self.record_path(&node.path, CatalogueItemNamespace::Type);
            visit::visit_type_path(self, node);
        }
    }

    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        self.record_path(&node.path, CatalogueItemNamespace::Trait);
        visit::visit_trait_bound(self, node);
    }

    fn visit_precise_capture(&mut self, node: &'ast syn::PreciseCapture) {
        if !self.enter_node() {
            return;
        }
        for parameter in &node.params {
            match parameter {
                syn::CapturedParam::Lifetime(lifetime) => {
                    let name = ident_text(&lifetime.ident);
                    if name == "_" || name == "static" {
                        continue;
                    }
                    if self.lifetime_parameters.iter().any(|parameter| parameter.as_str() == name) {
                        match ParamName::new(name) {
                            Ok(name) => {
                                self.push_param(ExtractedTypeRefPath::LifetimeParameter(name));
                            }
                            Err(_) => self.unsupported(),
                        }
                    } else {
                        self.unsupported();
                    }
                }
                syn::CapturedParam::Ident(ident) => {
                    let name = ident_text(ident);
                    if name == "Self" {
                        match ParamName::new(name) {
                            Ok(name) => {
                                self.push_param(ExtractedTypeRefPath::TypeParameter(name));
                            }
                            Err(_) => self.unsupported(),
                        }
                        continue;
                    }
                    let is_type_parameter =
                        self.type_parameters.iter().any(|parameter| parameter.as_str() == name);
                    let is_const_parameter =
                        self.const_parameters.iter().any(|parameter| parameter.as_str() == name);
                    match (is_type_parameter, is_const_parameter) {
                        (true, false) => match ParamName::new(name) {
                            Ok(name) => {
                                self.push_param(ExtractedTypeRefPath::TypeParameter(name));
                            }
                            Err(_) => self.unsupported(),
                        },
                        (false, true) => match ParamName::new(name) {
                            Ok(name) => {
                                self.push_param(ExtractedTypeRefPath::ConstParameter(name));
                            }
                            Err(_) => self.unsupported(),
                        },
                        (false, false) | (true, true) => self.unsupported(),
                    }
                }
                _ => self.unsupported(),
            }
            if self.error.is_some() {
                break;
            }
        }
        self.leave_node();
    }

    fn visit_lifetime(&mut self, node: &'ast syn::Lifetime) {
        let name = ident_text(&node.ident);
        let declared = self.lifetime_parameters.iter().any(|param| param.as_str() == name);
        if declared || (name != "static" && name != "_") {
            if let Ok(name) = ParamName::new(name) {
                self.push_param(ExtractedTypeRefPath::LifetimeParameter(name));
            } else {
                self.unsupported();
            }
        } else {
            // Built-in/elided lifetimes are syntax, not declared catalogue
            // parameters and therefore have no domain identity to classify.
        }
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if self.error.is_some() {
            return;
        }
        if node.qself.is_none() {
            self.record_expr_path(&node.path);
        }
        visit::visit_expr_path(self, node);
        if self.error.is_some() {
            return;
        }
        if let Some(qself) = node.qself.as_ref() {
            if qself.position > 0 {
                self.record_path_prefix(&node.path, qself.position);
            }
            for segment in node.path.segments.iter().skip(qself.position) {
                self.record_associated_label(&segment.ident);
            }
        }
    }

    fn visit_generic_argument(&mut self, node: &'ast syn::GenericArgument) {
        if let syn::GenericArgument::Type(ty) = node {
            // `syn` represents an unbraced `Buffer<N>` argument as a type
            // path.  The active const-parameter context is authoritative for
            // this otherwise ambiguous single-segment spelling.
            if self.record_unbraced_const_argument(ty) {
                return;
            }
        }
        match node {
            syn::GenericArgument::AssocType(assoc) => self.record_associated_label(&assoc.ident),
            syn::GenericArgument::AssocConst(assoc) => {
                self.record_associated_label(&assoc.ident);
            }
            syn::GenericArgument::Constraint(constraint) => {
                self.record_associated_label(&constraint.ident);
            }
            _ => {}
        }
        if self.error.is_none() {
            visit::visit_generic_argument(self, node);
        }
    }

    fn visit_path_arguments(&mut self, node: &'ast syn::PathArguments) {
        if !self.enter_node() {
            return;
        }
        visit::visit_path_arguments(self, node);
        self.leave_node();
    }
}

impl TypeRefPathVisitor<'_> {
    fn record_path_prefix(&mut self, path: &syn::Path, end: usize) {
        let mut rendered = String::new();
        if path.leading_colon.is_some() {
            rendered.push_str("::");
        }
        for (index, segment) in path.segments.iter().take(end).enumerate() {
            if index > 0 {
                rendered.push_str("::");
            }
            rendered.push_str(&ident_text(&segment.ident));
        }
        match TypeRef::new(rendered) {
            Ok(type_ref) => self.push(ExtractedTypeRefPath::trait_path(type_ref)),
            Err(_) => self.unsupported(),
        }
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
        rendered.push_str(&ident_text(&segment.ident));
    }
    rendered
}

fn ident_text(ident: &syn::Ident) -> String {
    let ident = ident.to_token_stream().to_string();
    ident.strip_prefix("r#").unwrap_or(&ident).to_owned()
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
        RolePayloadField, RuleTarget, TypeRefPathExtractionError, evaluate_catalogue_lint,
    };
    use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, ParamName};

    fn type_ref(source: &str) -> TypeRef {
        TypeRef::new(source.to_owned()).expect("non-empty TypeRef")
    }

    fn extract(
        source: &str,
        types: &[&str],
        lifetimes: &[&str],
        consts: &[&str],
    ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
        let types = types
            .iter()
            .map(|name| ParamName::new(*name).expect("type parameter"))
            .collect::<Vec<_>>();
        let lifetimes = lifetimes
            .iter()
            .map(|name| ParamName::new(*name).expect("lifetime parameter"))
            .collect::<Vec<_>>();
        let consts = consts
            .iter()
            .map(|name| ParamName::new(*name).expect("const parameter"))
            .collect::<Vec<_>>();
        SynTypeRefPathExtractorAdapter.extract(&type_ref(source), &types, &lifetimes, &consts)
    }

    #[test]
    fn test_syn_type_ref_path_extractor_skips_reference_syntax_tokens() {
        for source in ["&mut OrderPlaced", "*const OrderPlaced", "&'static OrderPlaced"] {
            let paths = extract(source, &[], &[], &[]).expect("syn extraction succeeds");
            assert_eq!(paths, vec![ExtractedTypeRefPath::type_path(type_ref("OrderPlaced"))]);
        }
    }

    #[test]
    fn test_syn_type_ref_path_extractor_accepts_standalone_maybe_sized_bound() {
        let paths = extract("?Sized", &[], &[], &[]).expect("bound extraction succeeds");

        assert_eq!(paths, vec![ExtractedTypeRefPath::trait_path(type_ref("Sized"))]);
    }

    #[test]
    fn test_syn_type_ref_path_extractor_classifies_external_constructor_and_inner_path() {
        let paths = extract("std::cell::Cell<OrderPlaced>", &[], &[], &[])
            .expect("syn extraction succeeds");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("std::cell::Cell")),
                ExtractedTypeRefPath::type_path(type_ref("OrderPlaced")),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_preserves_same_named_qualified_paths() {
        let paths = extract(
            "std::result::Result<domain::alpha::Event, domain::beta::Event>",
            &[],
            &[],
            &[],
        )
        .expect("syn extraction succeeds");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("std::result::Result")),
                ExtractedTypeRefPath::type_path(type_ref("domain::alpha::Event")),
                ExtractedTypeRefPath::type_path(type_ref("domain::beta::Event")),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_collects_dyn_and_impl_trait_bounds() {
        for source in ["dyn domain::ports::Port", "impl domain::ports::Port"] {
            let paths = extract(source, &[], &[], &[]).expect("syn extraction succeeds");

            assert_eq!(
                paths,
                vec![ExtractedTypeRefPath::trait_path(type_ref("domain::ports::Port"))],
                "trait-bound path was not extracted from `{source}`"
            );
        }
    }

    #[test]
    fn test_syn_type_ref_path_extractor_splits_qself_into_self_and_trait_paths() {
        let paths =
            extract("<domain::alpha::Wrapper as domain::ports::Port>::Output", &[], &[], &[])
                .expect("syn extraction succeeds");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("domain::alpha::Wrapper")),
                ExtractedTypeRefPath::trait_path(type_ref("domain::ports::Port")),
                ExtractedTypeRefPath::AssociatedItemLabel(ParamName::new("Output").unwrap()),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_carries_parameter_context_exclusively() {
        let paths = extract("std::vec::Vec<&'a T, [u8; N]>", &["T"], &["a"], &["N"])
            .expect("parameterized type extracts");

        assert!(paths.contains(&ExtractedTypeRefPath::type_path(type_ref("std::vec::Vec"))));
        assert!(paths.contains(&ExtractedTypeRefPath::TypeParameter(ParamName::new("T").unwrap())));
        assert!(
            paths.contains(&ExtractedTypeRefPath::LifetimeParameter(ParamName::new("a").unwrap()))
        );
        assert!(
            paths.contains(&ExtractedTypeRefPath::ConstParameter(ParamName::new("N").unwrap()))
        );
        assert!(paths.contains(&ExtractedTypeRefPath::type_path(type_ref("u8"))));
        assert!(!paths.iter().any(|path| {
            matches!(path, ExtractedTypeRefPath::Path { type_ref: path, .. } if matches!(path.as_str(), "T" | "a" | "N"))
        }));
    }

    #[test]
    fn test_syn_type_ref_path_extractor_classifies_unbraced_const_generic_argument() {
        let paths = extract("Buffer<N>", &[], &[], &["N"])
            .expect("unbraced const generic argument extracts");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("Buffer")),
                ExtractedTypeRefPath::ConstParameter(ParamName::new("N").unwrap()),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_skips_non_parameter_const_value_paths() {
        let array_paths =
            extract("[u8; CAPACITY]", &[], &[], &[]).expect("array const expression extracts");
        assert_eq!(array_paths, vec![ExtractedTypeRefPath::type_path(type_ref("u8"))]);

        let call_paths = extract("Wrapper<{ size_of::<T>() }>", &["T"], &[], &[])
            .expect("const block with nested type argument extracts");
        assert_eq!(
            call_paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("Wrapper")),
                ExtractedTypeRefPath::TypeParameter(ParamName::new("T").unwrap()),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_collects_qself_trait_in_const_expression() {
        let paths = extract("[u8; <domain::Ty as external::Trait>::N]", &[], &[], &["N"])
            .expect("qualified const expression extracts");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("u8")),
                ExtractedTypeRefPath::type_path(type_ref("domain::Ty")),
                ExtractedTypeRefPath::trait_path(type_ref("external::Trait")),
                ExtractedTypeRefPath::AssociatedItemLabel(ParamName::new("N").unwrap()),
            ]
        );

        let paths = extract("[u8; <domain::Ty>::N]", &[], &[], &["N"])
            .expect("qself-associated const expression extracts");
        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::type_path(type_ref("u8")),
                ExtractedTypeRefPath::type_path(type_ref("domain::Ty")),
                ExtractedTypeRefPath::AssociatedItemLabel(ParamName::new("N").unwrap()),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_classifies_precise_capture_parameters() {
        let paths = extract("impl Trait + use<'a, T, K>", &["T"], &["a"], &["K"])
            .expect("precise-capture parameters extract");

        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::trait_path(type_ref("Trait")),
                ExtractedTypeRefPath::LifetimeParameter(ParamName::new("a").unwrap()),
                ExtractedTypeRefPath::TypeParameter(ParamName::new("T").unwrap()),
                ExtractedTypeRefPath::ConstParameter(ParamName::new("K").unwrap()),
            ]
        );

        assert!(matches!(
            extract("impl Trait + use<'a, Unknown>", &["T"], &["a"], &["K"]),
            Err(TypeRefPathExtractionError::UnsupportedSyntax { .. })
        ));

        let paths = extract("use<'_, Self, T>", &["T"], &[], &[])
            .expect("built-in precise-capture parameters extract");
        assert_eq!(
            paths,
            vec![
                ExtractedTypeRefPath::TypeParameter(ParamName::new("Self").unwrap()),
                ExtractedTypeRefPath::TypeParameter(ParamName::new("T").unwrap()),
            ]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_rejects_verbatim_bounds() {
        for source in ["const external::Trait", "[const] external::Trait"] {
            assert!(matches!(
                extract(source, &[], &[], &[]),
                Err(TypeRefPathExtractionError::UnsupportedSyntax { .. })
            ));
        }
    }

    #[test]
    fn test_catalogue_lint_accepts_syntactically_valid_unknown_external_wrapper() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
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
            3,
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
                Some(module.clone()),
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
                Some(module),
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
    fn test_catalogue_lint_resolves_qualified_same_named_paths_and_passes_external_paths() {
        use domain::tddd::catalogue_linter::evaluate_catalogue_lint;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
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
        let module = |name: &str| {
            ModulePath::from_segments(vec![name.to_owned()]).expect("valid module path")
        };
        let unit_entry = |role: DataRole, module_path: &str| {
            TypeEntry::new(
                ItemAction::Add,
                role,
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(module(module_path)),
                None,
                vec![],
                vec![],
            )
        };
        let mut catalogue = domain::tddd::catalogue_v2::CatalogueDocument::new(
            3,
            CrateName::new("domain").expect("valid crate"),
            layer.clone(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::Event".to_owned()).unwrap(),
            unit_entry(DataRole::DomainEvent, "alpha"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::beta::Event".to_owned()).unwrap(),
            unit_entry(DataRole::value_object(), "beta"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesAlphaEvent".to_owned()).unwrap(),
            unit_entry(
                DataRole::UseCase {
                    handles: vec![
                        TypeRef::new(
                            "std::not_a_real_wrapper<&'static domain::alpha::Event>".to_owned(),
                        )
                        .unwrap(),
                    ],
                },
                "alpha",
            ),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesBetaEvent".to_owned()).unwrap(),
            unit_entry(
                DataRole::UseCase {
                    handles: vec![TypeRef::new("domain::beta::Event".to_owned()).unwrap()],
                },
                "alpha",
            ),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesExternalEvent".to_owned()).unwrap(),
            unit_entry(
                DataRole::UseCase {
                    handles: vec![TypeRef::new("external_crate::Event".to_owned()).unwrap()],
                },
                "alpha",
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
        .expect("all qualified and external references are completely inspectable");

        assert_eq!(violations.len(), 1);
        let Some(violation) = violations.first() else {
            panic!("the beta reference must produce one role violation");
        };
        assert_eq!(violation.entry_name(), "domain::alpha::HandlesBetaEvent");
        assert!(violation.message().contains("ValueObject"));
        assert!(violation.message().contains("DomainEvent"));
        assert!(!violations.iter().any(|violation| {
            matches!(
                violation.entry_name(),
                "domain::alpha::HandlesAlphaEvent" | "domain::alpha::HandlesExternalEvent"
            )
        }));
    }

    #[test]
    fn test_catalogue_lint_via_syn_adapter_fails_closed_for_ambiguous_short_name() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
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

        // The catalogue deliberately declares the same short name on two distinct
        // fully qualified paths; the unqualified `Event` reference must not fall back
        // to either declaration.
        let layer = LayerId::try_new("domain".to_owned()).expect("valid layer");
        let module = |name: &str| {
            ModulePath::from_segments(vec![name.to_owned()]).expect("valid module path")
        };
        let unit_entry = |role: DataRole, module_path: &str| {
            TypeEntry::new(
                ItemAction::Add,
                role,
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(module(module_path)),
                None,
                vec![],
                vec![],
            )
        };
        let mut catalogue = domain::tddd::catalogue_v2::CatalogueDocument::new(
            3,
            CrateName::new("domain").expect("valid crate"),
            layer.clone(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::Event".to_owned()).unwrap(),
            unit_entry(DataRole::DomainEvent, "alpha"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::beta::Event".to_owned()).unwrap(),
            unit_entry(DataRole::value_object(), "beta"),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesAmbiguousEvent".to_owned()).unwrap(),
            unit_entry(
                DataRole::UseCase { handles: vec![TypeRef::new("Event".to_owned()).unwrap()] },
                "alpha",
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
        let error = evaluate_catalogue_lint(
            &[rule],
            &all_catalogues,
            &layer,
            &NoopScanner,
            &SynTypeRefPathExtractorAdapter,
        )
        .expect_err("an ambiguous short-name reference must fail closed");
        let domain::tddd::catalogue_linter::CatalogueLinterError::IdentityResolutionFailed(
            domain::tddd::catalogue_v2::identity_resolution::CatalogueIdentityResolutionError::AmbiguousIdentifier(
                identifier,
                candidates,
            ),
        ) = error
        else {
            panic!("ambiguous catalogue identity must be reported as a fail-closed error");
        };
        assert_eq!(identifier.as_str(), "Event");
        let candidate_paths =
            candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(
            candidate_paths,
            vec!["domain::alpha::Event".to_owned(), "domain::beta::Event".to_owned(),]
        );
    }

    #[test]
    fn test_catalogue_lint_via_extractor_resolves_nested_paths_and_enumerates_ambiguous_candidates()
    {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
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
        let module = |name: &str| {
            ModulePath::from_segments(vec![name.to_owned()]).expect("valid module path")
        };
        let unit_entry = |role: DataRole, module_path: &str| {
            TypeEntry::new(
                ItemAction::Add,
                role,
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                Some(module(module_path)),
                None,
                vec![],
                vec![],
            )
        };
        let mut nested_catalogue = domain::tddd::catalogue_v2::CatalogueDocument::new(
            3,
            CrateName::new("domain").expect("valid crate"),
            layer.clone(),
        );
        nested_catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::Event".to_owned()).unwrap(),
            unit_entry(DataRole::DomainEvent, "alpha"),
        );
        nested_catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::beta::Event".to_owned()).unwrap(),
            unit_entry(DataRole::value_object(), "beta"),
        );
        nested_catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesNestedCorrectEvent".to_owned())
                .unwrap(),
            unit_entry(
                DataRole::UseCase {
                    handles: vec![
                        TypeRef::new("std::not_a_real_wrapper<domain::alpha::Event>").unwrap(),
                    ],
                },
                "alpha",
            ),
        );
        nested_catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesNestedWrongEvent".to_owned())
                .unwrap(),
            unit_entry(
                DataRole::UseCase {
                    handles: vec![
                        TypeRef::new("std::not_a_real_wrapper<domain::beta::Event>").unwrap(),
                    ],
                },
                "alpha",
            ),
        );

        let mut nested_catalogues = std::collections::BTreeMap::new();
        nested_catalogues.insert(layer.clone(), nested_catalogue.clone());
        let nested_rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: RolePayloadField::Handles,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .expect("valid identity rule");
        let nested_violations = evaluate_catalogue_lint(
            &[nested_rule],
            &nested_catalogues,
            &layer,
            &NoopScanner,
            &SynTypeRefPathExtractorAdapter,
        )
        .expect("nested TypeRef paths are completely inspectable");

        assert_eq!(nested_violations.len(), 1);
        let Some(nested_violation) = nested_violations.first() else {
            panic!("the nested wrong-role reference must produce a violation");
        };
        assert_eq!(nested_violation.entry_name(), "domain::alpha::HandlesNestedWrongEvent");
        assert!(nested_violation.message().contains("ValueObject"));
        assert!(nested_violation.message().contains("DomainEvent"));
        assert!(
            !nested_violations
                .iter()
                .any(|violation| violation.entry_name()
                    == "domain::alpha::HandlesNestedCorrectEvent")
        );

        let mut ambiguous_catalogue = nested_catalogue;
        ambiguous_catalogue.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::HandlesAmbiguousEvent".to_owned()).unwrap(),
            unit_entry(
                DataRole::UseCase { handles: vec![TypeRef::new("Event").unwrap()] },
                "alpha",
            ),
        );
        let mut ambiguous_catalogues = std::collections::BTreeMap::new();
        ambiguous_catalogues.insert(layer.clone(), ambiguous_catalogue);
        let ambiguous_rule = CatalogueLinterRule::new(
            RuleTarget::new(vec![RoleKind::UseCase]),
            CatalogueLinterRuleKind::ReferencedRoleConstraint {
                target_field: RolePayloadField::Handles,
                expected_role: RoleKind::DomainEvent,
            },
        )
        .expect("valid identity rule");
        let error = evaluate_catalogue_lint(
            &[ambiguous_rule],
            &ambiguous_catalogues,
            &layer,
            &NoopScanner,
            &SynTypeRefPathExtractorAdapter,
        )
        .expect_err("an accepted unqualified spelling must fail closed when ambiguous");
        let domain::tddd::catalogue_linter::CatalogueLinterError::IdentityResolutionFailed(
            domain::tddd::catalogue_v2::identity_resolution::CatalogueIdentityResolutionError::AmbiguousIdentifier(
                identifier,
                candidates,
            ),
        ) = error
        else {
            panic!("ambiguous catalogue identity must be reported as a fail-closed error");
        };
        assert_eq!(identifier.as_str(), "Event");
        let candidate_paths =
            candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(
            candidate_paths,
            vec!["domain::alpha::Event".to_owned(), "domain::beta::Event".to_owned(),]
        );
    }

    #[test]
    fn test_syn_type_ref_path_extractor_rejects_invalid_type_ref() {
        assert!(matches!(
            extract("(", &[], &[], &[]),
            Err(TypeRefPathExtractionError::UnsupportedSyntax { location })
                if location == type_ref("(")
        ));
    }

    #[test]
    fn test_syn_type_ref_path_extractor_rejects_unsupported_macro_syntax() {
        assert!(matches!(
            extract("foo!()", &[], &[], &[]),
            Err(TypeRefPathExtractionError::UnsupportedSyntax { location })
                if location == type_ref("foo!()")
        ));
    }

    #[test]
    fn test_syn_type_ref_path_extractor_rejects_partially_traversable_unsupported_syntax() {
        let source = "std::option::Option<Good, bad!()>";
        assert!(matches!(
            extract(source, &[], &[], &[]),
            Err(TypeRefPathExtractionError::UnsupportedSyntax { location })
                if location == type_ref(source)
        ));
    }

    #[test]
    fn test_syn_type_ref_path_extractor_enforces_depth_and_resource_limits() {
        let mut nested = "Leaf".to_owned();
        for _ in 0..40 {
            nested = format!("Option<{nested}>");
        }
        assert!(matches!(
            extract(&nested, &[], &[], &[]),
            Err(TypeRefPathExtractionError::DepthLimitExceeded { location }) if location.as_str() == nested
        ));

        let many = (0..600).map(|_| "Leaf").collect::<Vec<_>>().join(",");
        let many = format!("({many})");
        assert!(matches!(
            extract(&many, &[], &[], &[]),
            Err(TypeRefPathExtractionError::ResourceLimitExceeded { location }) if location.as_str() == many
        ));
    }
}
