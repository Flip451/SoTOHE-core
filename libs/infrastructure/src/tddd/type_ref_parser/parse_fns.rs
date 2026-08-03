//! Public parse entry points.

use std::collections::HashMap;

use rustdoc_types::{GenericBound, Id, Path, TraitBoundModifier, Type};
use syn::visit::Visit;

use super::constants::UNRESOLVED_CRATE_ID;
use super::generic_tokens;
use super::parse_ctx::{ParseCtx, bound_lifetimes_to_generic_params};
use super::precise_capture::convert_precise_capture;

/// Validates the declared generic-parameter context using plain lexical rules.
/// Type/bound strings themselves are not interpreted here; `syn` and chain ③
/// remain the source of syntax validity and mismatch visibility.
pub(crate) fn validate_generic_identifier_ambiguities(
    input: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    generic_tokens::validate(input, generic_params)
}

pub(crate) fn parse_type_ref<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_type_ref_with_generics(
        type_ref_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        &[],
    )
}

pub(crate) fn parse_type_ref_with_generics<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_type_ref_with_generics_inner(
        type_ref_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        false,
    )
}

/// Parses a type reference while preserving the source spelling of prelude
/// paths for lexical comparison with rustdoc output.
pub(crate) fn parse_type_ref_with_generics_preserving_spelling<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_type_ref_with_generics_inner(
        type_ref_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        true,
    )
}

fn parse_type_ref_with_generics_inner<F, G>(
    type_ref_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
    preserve_prelude_spelling: bool,
) -> Result<Type, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    validate_generic_identifier_ambiguities(type_ref_str, generic_params)?;
    let syn_type: syn::Type = syn::parse_str(type_ref_str)
        .map_err(|e| format!("syn parse error for `{type_ref_str}`: {e}"))?;
    reject_anonymous_const_blocks_in_type(&syn_type)?;
    if preserve_prelude_spelling {
        reject_unsupported_type_macros_in_type(&syn_type)?;
        reject_unsupported_array_lengths_in_type(&syn_type)?;
        reject_unsupported_const_arguments_in_type(&syn_type)?;
    }
    let mut ctx = ParseCtx {
        resolve_local,
        external_crate_ids,
        emit_external_crate,
        std_crate_id,
        generic_params,
        preserve_prelude_spelling,
    };
    Ok(ctx.convert_type(&syn_type))
}

pub(crate) fn parse_syn_type(type_ref_str: &str) -> syn::Result<syn::Type> {
    syn::parse_str(type_ref_str)
}

pub(crate) fn parse_syn_type_param_bound(type_ref_str: &str) -> syn::Result<syn::TypeParamBound> {
    syn::parse_str(type_ref_str)
}

/// Validates a type reference while checking const-generic arguments whose
/// renderer would otherwise collapse to `<const_expr>`.  Array-length syntax
/// is intentionally not restricted here because ordinary where-clause type
/// positions use the general array renderer.
pub(crate) fn validate_const_arguments_in_type_ref(
    type_ref_str: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    validate_generic_identifier_ambiguities(type_ref_str, generic_params)?;
    let syntax: syn::Type = syn::parse_str(type_ref_str)
        .map_err(|e| format!("invalid type syntax '{type_ref_str}': {e}"))?;
    reject_anonymous_const_blocks_in_type(&syntax)?;
    reject_unsupported_type_macros_in_type(&syntax)?;
    reject_unsupported_const_arguments_in_type(&syntax)
}

/// Validates a generic bound for the lexical alias-comparison path without
/// converting it to a rustdoc type.  The document codec uses this same
/// `syn`-backed gate so an alias bound cannot pass decoding and fail later in
/// the preserving-spelling encoder.
pub(crate) fn validate_lexical_generic_bound(
    bound_str: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    validate_generic_identifier_ambiguities(bound_str, generic_params)?;
    let syn_bound: syn::TypeParamBound = syn::parse_str(bound_str)
        .map_err(|e| format!("invalid bound syntax '{bound_str}': {e}"))?;
    reject_anonymous_const_blocks_in_bound(&syn_bound)?;
    reject_unsupported_type_macros_in_bound(&syn_bound)?;
    reject_unsupported_array_lengths_in_bound(&syn_bound)?;
    reject_unsupported_const_arguments_in_bound(&syn_bound)
}

pub(crate) fn parse_generic_bound_with_generics<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_generic_bound_with_generics_inner(
        bound_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        false,
    )
}

/// Parses a generic bound while preserving the source spelling of prelude
/// paths.  Alias declarations are compared lexically against rustdoc output,
/// so `Clone` must remain `Clone` rather than being expanded to
/// `std::clone::Clone` by the general type resolver.
pub(crate) fn parse_generic_bound_with_generics_preserving_spelling<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    parse_generic_bound_with_generics_inner(
        bound_str,
        resolve_local,
        std_crate_id,
        external_crate_ids,
        emit_external_crate,
        generic_params,
        true,
    )
}

fn parse_generic_bound_with_generics_inner<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
    generic_params: &[&str],
    preserve_prelude_spelling: bool,
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    validate_generic_identifier_ambiguities(bound_str, generic_params)?;
    let syn_bound: syn::TypeParamBound =
        syn::parse_str(bound_str).map_err(|e| format!("syn parse error for `{bound_str}`: {e}"))?;
    reject_anonymous_const_blocks_in_bound(&syn_bound)?;
    if preserve_prelude_spelling {
        reject_unsupported_type_macros_in_bound(&syn_bound)?;
        reject_unsupported_array_lengths_in_bound(&syn_bound)?;
        reject_unsupported_const_arguments_in_bound(&syn_bound)?;
    }

    let mut ctx = ParseCtx {
        resolve_local,
        external_crate_ids,
        emit_external_crate,
        std_crate_id,
        generic_params,
        preserve_prelude_spelling,
    };
    match syn_bound {
        syn::TypeParamBound::Lifetime(lt) => Ok(GenericBound::Outlives(format!("'{}", lt.ident))),
        syn::TypeParamBound::Trait(tb) => {
            let modifier = match tb.modifier {
                syn::TraitBoundModifier::None => TraitBoundModifier::None,
                syn::TraitBoundModifier::Maybe(_) => TraitBoundModifier::Maybe,
            };
            let generic_params = bound_lifetimes_to_generic_params(tb.lifetimes.as_ref());
            let trait_path = ctx.resolve_trait_bound_path(&tb.path);
            Ok(GenericBound::TraitBound { trait_: trait_path, generic_params, modifier })
        }
        syn::TypeParamBound::PreciseCapture(capture) => Ok(convert_precise_capture(&capture)),
        _ => Ok(GenericBound::TraitBound {
            trait_: Path { path: bound_str.to_string(), id: Id(UNRESOLVED_CRATE_ID), args: None },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }),
    }
}

/// Rejects complex anonymous const blocks after `syn` has parsed the expression.
///
/// The lexical adapter must not manufacture rustdoc's information-free `{ _ }` placeholder:
/// rustdoc does not retain enough data to distinguish two such blocks.  Visiting the parsed AST
/// keeps this guard precise (braces in comments or literals are not mistaken for blocks) without
/// reimplementing Rust's expression grammar.
#[derive(Default)]
struct AnonymousConstBlockVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for AnonymousConstBlockVisitor {
    fn visit_expr_block(&mut self, node: &'ast syn::ExprBlock) {
        self.found = true;
        syn::visit::visit_expr_block(self, node);
    }
}

fn reject_anonymous_const_blocks_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = AnonymousConstBlockVisitor::default();
    visitor.visit_type(syntax);
    reject_if_anonymous_const_block_found(visitor.found)
}

fn reject_anonymous_const_blocks_in_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    let mut visitor = AnonymousConstBlockVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_anonymous_const_block_found(visitor.found)
}

/// Rejects type macros in lexical comparison paths. `syn` can identify the
/// macro invocation but cannot expand it; converting it to `<unknown_type>`
/// would make distinct expanded types compare equal.
#[derive(Default)]
struct UnsupportedTypeMacroVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for UnsupportedTypeMacroVisitor {
    fn visit_type_macro(&mut self, node: &'ast syn::TypeMacro) {
        self.found = true;
        syn::visit::visit_type_macro(self, node);
    }
}

fn reject_unsupported_type_macros_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = UnsupportedTypeMacroVisitor::default();
    visitor.visit_type(syntax);
    reject_if_unsupported_type_macro_found(visitor.found)
}

fn reject_unsupported_type_macros_in_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    let mut visitor = UnsupportedTypeMacroVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_unsupported_type_macro_found(visitor.found)
}

fn reject_if_unsupported_type_macro_found(found: bool) -> Result<(), String> {
    if found {
        Err(
            "type macros are not supported by lexical type comparison because their expansion cannot be represented"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

/// Rejects array-length expressions whose spelling cannot be compared safely
/// against rustdoc's normalized representation.
///
/// The lexical adapter deliberately accepts only the expression forms already
/// handled by [`super::helpers::array_len_to_string`]: integer literals,
/// parenthesized forms, and the supported binary operators. Named constants
/// are rejected because rustdoc may evaluate them while the catalogue has no
/// value environment with which to reproduce that normalization. Method calls
/// (for example `10usize.pow(2)`), unary expressions, casts, and other
/// expressions are rejected instead of being rendered as a lossy marker.
#[derive(Default)]
struct UnsupportedArrayLengthVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for UnsupportedArrayLengthVisitor {
    fn visit_type_array(&mut self, node: &'ast syn::TypeArray) {
        if !is_supported_array_length_expr(&node.len) {
            self.found = true;
        }
        syn::visit::visit_type_array(self, node);
    }
}

fn is_supported_array_length_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(lit) => matches!(lit.lit, syn::Lit::Int(_)),
        syn::Expr::Path(_) => false,
        syn::Expr::Paren(paren) => is_supported_array_length_expr(&paren.expr),
        syn::Expr::Binary(binary) => {
            matches!(
                binary.op,
                syn::BinOp::Add(_)
                    | syn::BinOp::Sub(_)
                    | syn::BinOp::Mul(_)
                    | syn::BinOp::Div(_)
                    | syn::BinOp::Rem(_)
                    | syn::BinOp::BitAnd(_)
                    | syn::BinOp::BitOr(_)
                    | syn::BinOp::BitXor(_)
                    | syn::BinOp::Shl(_)
                    | syn::BinOp::Shr(_)
            ) && is_supported_array_length_expr(&binary.left)
                && is_supported_array_length_expr(&binary.right)
        }
        _ => false,
    }
}

fn reject_unsupported_array_lengths_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = UnsupportedArrayLengthVisitor::default();
    visitor.visit_type(syntax);
    reject_if_unsupported_array_length_found(visitor.found)
}

fn reject_unsupported_array_lengths_in_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    let mut visitor = UnsupportedArrayLengthVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_unsupported_array_length_found(visitor.found)
}

/// Rejects const generic arguments that the lexical renderer would collapse
/// to its information-free placeholder. The preserving path is deliberately
/// fail-closed for compound expressions; ordinary type parsing keeps its
/// historical permissive behavior.
#[derive(Default)]
struct UnsupportedConstArgumentVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for UnsupportedConstArgumentVisitor {
    fn visit_generic_argument(&mut self, node: &'ast syn::GenericArgument) {
        match node {
            syn::GenericArgument::Const(expr) => {
                if !is_supported_const_argument_expr(expr) {
                    self.found = true;
                }
            }
            syn::GenericArgument::AssocConst(assoc_const) => {
                if !is_supported_const_argument_expr(&assoc_const.value) {
                    self.found = true;
                }
            }
            _ => {}
        }
        syn::visit::visit_generic_argument(self, node);
    }
}

fn is_supported_const_argument_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(lit) => {
            matches!(
                lit.lit,
                syn::Lit::Int(_)
                    | syn::Lit::Str(_)
                    | syn::Lit::Bool(_)
                    | syn::Lit::Char(_)
                    | syn::Lit::Byte(_)
            )
        }
        syn::Expr::Path(path) => {
            path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path
                    .path
                    .segments
                    .iter()
                    .all(|segment| matches!(segment.arguments, syn::PathArguments::None))
        }
        syn::Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_))
                && matches!(&*unary.expr, syn::Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Int(_)))
        }
        _ => false,
    }
}

fn reject_unsupported_const_arguments_in_type(syntax: &syn::Type) -> Result<(), String> {
    let mut visitor = UnsupportedConstArgumentVisitor::default();
    visitor.visit_type(syntax);
    reject_if_unsupported_const_argument_found(visitor.found)
}

fn reject_unsupported_const_arguments_in_bound(syntax: &syn::TypeParamBound) -> Result<(), String> {
    let mut visitor = UnsupportedConstArgumentVisitor::default();
    visitor.visit_type_param_bound(syntax);
    reject_if_unsupported_const_argument_found(visitor.found)
}

fn reject_if_unsupported_const_argument_found(found: bool) -> Result<(), String> {
    if found {
        Err(
            "const generic argument expressions outside the supported lexical forms are not supported by lexical type comparison"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

fn reject_if_unsupported_array_length_found(found: bool) -> Result<(), String> {
    if found {
        Err(
            "array length expressions outside the supported lexical forms are not supported by lexical type comparison"
                .to_owned(),
        )
    } else {
        Ok(())
    }
}

fn reject_if_anonymous_const_block_found(found: bool) -> Result<(), String> {
    if found {
        Err("anonymous const/block expressions are not supported by lexical type comparison"
            .to_owned())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rustdoc_types::Type;

    #[test]
    fn test_plain_generic_name_parses() {
        let parsed = super::parse_type_ref_with_generics(
            "Option<T>",
            &|_| None,
            0,
            &HashMap::new(),
            &mut |_| 1,
            &["T"],
        );
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_keyword_generic_name_is_rejected_lexically() {
        let result = super::validate_generic_identifier_ambiguities("Vec<type>", &["type"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_raw_generic_name_is_rejected_lexically() {
        let result = super::validate_generic_identifier_ambiguities("Vec<T>", &["r#T"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_weak_keyword_generic_names_are_rejected_lexically() {
        for name in ["macro_rules", "raw", "safe"] {
            assert!(
                super::validate_generic_identifier_ambiguities("Vec<T>", &[name]).is_err(),
                "weak keyword `{name}` must be rejected"
            );
        }
    }

    #[test]
    fn test_primitive_spelled_generic_remains_a_plain_name() {
        let parsed = super::parse_type_ref_with_generics(
            "bool",
            &|_| None,
            0,
            &HashMap::new(),
            &mut |_| 1,
            &["Bool"],
        );
        assert_eq!(parsed, Ok(Type::Primitive("bool".to_owned())));
    }
}
