//! Canonical source-spelling renderer for the converted rustdoc representation.
//!
//! This is one half of the closed acceptance grammar for the alias lexical
//! comparison (the other half is the syntax allowlist in `closed_grammar`).
//! The renderer prints a `rustdoc_types::Type` / `GenericBound` back to Rust
//! source text in exactly one spelling — the canonical form. The round-trip
//! gate accepts a catalogue string only when it token-equals the canonical
//! rendering of its own converted representation, so every spelling the
//! converter or rustdoc silently normalizes (turbofish, redundant parens,
//! trailing punctuation, implicit ABIs, placeholder names, …) fails the gate
//! with no per-spelling recognition code.
//!
//! Returning `None` means the representation contains a construct with no
//! canonical spelling (for example an unresolved marker); the gate treats
//! that as a rejection, keeping the grammar fail-closed.

use rustdoc_types::{
    Abi, AssocItemConstraintKind, DynTrait, FunctionPointer, GenericArg, GenericArgs, GenericBound,
    GenericParamDef, GenericParamDefKind, Term, TraitBoundModifier, Type,
};

pub(super) fn render_type(ty: &Type) -> Option<String> {
    match ty {
        Type::ResolvedPath(path) => render_path(path),
        Type::Generic(name) => Some(name.clone()),
        Type::Primitive(name) => {
            if name == "never" {
                Some("!".to_owned())
            } else {
                Some(name.clone())
            }
        }
        Type::Tuple(elements) => {
            let rendered: Option<Vec<String>> = elements.iter().map(render_type).collect();
            let rendered = rendered?;
            match rendered.as_slice() {
                [] => Some("()".to_owned()),
                [single] => Some(format!("({single},)")),
                _ => Some(format!("({})", rendered.join(", "))),
            }
        }
        Type::Slice(inner) => Some(format!("[{}]", render_type(inner)?)),
        Type::Array { type_, len } => Some(format!("[{}; {len}]", render_type(type_)?)),
        Type::BorrowedRef { lifetime, is_mutable, type_ } => {
            let lifetime_str = lifetime.as_ref().map(|lt| format!("{lt} ")).unwrap_or_default();
            let mut_str = if *is_mutable { "mut " } else { "" };
            Some(format!("&{lifetime_str}{mut_str}{}", render_pointee(type_)?))
        }
        Type::RawPointer { is_mutable, type_ } => {
            let kw = if *is_mutable { "mut" } else { "const" };
            Some(format!("*{kw} {}", render_pointee(type_)?))
        }
        Type::DynTrait(dyn_trait) => render_dyn_trait(dyn_trait),
        Type::FunctionPointer(fp) => render_function_pointer(fp),
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let assoc_args = match args.as_deref() {
                Some(a) => render_generic_args(a)?,
                None => String::new(),
            };
            match trait_ {
                Some(trait_path) => Some(format!(
                    "<{} as {}>::{name}{assoc_args}",
                    render_type(self_type)?,
                    render_path(trait_path)?
                )),
                None => Some(format!("{}::{name}{assoc_args}", render_type(self_type)?)),
            }
        }
        // No canonical spelling exists for these in the closed alias grammar:
        // `impl Trait` / `_` are rejected by the allowlist, `Pat` cannot occur,
        // and unresolved markers must never round-trip.
        Type::ImplTrait(_) | Type::Infer | Type::Pat { .. } => None,
    }
}

pub(super) fn render_bound(bound: &GenericBound) -> Option<String> {
    match bound {
        GenericBound::TraitBound { trait_, generic_params, modifier } => {
            let binder = render_binder(generic_params)?;
            let modifier_str = match modifier {
                TraitBoundModifier::None => "",
                TraitBoundModifier::Maybe => "?",
                TraitBoundModifier::MaybeConst => return None,
            };
            Some(format!("{binder}{modifier_str}{}", render_path(trait_)?))
        }
        GenericBound::Outlives(lifetime) => Some(lifetime.clone()),
        GenericBound::Use(_) => None,
    }
}

fn render_path(path: &rustdoc_types::Path) -> Option<String> {
    // Unresolved markers (`<unknown_type>`, `<generic_with_arguments>`, …) are
    // angle-bracket-wrapped and can never token-equal real source, so they are
    // rendered verbatim and fail the round-trip naturally.
    let args = match path.args.as_deref() {
        Some(a) => render_generic_args(a)?,
        None => String::new(),
    };
    Some(format!("{}{args}", path.path))
}

fn render_generic_args(args: &GenericArgs) -> Option<String> {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => {
            if args.is_empty() && constraints.is_empty() {
                // An empty argument list has no canonical spelling (`Tr<>` is
                // never emitted); the converter also never produces this form.
                return None;
            }
            let mut parts: Vec<String> = Vec::new();
            for arg in args {
                parts.push(render_generic_arg(arg)?);
            }
            for constraint in constraints {
                let constraint_args = match constraint.args.as_deref() {
                    Some(a) => render_generic_args(a)?,
                    None => String::new(),
                };
                let binding = match &constraint.binding {
                    AssocItemConstraintKind::Equality(term) => {
                        format!(" = {}", render_term(term)?)
                    }
                    AssocItemConstraintKind::Constraint(bounds) => {
                        let rendered: Option<Vec<String>> =
                            bounds.iter().map(render_bound).collect();
                        format!(": {}", rendered?.join(" + "))
                    }
                };
                parts.push(format!("{}{constraint_args}{binding}", constraint.name));
            }
            Some(format!("<{}>", parts.join(", ")))
        }
        GenericArgs::Parenthesized { inputs, output } => {
            let rendered: Option<Vec<String>> = inputs.iter().map(render_type).collect();
            let output_str = match output {
                Some(ty) => format!(" -> {}", render_type(ty)?),
                None => String::new(),
            };
            Some(format!("({}){output_str}", rendered?.join(", ")))
        }
        GenericArgs::ReturnTypeNotation => None,
    }
}

fn render_generic_arg(arg: &GenericArg) -> Option<String> {
    match arg {
        GenericArg::Lifetime(lifetime) => Some(lifetime.clone()),
        GenericArg::Type(ty) => render_type(ty),
        GenericArg::Const(constant) => Some(constant.expr.clone()),
        GenericArg::Infer => None,
    }
}

fn render_term(term: &Term) -> Option<String> {
    match term {
        Term::Type(ty) => render_type(ty),
        Term::Constant(constant) => Some(constant.expr.clone()),
    }
}

fn render_dyn_trait(dyn_trait: &DynTrait) -> Option<String> {
    if dyn_trait.traits.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for poly in &dyn_trait.traits {
        let binder = render_binder(&poly.generic_params)?;
        parts.push(format!("{binder}{}", render_path(&poly.trait_)?));
    }
    if let Some(lifetime) = &dyn_trait.lifetime {
        parts.push(lifetime.clone());
    }
    Some(format!("dyn {}", parts.join(" + ")))
}

fn render_function_pointer(fp: &FunctionPointer) -> Option<String> {
    let binder = render_binder(&fp.generic_params)?;
    let unsafe_str = if fp.header.is_unsafe { "unsafe " } else { "" };
    let abi_str = render_abi(&fp.header.abi)?;
    let mut inputs: Vec<String> = Vec::new();
    for (name, ty) in &fp.sig.inputs {
        // The representation stores `_` for an omitted parameter name; the
        // canonical spelling omits the name entirely.
        if name == "_" {
            inputs.push(render_type(ty)?);
        } else {
            inputs.push(format!("{name}: {}", render_type(ty)?));
        }
    }
    if fp.sig.is_c_variadic {
        inputs.push("...".to_owned());
    }
    let output_str = match &fp.sig.output {
        Some(ty) => format!(" -> {}", render_type(ty)?),
        None => String::new(),
    };
    Some(format!("{binder}{unsafe_str}{abi_str}fn({}){output_str}", inputs.join(", ")))
}

fn render_abi(abi: &Abi) -> Option<String> {
    let name = match abi {
        Abi::Rust => return Some(String::new()),
        Abi::C { unwind: false } => "\"C\"".to_owned(),
        Abi::C { unwind: true } => "\"C-unwind\"".to_owned(),
        Abi::Cdecl { unwind: false } => "\"cdecl\"".to_owned(),
        Abi::Cdecl { unwind: true } => "\"cdecl-unwind\"".to_owned(),
        Abi::Stdcall { unwind: false } => "\"stdcall\"".to_owned(),
        Abi::Stdcall { unwind: true } => "\"stdcall-unwind\"".to_owned(),
        Abi::Fastcall { unwind: false } => "\"fastcall\"".to_owned(),
        Abi::Fastcall { unwind: true } => "\"fastcall-unwind\"".to_owned(),
        Abi::Aapcs { unwind: false } => "\"aapcs\"".to_owned(),
        Abi::Aapcs { unwind: true } => "\"aapcs-unwind\"".to_owned(),
        Abi::Win64 { unwind: false } => "\"win64\"".to_owned(),
        Abi::Win64 { unwind: true } => "\"win64-unwind\"".to_owned(),
        Abi::SysV64 { unwind: false } => "\"sysv64\"".to_owned(),
        Abi::SysV64 { unwind: true } => "\"sysv64-unwind\"".to_owned(),
        Abi::System { unwind: false } => "\"system\"".to_owned(),
        Abi::System { unwind: true } => "\"system-unwind\"".to_owned(),
        // `Abi::Other` already carries rustdoc's quoted literal spelling.
        Abi::Other(other) => other.clone(),
    };
    Some(format!("extern {name} "))
}

fn render_binder(params: &[GenericParamDef]) -> Option<String> {
    if params.is_empty() {
        return Some(String::new());
    }
    let mut parts: Vec<String> = Vec::new();
    for param in params {
        match &param.kind {
            GenericParamDefKind::Lifetime { outlives } => {
                if outlives.is_empty() {
                    parts.push(param.name.clone());
                } else {
                    parts.push(format!("{}: {}", param.name, outlives.join(" + ")));
                }
            }
            // Higher-ranked type/const parameters have no canonical spelling
            // in the closed alias grammar.
            GenericParamDefKind::Type { .. } | GenericParamDefKind::Const { .. } => return None,
        }
    }
    Some(format!("for<{}> ", parts.join(", ")))
}

/// A `dyn` pointee with more than one bound requires parentheses behind `&` /
/// `*` (`&(dyn A + B)`); every other pointee renders bare.
fn render_pointee(ty: &Type) -> Option<String> {
    let rendered = render_type(ty)?;
    if let Type::DynTrait(dyn_trait) = ty {
        let bound_count = dyn_trait.traits.len() + usize::from(dyn_trait.lifetime.is_some());
        if bound_count > 1 {
            return Some(format!("({rendered})"));
        }
    }
    Some(rendered)
}
