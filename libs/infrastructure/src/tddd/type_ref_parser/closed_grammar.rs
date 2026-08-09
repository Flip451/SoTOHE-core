//! The closed acceptance grammar for alias lexical validation.
//!
//! Thirty-plus review rounds showed that rejecting unrepresentable spellings
//! one denylist entry at a time never converges: the set of `syn`-parseable
//! legal Rust is open, so a finite denylist is inherently fail-open. This
//! module closes the grammar with two rules instead:
//!
//! - **Rule A — canonical round-trip.** A catalogue string is accepted only
//!   when it token-equals the canonical rendering
//!   (`canonical_render`) of its own converted representation. Every spelling
//!   the converter or rustdoc silently normalizes — turbofish, redundant
//!   parentheses, trailing punctuation, implicit ABIs, placeholder lifetimes
//!   and names, evaluated const forms — fails this gate with no per-spelling
//!   recognition code, because the conversion drops the information and the
//!   canonical rendering no longer matches.
//! - **Rule B — syntax allowlist.** The accepted syntax classes are defined
//!   positively from what rustdoc can emit for a generic type alias
//!   bound / target position. AST node kinds outside the list, attributes,
//!   lifetimes that are neither `'static` nor introduced by a visible `for<>`
//!   binder, trait paths that cannot name a trait, non-`Sized` relaxed
//!   bounds, and precise-capture lists are rejected by default, so novel
//!   constructs fail closed without new code.
//!
//! Expression-level limits for const arguments and array lengths remain in
//! `parse_fns` (`is_simple_const_expr` / `is_supported_array_length_expr`);
//! they are the same allowlist idea applied to expressions.
//!
//! The grammar governs the CATALOGUE side of the comparison. The
//! implementation side is observed only through rustdoc's representation,
//! which itself normalizes source-only spelling variance (turbofish,
//! trailing commas, redundant parentheses, …) before this crate ever sees
//! it. Those implementation-side distinctions are therefore contractually
//! identified with their canonical form: the lexical contract is defined
//! over the rustdoc representation, not over unobservable source bytes.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use rustdoc_types::Id;
use syn::visit::Visit;

use super::canonical_render::{render_bound, render_type};
use super::constants::PRIMITIVE_TYPES;
use super::parse_ctx::ParseCtx;

const NON_CANONICAL: &str = "non-canonical spelling: the catalogue must use the canonical form of the declaration \
     (the spelling rustdoc itself emits); this variant is normalized away by the \
     representation and cannot be compared lexically";

const UNSUPPORTED_SYNTAX: &str = "unsupported syntax element: the alias lexical grammar accepts only constructs that \
     rustdoc can emit for a generic type alias declaration";

/// Rule A + Rule B for a bound string (a single `TypeParamBound`).
///
/// A supported `~const ` prefix is stripped before both rules; its deeper
/// shape is validated separately by `validate_maybe_const_bound`.
pub(crate) fn enforce_closed_bound_grammar(
    bound_str: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    let stripped = bound_str.strip_prefix("~const ").map_or(bound_str, str::trim_start);
    let syn_bound: syn::TypeParamBound =
        syn::parse_str(stripped).map_err(|e| format!("invalid bound syntax '{bound_str}': {e}"))?;

    let mut visitor = AllowlistVisitor::new(generic_params);
    visitor.visit_type_param_bound(&syn_bound);
    visitor.finish()?;

    let converted = convert_bound_with_dummy_context(&syn_bound, generic_params);
    let canonical = render_bound(&converted).ok_or_else(|| UNSUPPORTED_SYNTAX.to_owned())?;
    if tokens_match(&canonical, stripped) {
        Ok(())
    } else {
        Err(format!("{NON_CANONICAL}; expected `{canonical}`"))
    }
}

/// Rule A + Rule B for a type string (`syn::Type`).
pub(crate) fn enforce_closed_type_grammar(
    type_ref_str: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    enforce_type_grammar_with_policy(type_ref_str, generic_params, LifetimePolicy::ScopedOnly)
}

/// Rule A + Rule B for a type-alias TARGET string.
///
/// The catalogue schema declares only type parameters, so an alias whose real
/// declaration carries a lifetime parameter records that lifetime lexically in
/// the target (an established modeling convention, e.g.
/// `Pin<Box<dyn Future<…> + Send + 'a>>`). A named lifetime in a target is a
/// reference to a source-declared lifetime parameter the schema cannot
/// express — not an undeclared lifetime in the rustc sense (E0261 judges the
/// real declaration, which does declare it). Free named lifetimes are
/// therefore in scope for targets; the anonymous `'_` stays rejected because
/// rustc forbids it in alias signatures.
pub(crate) fn enforce_closed_alias_target_grammar(
    type_ref_str: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    enforce_type_grammar_with_policy(type_ref_str, generic_params, LifetimePolicy::AnyNamed)
}

fn enforce_type_grammar_with_policy(
    type_ref_str: &str,
    generic_params: &[&str],
    lifetime_policy: LifetimePolicy,
) -> Result<(), String> {
    let syn_type: syn::Type = syn::parse_str(type_ref_str)
        .map_err(|e| format!("invalid type syntax '{type_ref_str}': {e}"))?;

    let mut visitor = AllowlistVisitor::with_lifetime_policy(generic_params, lifetime_policy);
    visitor.visit_type(&syn_type);
    visitor.finish()?;

    let converted = convert_type_with_dummy_context(&syn_type, generic_params);
    let canonical = render_type(&converted).ok_or_else(|| UNSUPPORTED_SYNTAX.to_owned())?;
    if tokens_match(&canonical, type_ref_str) {
        Ok(())
    } else {
        Err(format!("{NON_CANONICAL}; expected `{canonical}`"))
    }
}

/// Token-level equality: both spellings must produce the same token stream
/// (whitespace differences are the only tolerated variance).
fn tokens_match(a: &str, b: &str) -> bool {
    let (Ok(tokens_a), Ok(tokens_b)) = (a.parse::<TokenStream>(), b.parse::<TokenStream>()) else {
        return false;
    };
    tokens_a.to_string() == tokens_b.to_string()
}

fn convert_bound_with_dummy_context(
    syn_bound: &syn::TypeParamBound,
    generic_params: &[&str],
) -> rustdoc_types::GenericBound {
    with_dummy_context(generic_params, |ctx| match syn_bound {
        syn::TypeParamBound::Lifetime(lt) => {
            rustdoc_types::GenericBound::Outlives(format!("'{}", lt.ident))
        }
        syn::TypeParamBound::Trait(tb) => {
            let modifier = match tb.modifier {
                syn::TraitBoundModifier::None => rustdoc_types::TraitBoundModifier::None,
                syn::TraitBoundModifier::Maybe(_) => rustdoc_types::TraitBoundModifier::Maybe,
            };
            let generic_params_rendered =
                super::parse_ctx::bound_lifetimes_to_generic_params(tb.lifetimes.as_ref());
            let trait_path = ctx.resolve_trait_bound_path(&tb.path);
            rustdoc_types::GenericBound::TraitBound {
                trait_: trait_path,
                generic_params: generic_params_rendered,
                modifier,
            }
        }
        syn::TypeParamBound::PreciseCapture(capture) => {
            super::precise_capture::convert_precise_capture(capture)
        }
        _ => rustdoc_types::GenericBound::TraitBound {
            trait_: rustdoc_types::Path {
                path: "<unsupported_bound>".to_owned(),
                id: Id(u32::MAX),
                args: None,
            },
            generic_params: vec![],
            modifier: rustdoc_types::TraitBoundModifier::None,
        },
    })
}

fn convert_type_with_dummy_context(
    syn_type: &syn::Type,
    generic_params: &[&str],
) -> rustdoc_types::Type {
    with_dummy_context(generic_params, |ctx| ctx.convert_type(syn_type))
}

/// Runs a conversion with inert resolver context. Resolution affects only
/// graph IDs and external-crate registration, never the preserved spelling,
/// so the canonical rendering is identical to the real encoding pass.
fn with_dummy_context<R>(
    generic_params: &[&str],
    convert: impl FnOnce(&mut ParseCtx<'_, fn(&str) -> Option<Id>, Box<dyn FnMut(String) -> u32>>) -> R,
) -> R {
    fn no_local(_name: &str) -> Option<Id> {
        None
    }
    let external_crate_ids: HashMap<String, u32> = HashMap::new();
    let mut emit: Box<dyn FnMut(String) -> u32> = Box::new(|_name: String| 0);
    let mut ctx = ParseCtx {
        resolve_local: &(no_local as fn(&str) -> Option<Id>),
        external_crate_ids: &external_crate_ids,
        emit_external_crate: &mut emit,
        std_crate_id: 0,
        generic_params,
        preserve_prelude_spelling: true,
    };
    convert(&mut ctx)
}

// ---------------------------------------------------------------------------
// Rule B — syntax allowlist visitor
// ---------------------------------------------------------------------------

/// How lifetime USES outside a binder are judged.
#[derive(Clone, Copy)]
enum LifetimePolicy {
    /// Bound / where positions: only `'static` or binder-introduced names.
    ScopedOnly,
    /// Alias targets: source-declarable named lifetimes (the schema cannot
    /// declare lifetime parameters, so targets carry them lexically); `'_`
    /// and Rust's reserved lifetime names stay rejected.
    AnyNamed,
}

/// The lifetime name used for Rust declaration/scope comparison. Raw
/// identifiers and their ordinary spelling name the same lifetime.
fn normalized_lifetime_name(name: &str) -> &str {
    name.strip_prefix("r#").unwrap_or(name)
}

/// Whether a lifetime spelling is reserved where a source declaration must
/// introduce a user-defined lifetime. `syn` validates token syntax; this
/// closes the remaining rustc-reserved names while leaving keyword and Unicode
/// lifetime names available.
fn is_reserved_declared_lifetime_name(name: &str) -> bool {
    matches!(normalized_lifetime_name(name), "_" | "self" | "Self" | "super" | "crate")
        || name == "r#static"
}

struct AllowlistVisitor<'params, 'names> {
    generic_params: &'params [&'names str],
    /// Lifetime names (without `'`) introduced by enclosing `for<>` binders.
    lifetime_scope: Vec<String>,
    lifetime_policy: LifetimePolicy,
    rejection: Option<String>,
}

impl<'params, 'names> AllowlistVisitor<'params, 'names> {
    fn new(generic_params: &'params [&'names str]) -> Self {
        Self::with_lifetime_policy(generic_params, LifetimePolicy::ScopedOnly)
    }

    fn with_lifetime_policy(
        generic_params: &'params [&'names str],
        lifetime_policy: LifetimePolicy,
    ) -> Self {
        Self { generic_params, lifetime_scope: vec![], lifetime_policy, rejection: None }
    }

    fn finish(self) -> Result<(), String> {
        match self.rejection {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    fn reject(&mut self, detail: &str) {
        if self.rejection.is_none() {
            self.rejection = Some(format!("{UNSUPPORTED_SYNTAX}: {detail}"));
        }
    }

    fn check_lifetime_use(&mut self, lifetime: &syn::Lifetime) {
        let name = lifetime.ident.to_string();
        let normalized_name = normalized_lifetime_name(&name);
        match self.lifetime_policy {
            LifetimePolicy::ScopedOnly => {
                if name == "r#static"
                    || (normalized_name != "static"
                        && !self.lifetime_scope.iter().any(|scoped| scoped == normalized_name))
                {
                    self.reject(&format!(
                        "lifetime `'{name}` is neither `'static` nor introduced by a visible \
                         `for<>` binder"
                    ));
                }
            }
            LifetimePolicy::AnyNamed => {
                // `syn` has already checked lifetime-token syntax.  These
                // are the remaining reserved forms that rustc does not allow
                // in lifetime declarations; raw keyword names such as
                // `'r#async` remain source-declarable and are accepted.
                if is_reserved_declared_lifetime_name(&name) {
                    self.reject(&format!(
                        "lifetime `'{name}` is not valid in a type-alias signature"
                    ));
                }
            }
        }
    }

    /// Pushes the binder's lifetimes into scope (validating the binder itself)
    /// and returns how many names were added.
    fn enter_binder(&mut self, lifetimes: Option<&syn::BoundLifetimes>) -> usize {
        let Some(binder) = lifetimes else {
            return 0;
        };
        let mut added = 0;
        for param in &binder.lifetimes {
            match param {
                syn::GenericParam::Lifetime(lifetime_param) => {
                    if !lifetime_param.attrs.is_empty() {
                        self.reject("attributes on binder parameters");
                    }
                    // Rustc rejects lifetime bounds inside an HRTB binder
                    // ("bounds cannot be used in this context"), so
                    // `for<'a: 'static>` cannot appear in rustdoc output.
                    if !lifetime_param.bounds.is_empty() {
                        self.reject("lifetime bounds cannot appear in a `for<>` binder");
                    }
                    // A binder must DECLARE fresh names: reserved names are
                    // rejected by rustc (E0262), and a name already in scope is
                    // a duplicate or shadowing declaration (E0403 / E0496).
                    let name = lifetime_param.lifetime.ident.to_string();
                    let normalized_name = normalized_lifetime_name(&name);
                    if normalized_name == "static" || is_reserved_declared_lifetime_name(&name) {
                        self.reject(&format!("`'{name}` cannot be declared by a `for<>` binder"));
                    }
                    if self.lifetime_scope.iter().any(|scoped| scoped == normalized_name) {
                        self.reject(&format!(
                            "binder lifetime `'{name}` duplicates or shadows a lifetime already \
                             in scope"
                        ));
                    }
                    self.lifetime_scope.push(normalized_name.to_owned());
                    added += 1;
                }
                syn::GenericParam::Type(_) | syn::GenericParam::Const(_) => {
                    self.reject("non-lifetime binder parameters");
                }
            }
        }
        added
    }

    fn exit_binder(&mut self, added: usize) {
        let new_len = self.lifetime_scope.len().saturating_sub(added);
        self.lifetime_scope.truncate(new_len);
    }

    /// A trait-position path must be able to name a trait: single-segment
    /// primitives and paths rooted at a declared generic parameter cannot.
    fn check_trait_path(&mut self, path: &syn::Path) {
        let Some(first) = path.segments.first() else {
            self.reject("empty trait path");
            return;
        };
        let first_name = first.ident.to_string();
        if path.segments.len() == 1 && PRIMITIVE_TYPES.contains(&first_name.as_str()) {
            self.reject("a primitive type cannot be used as a trait bound");
        }
        if (path.leading_colon.is_none() || path.segments.len() == 1)
            && self.generic_params.iter().any(|generic| *generic == first_name)
        {
            self.reject("a declared generic parameter cannot be used as a trait bound");
        }
    }
}

impl<'ast, 'params, 'names> Visit<'ast> for AllowlistVisitor<'params, 'names> {
    fn visit_type(&mut self, node: &'ast syn::Type) {
        match node {
            syn::Type::Path(_)
            | syn::Type::Reference(_)
            | syn::Type::Ptr(_)
            | syn::Type::Slice(_)
            | syn::Type::Array(_)
            | syn::Type::Tuple(_)
            | syn::Type::BareFn(_)
            | syn::Type::TraitObject(_)
            | syn::Type::Never(_)
            | syn::Type::Paren(_) => syn::visit::visit_type(self, node),
            syn::Type::ImplTrait(_) => {
                self.reject("`impl Trait` is not valid in a type-alias declaration");
            }
            syn::Type::Infer(_) => {
                self.reject("the infer placeholder `_` is not valid in a type-alias declaration");
            }
            syn::Type::Macro(_) => {
                self.reject("type macros cannot be represented for lexical comparison");
            }
            _ => self.reject("unrecognized type syntax"),
        }
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if let Some(qself) = &node.qself {
            if qself.position > 0 {
                let prefix: syn::punctuated::Punctuated<syn::PathSegment, syn::Token![::]> =
                    node.path.segments.iter().take(qself.position).cloned().collect();
                let trait_path =
                    syn::Path { leading_colon: node.path.leading_colon, segments: prefix };
                self.check_trait_path(&trait_path);
            }
        }
        syn::visit::visit_type_path(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        if node.segments.iter().any(|segment| segment.ident == "Self") {
            self.reject("`Self` is not valid in a type-alias declaration");
        }
        syn::visit::visit_path(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.path.segments.iter().any(|segment| segment.ident.to_string().starts_with("r#")) {
            self.reject("raw identifiers are not valid in const expressions");
        }
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_type_param_bound(&mut self, node: &'ast syn::TypeParamBound) {
        match node {
            syn::TypeParamBound::Trait(_) | syn::TypeParamBound::Lifetime(_) => {
                syn::visit::visit_type_param_bound(self, node);
            }
            syn::TypeParamBound::PreciseCapture(_) => {
                self.reject("`use<..>` precise-capture lists are not valid bounds here");
            }
            _ => self.reject("unrecognized bound syntax"),
        }
    }

    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        if let syn::TraitBoundModifier::Maybe(_) = node.modifier {
            let is_sized = node.path.segments.len() == 1
                && node.path.segments.first().is_some_and(|segment| segment.ident == "Sized")
                && node.path.leading_colon.is_none();
            if !is_sized {
                self.reject("`?` may only relax the `Sized` bound");
            }
        }
        self.check_trait_path(&node.path);
        let added = self.enter_binder(node.lifetimes.as_ref());
        self.visit_path(&node.path);
        self.exit_binder(added);
    }

    fn visit_type_bare_fn(&mut self, node: &'ast syn::TypeBareFn) {
        // Rustc rejects unrecognized ABI names outright (E0703); only the
        // stable calling conventions can appear in compiler-validated rustdoc
        // output. This includes the conventions the converter keeps as
        // `Abi::Other` (`efiapi`, the `thiscall` family).
        if let Some(name) = node.abi.as_ref().and_then(|abi| abi.name.as_ref()) {
            const SUPPORTED_ABI_NAMES: &[&str] = &[
                "Rust",
                "C",
                "C-unwind",
                "cdecl",
                "cdecl-unwind",
                "stdcall",
                "stdcall-unwind",
                "fastcall",
                "fastcall-unwind",
                "aapcs",
                "aapcs-unwind",
                "win64",
                "win64-unwind",
                "sysv64",
                "sysv64-unwind",
                "system",
                "system-unwind",
                "efiapi",
                "thiscall",
                "thiscall-unwind",
            ];
            if !SUPPORTED_ABI_NAMES.contains(&name.value().as_str()) {
                self.reject(&format!(
                    "`extern \"{}\"` is not a calling convention rustc supports (E0703)",
                    name.value()
                ));
            }
        }
        let added = self.enter_binder(node.lifetimes.as_ref());
        for input in &node.inputs {
            if !input.attrs.is_empty() {
                self.reject("attributes on bare-function parameters");
            }
            self.visit_type(&input.ty);
        }
        if let Some(variadic) = &node.variadic {
            if !variadic.attrs.is_empty() {
                self.reject("attributes on variadic parameters");
            }
            if variadic.name.is_some() {
                self.reject("named variadic parameters cannot be represented");
            }
            // Rustc rejects C-variadic function pointers without a compatible
            // calling convention (E0045). The compatible set under the
            // workspace toolchain is the C / cdecl families plus the
            // compiler-supported platform conventions: `sysv64`, `win64`,
            // `efiapi`, `aapcs`, their supported unwind forms, and `system`.
            // C23 variadics (Rust 1.80) permit an empty named-parameter
            // list, so no arity restriction applies.
            let abi_compatible =
                node.abi.as_ref().and_then(|abi| abi.name.as_ref()).is_some_and(|name| {
                    matches!(
                        name.value().as_str(),
                        "C" | "C-unwind"
                            | "cdecl"
                            | "cdecl-unwind"
                            | "sysv64"
                            | "sysv64-unwind"
                            | "win64"
                            | "win64-unwind"
                            | "efiapi"
                            | "aapcs"
                            | "aapcs-unwind"
                            | "system"
                            | "system-unwind"
                    )
                });
            if !abi_compatible {
                self.reject(
                    "C-variadic function pointers require an explicit compatible calling \
                     convention (the C / cdecl families or a supported platform ABI)",
                );
            }
        }
        if let syn::ReturnType::Type(_, output) = &node.output {
            self.visit_type(output);
        }
        self.exit_binder(added);
    }

    fn visit_lifetime(&mut self, node: &'ast syn::Lifetime) {
        self.check_lifetime_use(node);
        syn::visit::visit_lifetime(self, node);
    }
}
