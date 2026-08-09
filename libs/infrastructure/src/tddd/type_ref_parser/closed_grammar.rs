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

use proc_macro2::TokenStream;
use syn::visit::Visit;

use super::canonical_render::{render_bound, render_type};
use super::constants::PRIMITIVE_TYPES;
use super::dummy_context::{convert_bound_with_dummy_context, convert_type_with_dummy_context};

const NON_CANONICAL: &str = "non-canonical spelling: the catalogue must use the canonical form of the declaration \
     (the spelling rustdoc itself emits); this variant is normalized away by the \
     representation and cannot be compared lexically";

const UNSUPPORTED_SYNTAX: &str = "unsupported syntax element: the alias lexical grammar accepts only constructs that \
     rustdoc can emit for a generic type alias declaration";

/// Rule A + Rule B for a bound string (a single `TypeParamBound`).
pub(crate) fn enforce_closed_bound_grammar(
    bound_str: &str,
    generic_params: &[&str],
) -> Result<(), String> {
    let syn_bound: syn::TypeParamBound = syn::parse_str(bound_str)
        .map_err(|e| format!("invalid bound syntax '{bound_str}': {e}"))?;

    let mut visitor = AllowlistVisitor::new(generic_params);
    // Rustc permits a relaxed bound only directly on a type parameter of the
    // closest item: the ROOT position of a bound string qualifies, nested
    // positions (dyn objects, associated-item constraints, …) never do.
    visitor.relaxed_bound_allowed = true;
    visitor.visit_type_param_bound(&syn_bound);
    visitor.finish()?;

    let converted = convert_bound_with_dummy_context(&syn_bound, generic_params);
    let canonical = render_bound(&converted).ok_or_else(|| UNSUPPORTED_SYNTAX.to_owned())?;
    if tokens_match(&canonical, bound_str) {
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

// ---------------------------------------------------------------------------
// Rule B — syntax allowlist visitor
// ---------------------------------------------------------------------------

/// How lifetime USES outside a binder are judged.
#[derive(Clone, Copy)]
enum LifetimePolicy {
    /// Bound / where positions: only `'static` or binder-introduced names.
    ScopedOnly,
    /// Alias targets: source-declarable named lifetimes (the schema cannot
    /// declare lifetime parameters, so targets carry them lexically); `'_`,
    /// Rust's reserved lifetime names, and raw spellings (never
    /// rustdoc-observable) stay rejected.
    AnyNamed,
}

/// Whether a lifetime spelling is reserved where a source declaration must
/// introduce a user-defined lifetime. `syn` validates token syntax; this
/// closes the remaining rustc-reserved names while leaving keyword and Unicode
/// lifetime names available. Raw spellings are rejected before this check.
fn is_reserved_declared_lifetime_name(name: &str) -> bool {
    matches!(name, "_" | "self" | "Self" | "super" | "crate")
}

/// Standard-library types (structs / enums) that a trait-position path can
/// Whether a spelling is unavailable as a bare function-pointer parameter
/// name in the current Rust edition. Weak keywords remain valid identifiers in
/// this context (`raw`, `safe`, `union`, and `macro_rules`), so this must not
/// reuse the generic-declaration predicate.
fn is_bare_fn_parameter_reserved_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "gen"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    )
}

struct AllowlistVisitor<'params, 'names> {
    generic_params: &'params [&'names str],
    /// Lifetime names (without `'`) introduced by enclosing `for<>` binders.
    lifetime_scope: Vec<String>,
    lifetime_policy: LifetimePolicy,
    /// `?Sized` is valid only at a direct alias-parameter bound.
    relaxed_bound_allowed: bool,
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
        Self {
            generic_params,
            lifetime_scope: vec![],
            lifetime_policy,
            relaxed_bound_allowed: false,
            rejection: None,
        }
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

    fn check_const_expr_not_declared_type_param(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Path(path) => {
                if path.qself.is_none()
                    && path.path.leading_colon.is_none()
                    && path.path.segments.len() == 1
                {
                    if let Some(segment) = path.path.segments.first() {
                        let name = segment.ident.to_string();
                        if self.generic_params.contains(&name.as_str()) {
                            self.reject(&format!(
                                "const expression uses declared type parameter `{name}` as a \
                                 value (rustc E0423)"
                            ));
                        }
                    }
                }
            }
            syn::Expr::Block(block) => {
                if let [syn::Stmt::Expr(inner, None)] = block.block.stmts.as_slice() {
                    self.check_const_expr_not_declared_type_param(inner);
                }
            }
            syn::Expr::Unary(unary) => {
                self.check_const_expr_not_declared_type_param(&unary.expr);
            }
            _ => {}
        }
    }

    /// Rejects lifetime spellings that can never match rustdoc output: raw
    /// spellings (`'r#async` normalizes to `'async`) and non-NFC Unicode
    /// names (rustc normalizes identifiers to NFC). The normalized spelling
    /// is the one accepted representation of such a source declaration.
    fn check_raw_lifetime_spelling(&mut self, name: &str) -> bool {
        if let Some(stripped) = name.strip_prefix("r#") {
            self.reject(&format!(
                "raw lifetime `'{name}` never appears in rustdoc output (it normalizes to \
                 `'{stripped}`); use the normalized spelling"
            ));
            return true;
        }
        if !unicode_normalization::is_nfc(name) {
            self.reject(&format!(
                "lifetime `'{name}` is not NFC-normalized; rustdoc emits NFC identifiers"
            ));
            return true;
        }
        false
    }

    fn check_lifetime_use(&mut self, lifetime: &syn::Lifetime) {
        let name = lifetime.ident.to_string();
        if self.check_raw_lifetime_spelling(&name) {
            return;
        }
        match self.lifetime_policy {
            LifetimePolicy::ScopedOnly => {
                if name != "static" && !self.lifetime_scope.contains(&name) {
                    self.reject(&format!(
                        "lifetime `'{name}` is neither `'static` nor introduced by a visible \
                         `for<>` binder"
                    ));
                }
            }
            LifetimePolicy::AnyNamed => {
                // `syn` has already checked lifetime-token syntax.  These
                // are the remaining reserved forms that rustc does not allow
                // in lifetime declarations; rustdoc-normalized keyword names
                // such as `'async` remain accepted.
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
                    // Raw spellings are rejected as never rustdoc-observable.
                    let name = lifetime_param.lifetime.ident.to_string();
                    if !self.check_raw_lifetime_spelling(&name) {
                        if name == "static" || is_reserved_declared_lifetime_name(&name) {
                            self.reject(&format!(
                                "`'{name}` cannot be declared by a `for<>` binder"
                            ));
                        }
                        if self.lifetime_scope.contains(&name) {
                            self.reject(&format!(
                                "binder lifetime `'{name}` duplicates or shadows a lifetime \
                                 already in scope"
                            ));
                        }
                    }
                    self.lifetime_scope.push(name);
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
    /// primitives, paths rooted at a declared generic parameter, and exact
    /// std / core canonical paths of KNOWN standard-library non-trait types
    /// (rustc E0404, "expected trait, found struct") cannot. Bare short names
    /// stay open-world — the trait-path resolver checks local catalogue items
    /// first, so `T: Vec<u8>` may legitimately name a local trait.
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
        if let Some(known) = super::non_trait_paths::known_non_trait_type(path) {
            self.reject(&format!(
                "`{known}` is a standard-library type, not a trait, and cannot be used as a \
                 trait bound (rustc E0404)"
            ));
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
            // Rustc permits a relaxed bound only directly on a type
            // parameter of the closest item; nested positions (dyn
            // objects, associated-item constraints, impl-trait lists)
            // are rejected.
            if !self.relaxed_bound_allowed {
                self.reject("`?Sized` is only valid as a direct type-parameter bound");
            }
        }
        // Anything below this bound is a nested position.
        self.relaxed_bound_allowed = false;
        self.check_trait_path(&node.path);
        let added = self.enter_binder(node.lifetimes.as_ref());
        self.visit_path(&node.path);
        self.exit_binder(added);
    }

    fn visit_generic_argument(&mut self, node: &'ast syn::GenericArgument) {
        if let syn::GenericArgument::Const(expr)
        | syn::GenericArgument::AssocConst(syn::AssocConst { value: expr, .. }) = node
        {
            self.check_const_expr_not_declared_type_param(expr);
        }
        syn::visit::visit_generic_argument(self, node);
    }

    fn visit_type_array(&mut self, node: &'ast syn::TypeArray) {
        self.check_const_expr_not_declared_type_param(&node.len);
        syn::visit::visit_type_array(self, node);
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
            // Rustc allows `self` only in associated functions. Raw spellings
            // never reach rustdoc, and strict / reserved keywords cannot name
            // bare-function parameters. Weak keywords remain declarable in
            // this context, so use its dedicated keyword set. Delegate
            // identifier validity to syn so this admits the Unicode identifiers
            // Rust accepts instead of applying the ASCII-only generic-name rule.
            if let Some((name, _)) = &input.name {
                let name = name.to_string();
                // `syn::Ident` is not edition-aware (its keyword table omits
                // the Rust 2024 reserved keyword `gen`) and preserves non-NFC
                // Unicode spellings that rustc normalizes, so the keyword
                // list and NFC form are validated explicitly. Raw spellings
                // never appear in rustdoc output.
                if name != "_"
                    && (name.starts_with("r#")
                        || is_bare_fn_parameter_reserved_keyword(&name)
                        || syn::parse_str::<syn::Ident>(&name).is_err()
                        || !unicode_normalization::is_nfc(&name))
                {
                    self.reject(&format!(
                        "bare-function parameter name `{name}` is not valid in a function \
                         pointer declaration"
                    ));
                }
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
