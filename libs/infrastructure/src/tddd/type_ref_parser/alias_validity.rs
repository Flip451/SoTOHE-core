//! Guards for alias declarations rustc itself refuses to compile.
//!
//! Unlike the spelling guards in the sibling `bound_spelling` module — which
//! reject notations the structural representation cannot retain — these
//! guards reject declarations that are outright invalid in a type-alias item,
//! so admitting them would record an external contract no implementation can
//! satisfy.

use syn::visit::Visit;

/// Rust permits only a single explicit lifetime bound on a trait object
/// (E0226), and the converter keeps only the first, so `dyn Tr + 'static +
/// 'static` would record an alias contract no implementation can compile.
/// A trait object carrying more than one lifetime bound is rejected.
#[derive(Default)]
struct MultipleDynLifetimeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for MultipleDynLifetimeVisitor {
    fn visit_type_trait_object(&mut self, node: &'ast syn::TypeTraitObject) {
        self.found |= node
            .bounds
            .iter()
            .filter(|bound| matches!(bound, syn::TypeParamBound::Lifetime(_)))
            .count()
            > 1;
        syn::visit::visit_type_trait_object(self, node);
    }
}

pub(super) fn reject_multiple_dyn_lifetimes_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = MultipleDynLifetimeVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_multiple_dyn_lifetimes_found(visitor.found)
}

pub(super) fn reject_multiple_dyn_lifetimes_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = MultipleDynLifetimeVisitor::default();
    visitor.visit_type(syntax);
    reject_if_multiple_dyn_lifetimes_found(visitor.found)
}

fn reject_if_multiple_dyn_lifetimes_found(found: bool) -> Result<(), String> {
    if found {
        Err("a trait object permits only a single explicit lifetime bound".to_owned())
    } else {
        Ok(())
    }
}

/// Rustc rejects `impl Trait` in generic arguments (E0562) and `Self` in
/// top-level type-alias declarations (E0411), so either node would record an
/// alias contract no implementation can compile. Both are rejected at the
/// boundary.
#[derive(Default)]
struct AliasInvalidNodeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for AliasInvalidNodeVisitor {
    fn visit_type_impl_trait(&mut self, node: &'ast syn::TypeImplTrait) {
        self.found = true;
        syn::visit::visit_type_impl_trait(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.found |= node.segments.iter().any(|segment| segment.ident == "Self");
        syn::visit::visit_path(self, node);
    }
}

pub(super) fn reject_alias_invalid_nodes_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = AliasInvalidNodeVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_alias_invalid_node_found(visitor.found)
}

pub(super) fn reject_alias_invalid_nodes_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = AliasInvalidNodeVisitor::default();
    visitor.visit_type(syntax);
    reject_if_alias_invalid_node_found(visitor.found)
}

fn reject_if_alias_invalid_node_found(found: bool) -> Result<(), String> {
    if found {
        Err("`impl Trait` and `Self` are not valid in type-alias declarations".to_owned())
    } else {
        Ok(())
    }
}

/// The alias model declares only type parameters, so a lifetime bound can
/// reference no declared lifetime: rustc rejects `type A<T: 'a> = T` for the
/// undeclared `'a`. Only `'static` is always in scope and stays accepted.
pub(super) fn reject_undeclared_lifetime_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    if let syn::TypeParamBound::Lifetime(lifetime) = syntax {
        if lifetime.ident != "static" {
            return Err(format!(
                "lifetime bound `'{}` references no declarable lifetime parameter; only `'static` is supported",
                lifetime.ident
            ));
        }
    }
    Ok(())
}

/// Rust rejects the infer placeholder `_` in type-alias item signatures
/// (E0121), so a bound such as `T: Outer<_>` would record an alias contract
/// no implementation can compile. Nested `Type::Infer` nodes are rejected at
/// the boundary.
#[derive(Default)]
struct InferPlaceholderVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for InferPlaceholderVisitor {
    fn visit_type_infer(&mut self, node: &'ast syn::TypeInfer) {
        self.found = true;
        syn::visit::visit_type_infer(self, node);
    }
}

pub(super) fn reject_infer_placeholders_in_bound(
    syntax: &syn::TypeParamBound,
) -> Result<(), String> {
    let mut visitor = InferPlaceholderVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_infer_placeholder_found(visitor.found)
}

pub(super) fn reject_infer_placeholders_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = InferPlaceholderVisitor::default();
    visitor.visit_type(syntax);
    reject_if_infer_placeholder_found(visitor.found)
}

fn reject_if_infer_placeholder_found(found: bool) -> Result<(), String> {
    if found {
        Err("infer placeholders (`_`) are not valid in type-alias signatures".to_owned())
    } else {
        Ok(())
    }
}
