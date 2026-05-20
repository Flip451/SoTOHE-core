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

use std::collections::HashMap;

use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    GenericParamDef, GenericParamDefKind, Id, Path, PolyTrait, TraitBoundModifier, Type,
};

/// Sentinel crate_id for unresolved identifiers (ADR 2 D10 open-world).
pub(crate) const UNRESOLVED_CRATE_ID: u32 = u32::MAX;

/// Well-known Rust primitives that map to `Type::Primitive`.
const PRIMITIVE_TYPES: &[&str] = &[
    "bool", "char", "str", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16",
    "u32", "u64", "u128", "usize", "never",
];

/// std prelude types that are auto-resolved as `Type::ResolvedPath` with an
/// `std` external crate reference (ADR 2 D11).
pub(crate) const STD_PRELUDE_TYPES: &[&str] = &[
    "Vec",
    "Option",
    "Result",
    "String",
    "Box",
    "Iterator",
    "Default",
    "Clone",
    "Copy",
    "Debug",
    "Display",
    "PartialEq",
    "Eq",
    "Hash",
    "Ord",
    "PartialOrd",
    "Send",
    "Sync",
    "Sized",
    "Unpin",
    "Drop",
    "AsRef",
    "AsMut",
    "Deref",
    "DerefMut",
    "From",
    "Into",
    "TryFrom",
    "TryInto",
    "IntoIterator",
    "DoubleEndedIterator",
    "ExactSizeIterator",
    "FnOnce",
    "FnMut",
    "Fn",
    "ToString",
    "ToOwned",
    "BorrowMut",
    "Borrow",
    "Pin",
    "PhantomData",
    "HashMap",
    "BTreeMap",
    "HashSet",
    "BTreeSet",
    "VecDeque",
    "LinkedList",
    "Arc",
    "Rc",
    "Mutex",
    "RwLock",
];

// ---------------------------------------------------------------------------
// Public parse function
// ---------------------------------------------------------------------------

/// Parses a `TypeRef` string and converts it to `rustdoc_types::Type`.
///
/// The caller provides:
/// - `type_ref_str`: the raw string (e.g. `"Result<Option<User>, DomainError>"`).
/// - `resolve_local`: a closure that looks up a short name declared in the current
///   catalogue and returns its `rustdoc_types::Id`, or `None` if not found.
/// - `std_crate_id`: the crate_id assigned to `"std"` in `external_crates`.
/// - `external_crate_ids`: a snapshot of known `crate_name → crate_id` mappings.
/// - `emit_external_crate`: a callback invoked when a new external crate name is
///   encountered; returns the new crate_id.
///
/// # Errors
///
/// Returns an error string if `syn` fails to parse `type_ref_str`.
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
    let syn_type: syn::Type = syn::parse_str(type_ref_str)
        .map_err(|e| format!("syn parse error for `{type_ref_str}`: {e}"))?;

    // `std_crate_id` is kept in the public signature for API stability (callers must
    // pass the registered std crate_id), but Path.id always uses UNRESOLVED_CRATE_ID
    // for external types since item ids are not available at A-codec time.
    let _ = std_crate_id;
    let mut ctx = ParseCtx { resolve_local, external_crate_ids, emit_external_crate };

    Ok(ctx.convert_type(&syn_type))
}

// ---------------------------------------------------------------------------
// Internal parse context
// ---------------------------------------------------------------------------

struct ParseCtx<'a, F, G> {
    resolve_local: &'a F,
    external_crate_ids: &'a HashMap<String, u32>,
    emit_external_crate: &'a mut G,
}

impl<'a, F, G> ParseCtx<'a, F, G>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    fn convert_type(&mut self, ty: &syn::Type) -> Type {
        match ty {
            syn::Type::Path(type_path) => self.convert_type_path(type_path),
            syn::Type::Tuple(type_tuple) => self.convert_tuple(type_tuple),
            syn::Type::Slice(type_slice) => self.convert_slice(type_slice),
            syn::Type::Array(type_array) => self.convert_array(type_array),
            syn::Type::Reference(type_ref) => self.convert_reference(type_ref),
            syn::Type::Ptr(type_ptr) => self.convert_ptr(type_ptr),
            syn::Type::Never(_) => Type::Primitive("never".to_string()),
            syn::Type::Infer(_) => Type::Infer,
            // `dyn Trait + Trait2` — encode each bound as a `PolyTrait` entry.
            syn::Type::TraitObject(trait_obj) => self.convert_dyn_trait(trait_obj),
            // `impl Trait + Trait2` — encode each bound as a `GenericBound`.
            syn::Type::ImplTrait(impl_trait) => self.convert_impl_trait(impl_trait),
            // `(T)` — parenthesized type; unwrap to the inner type.
            syn::Type::Paren(paren) => self.convert_type(&paren.elem),
            // `fn(A, B) -> C` — function pointer; encode with minimal signature.
            syn::Type::BareFn(bare_fn) => self.convert_bare_fn(bare_fn),
            _ => unresolved_type("<unknown_type>"),
        }
    }

    fn convert_type_path(&mut self, type_path: &syn::TypePath) -> Type {
        // Collect path segments.
        let segments: Vec<_> = type_path.path.segments.iter().collect();
        if segments.is_empty() {
            return unresolved_type("<empty_path>");
        }

        // Qualified path (`<T as Trait>::Assoc`) — encode as unresolved marker.
        if type_path.qself.is_some() {
            return unresolved_type("<qualified_path>");
        }

        // Multi-segment: first segment is a crate name prefix.
        if segments.len() > 1 {
            return self.convert_crate_prefixed_path(&segments);
        }

        // Single-segment. We already checked `segments.is_empty()` above.
        let Some(first_seg) = segments.first() else {
            return unresolved_type("<empty_path>");
        };
        let name = first_seg.ident.to_string();

        // 1. Rust primitive?
        if PRIMITIVE_TYPES.contains(&name.as_str()) {
            return Type::Primitive(name);
        }

        // 2. Unit type `()`?  (expressed as empty-path in some syn versions)
        if name == "()" {
            return Type::Tuple(vec![]);
        }

        // 3. Local catalogue declaration?
        //
        // Checked BEFORE the `Self` keyword and the std prelude allowlist so that a
        // catalogue type with the same spelling as a prelude item or the `Self` keyword
        // (highly unusual but technically possible) is resolved to the local item id
        // rather than silently mapped to a sentinel or std path.
        if let Some(local_id) = (self.resolve_local)(&name) {
            let generic_args = self.convert_generic_args(&first_seg.arguments);
            return Type::ResolvedPath(Path {
                path: name,
                id: local_id,
                args: generic_args.map(Box::new),
            });
        }

        // 4. `Self` keyword — represents the implementing type. Encoded as a
        //    sentinel `ResolvedPath` with `Id(0)` (root module placeholder).
        //    Callers that need to remap `Self` to a concrete type do so after
        //    codec time.
        if name == "Self" {
            return Type::ResolvedPath(Path { path: "Self".to_string(), id: Id(0), args: None });
        }

        // 5. std prelude allowlist?
        //
        // `Path.id` is an item id, not a crate id. For external types we do not have
        // the actual item id, so we use the unresolved marker sentinel (`UNRESOLVED_CRATE_ID`).
        // Use the canonical `std::module::TypeName` path so downstream consumers can
        // distinguish real std types from truly-unresolved identifiers.
        if STD_PRELUDE_TYPES.contains(&name.as_str()) {
            let generic_args = self.convert_generic_args(&first_seg.arguments);
            let canonical = std_canonical_path(&name);
            return Type::ResolvedPath(Path {
                path: canonical,
                id: Id(UNRESOLVED_CRATE_ID),
                args: generic_args.map(Box::new),
            });
        }

        // 6. Unresolved marker.
        let generic_args = self.convert_generic_args(&first_seg.arguments);
        Type::ResolvedPath(Path {
            path: name,
            id: Id(UNRESOLVED_CRATE_ID),
            args: generic_args.map(Box::new),
        })
    }

    fn convert_crate_prefixed_path(&mut self, segments: &[&syn::PathSegment]) -> Type {
        // Caller guarantees segments.len() > 1 (multi-segment path).
        let Some(first_seg) = segments.first() else {
            return unresolved_type("<empty_crate_path>");
        };
        let first_name = first_seg.ident.to_string();
        let Some(last_seg) = segments.last() else {
            return unresolved_type("<empty_crate_path>");
        };
        let full_path = segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");

        // `crate`, `self`, `super` are Rust path keywords, not external crate names.
        // For relative/self-referential paths, attempt to resolve the last segment
        // against the local catalogue; fall back to an unresolved marker without
        // registering a spurious `external_crates` entry.
        let is_path_keyword = matches!(first_name.as_str(), "crate" | "self" | "super");
        if is_path_keyword {
            let last_name = last_seg.ident.to_string();
            let generic_args = self.convert_generic_args(&last_seg.arguments);
            if let Some(local_id) = (self.resolve_local)(&last_name) {
                return Type::ResolvedPath(Path {
                    path: full_path,
                    id: local_id,
                    args: generic_args.map(Box::new),
                });
            }
            return Type::ResolvedPath(Path {
                path: full_path,
                id: Id(UNRESOLVED_CRATE_ID),
                args: generic_args.map(Box::new),
            });
        }

        // Regular multi-segment path: first segment is an external crate name.
        // Ensure the external crate is registered (so Crate::external_crates is populated),
        // but do not store the crate id in Path.id — Path.id is an item id.
        // Use UNRESOLVED_CRATE_ID as the item id sentinel for external types.
        if !self.external_crate_ids.contains_key(&first_name) {
            (self.emit_external_crate)(first_name);
        }

        let generic_args = self.convert_generic_args(&last_seg.arguments);

        Type::ResolvedPath(Path {
            path: full_path,
            id: Id(UNRESOLVED_CRATE_ID),
            args: generic_args.map(Box::new),
        })
    }

    fn convert_tuple(&mut self, type_tuple: &syn::TypeTuple) -> Type {
        let elems: Vec<Type> = type_tuple.elems.iter().map(|t| self.convert_type(t)).collect();
        Type::Tuple(elems)
    }

    fn convert_slice(&mut self, type_slice: &syn::TypeSlice) -> Type {
        let inner = self.convert_type(&type_slice.elem);
        Type::Slice(Box::new(inner))
    }

    fn convert_array(&mut self, type_array: &syn::TypeArray) -> Type {
        let inner = self.convert_type(&type_array.elem);
        let len_str = array_len_to_string(&type_array.len);
        Type::Array { type_: Box::new(inner), len: len_str }
    }

    fn convert_reference(&mut self, type_ref: &syn::TypeReference) -> Type {
        let inner = self.convert_type(&type_ref.elem);
        let is_mutable = type_ref.mutability.is_some();
        let lifetime = type_ref.lifetime.as_ref().map(|lt| format!("'{}", lt.ident));
        Type::BorrowedRef { lifetime, is_mutable, type_: Box::new(inner) }
    }

    fn convert_ptr(&mut self, type_ptr: &syn::TypePtr) -> Type {
        let inner = self.convert_type(&type_ptr.elem);
        let is_mutable = type_ptr.mutability.is_some();
        Type::RawPointer { is_mutable, type_: Box::new(inner) }
    }

    /// Resolves a `syn::Path` (from a trait bound) to a `rustdoc_types::Path`.
    ///
    /// Runs the same resolution logic as `convert_type_path` for the trait name:
    /// local catalogue check → std prelude allowlist → external crate registration.
    /// This ensures that `dyn Clone`, `impl Iterator<Item = T>`, and
    /// `dyn serde::Serialize` are resolved consistently with plain path types.
    fn resolve_trait_bound_path(&mut self, syn_path: &syn::Path) -> Path {
        let segments: Vec<_> = syn_path.segments.iter().collect();
        let full_path = segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");

        // Multi-segment: treat first as a crate/module prefix.
        if segments.len() > 1 {
            let Some(first_seg_ref) = segments.first() else {
                return Path { path: full_path, id: Id(UNRESOLVED_CRATE_ID), args: None };
            };
            let first_name = first_seg_ref.ident.to_string();
            let Some(last_seg) = segments.last() else {
                return Path { path: full_path, id: Id(UNRESOLVED_CRATE_ID), args: None };
            };
            let args = self.convert_generic_args(&last_seg.arguments);

            let is_path_keyword = matches!(first_name.as_str(), "crate" | "self" | "super");
            let id = if is_path_keyword {
                let last_name = last_seg.ident.to_string();
                if let Some(local_id) = (self.resolve_local)(&last_name) {
                    local_id
                } else {
                    Id(UNRESOLVED_CRATE_ID)
                }
            } else {
                // Register external crate if new.
                if !self.external_crate_ids.contains_key(&first_name) {
                    (self.emit_external_crate)(first_name);
                }
                Id(UNRESOLVED_CRATE_ID)
            };
            return Path { path: full_path, id, args: args.map(Box::new) };
        }

        // Single-segment: run the same resolution chain as `convert_type_path`.
        let Some(first_seg) = segments.first() else {
            return Path { path: full_path, id: Id(UNRESOLVED_CRATE_ID), args: None };
        };
        let name = first_seg.ident.to_string();
        let args = self.convert_generic_args(&first_seg.arguments);

        // Local catalogue?
        if let Some(local_id) = (self.resolve_local)(&name) {
            return Path { path: name, id: local_id, args: args.map(Box::new) };
        }

        // std prelude?
        if STD_PRELUDE_TYPES.contains(&name.as_str()) {
            let canonical = std_canonical_path(&name);
            return Path { path: canonical, id: Id(UNRESOLVED_CRATE_ID), args: args.map(Box::new) };
        }

        // Unresolved marker.
        Path { path: name, id: Id(UNRESOLVED_CRATE_ID), args: args.map(Box::new) }
    }

    /// Converts a `dyn Trait + Trait2` type object into `Type::DynTrait`.
    ///
    /// Each `TypeParamBound::Trait` in the bound list becomes a `PolyTrait` entry.
    /// Trait paths are resolved via `resolve_trait_bound_path` (same logic as
    /// `convert_type_path`), so local catalogue types and std prelude entries are
    /// handled consistently.
    /// Lifetime bounds are recorded in `DynTrait::lifetime` (first one wins; rare in
    /// catalogue `TypeRef` strings).
    fn convert_dyn_trait(&mut self, trait_obj: &syn::TypeTraitObject) -> Type {
        let mut traits = vec![];
        let mut lifetime: Option<String> = None;
        for bound in &trait_obj.bounds {
            match bound {
                syn::TypeParamBound::Trait(tb) => {
                    let trait_path = self.resolve_trait_bound_path(&tb.path);
                    traits.push(PolyTrait { trait_: trait_path, generic_params: vec![] });
                }
                syn::TypeParamBound::Lifetime(lt) => {
                    if lifetime.is_none() {
                        lifetime = Some(format!("'{}", lt.ident));
                    }
                }
                _ => {}
            }
        }
        Type::DynTrait(DynTrait { traits, lifetime })
    }

    /// Converts an `impl Trait + Trait2` type into `Type::ImplTrait`.
    ///
    /// Each `TypeParamBound::Trait` becomes a `GenericBound::TraitBound`, with the
    /// trait path resolved via `resolve_trait_bound_path`.
    fn convert_impl_trait(&mut self, impl_trait: &syn::TypeImplTrait) -> Type {
        let mut bounds = vec![];
        for bound in &impl_trait.bounds {
            match bound {
                syn::TypeParamBound::Trait(tb) => {
                    let trait_path = self.resolve_trait_bound_path(&tb.path);
                    bounds.push(GenericBound::TraitBound {
                        trait_: trait_path,
                        generic_params: vec![],
                        modifier: TraitBoundModifier::None,
                    });
                }
                syn::TypeParamBound::Lifetime(lt) => {
                    bounds.push(GenericBound::Outlives(format!("'{}", lt.ident)));
                }
                _ => {}
            }
        }
        Type::ImplTrait(bounds)
    }

    /// Converts a bare function pointer `fn(A, B) -> C` into `Type::FunctionPointer`.
    ///
    /// The parameter types are encoded via `convert_type`; the return type is
    /// treated as `None` for `-> ()` or encoded inline. The ABI is encoded from
    /// the `extern "..."` annotation, and HRTB lifetime binders (`for<'a>`) are
    /// encoded into `generic_params`.
    fn convert_bare_fn(&mut self, bare_fn: &syn::TypeBareFn) -> Type {
        let inputs: Vec<(String, Type)> = bare_fn
            .inputs
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                let name = arg
                    .name
                    .as_ref()
                    .map(|(n, _)| n.to_string())
                    .unwrap_or_else(|| format!("_arg{i}"));
                let ty = self.convert_type(&arg.ty);
                (name, ty)
            })
            .collect();
        let output = match &bare_fn.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => {
                let converted = self.convert_type(ty);
                // Treat explicit `-> ()` as no output (consistent with encode_method_items).
                if matches!(&converted, Type::Tuple(v) if v.is_empty()) {
                    None
                } else {
                    Some(converted)
                }
            }
        };
        let is_unsafe = bare_fn.unsafety.is_some();
        let abi = syn_abi_to_rustdoc_abi(bare_fn.abi.as_ref());
        let generic_params = bound_lifetimes_to_generic_params(bare_fn.lifetimes.as_ref());
        Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
            sig: rustdoc_types::FunctionSignature {
                inputs,
                output,
                is_c_variadic: bare_fn.variadic.is_some(),
            },
            generic_params,
            header: rustdoc_types::FunctionHeader {
                is_async: false,
                is_const: false,
                is_unsafe,
                abi,
            },
        }))
    }

    fn convert_generic_args(&mut self, args: &syn::PathArguments) -> Option<GenericArgs> {
        match args {
            syn::PathArguments::None => None,
            syn::PathArguments::AngleBracketed(ab) => {
                // Split angle-bracketed args into type/lifetime/const args and
                // associated-type constraints (`Item = T`, `Output: Trait`).
                let mut generic_args: Vec<GenericArg> = vec![];
                let mut constraints: Vec<AssocItemConstraint> = vec![];

                for arg in &ab.args {
                    match arg {
                        syn::GenericArgument::AssocType(assoc) => {
                            // `Iterator<Item = T>` or `Iterator<Item<'a> = T>` — encode as
                            // an AssocItemConstraint. The optional `assoc.generics` angle
                            // brackets (e.g. `Item<'a>`) are encoded into the `args` field.
                            let ty = self.convert_type(&assoc.ty);
                            let args = angle_bracketed_to_generic_args(self, &assoc.generics);
                            constraints.push(AssocItemConstraint {
                                name: assoc.ident.to_string(),
                                args: args.map(Box::new),
                                binding: AssocItemConstraintKind::Equality(
                                    rustdoc_types::Term::Type(ty),
                                ),
                            });
                        }
                        syn::GenericArgument::AssocConst(assoc_const) => {
                            // `Trait<LENGTH = 32>` or `Trait<LEN<'a> = 32>` — encode the
                            // value as a const expression. Use `expr_to_token_string` (which
                            // handles binary/unary/paren/cast forms) so that `Trait<N = A + 1>`
                            // or `Trait<LEN = N as usize>` preserve their actual value.
                            let expr_str = expr_to_token_string(&assoc_const.value);
                            let args = angle_bracketed_to_generic_args(self, &assoc_const.generics);
                            constraints.push(AssocItemConstraint {
                                name: assoc_const.ident.to_string(),
                                args: args.map(Box::new),
                                binding: AssocItemConstraintKind::Equality(
                                    rustdoc_types::Term::Constant(rustdoc_types::Constant {
                                        expr: expr_str.clone(),
                                        value: Some(expr_str),
                                        is_literal: matches!(&assoc_const.value, syn::Expr::Lit(_)),
                                    }),
                                ),
                            });
                        }
                        syn::GenericArgument::Constraint(constraint) => {
                            // `Iterator<Item: Clone>` — encode as a Constraint binding.
                            // Convert each TypeParamBound to a GenericBound.
                            let bounds: Vec<GenericBound> = constraint
                                .bounds
                                .iter()
                                .filter_map(|b| match b {
                                    syn::TypeParamBound::Trait(tb) => {
                                        let trait_path = self.resolve_trait_bound_path(&tb.path);
                                        Some(GenericBound::TraitBound {
                                            trait_: trait_path,
                                            generic_params: vec![],
                                            modifier: TraitBoundModifier::None,
                                        })
                                    }
                                    syn::TypeParamBound::Lifetime(lt) => {
                                        Some(GenericBound::Outlives(format!("'{}", lt.ident)))
                                    }
                                    _ => None,
                                })
                                .collect();
                            constraints.push(AssocItemConstraint {
                                name: constraint.ident.to_string(),
                                args: None,
                                binding: AssocItemConstraintKind::Constraint(bounds),
                            });
                        }
                        _ => {
                            if let Some(ga) = self.convert_generic_arg(arg) {
                                generic_args.push(ga);
                            }
                        }
                    }
                }

                if generic_args.is_empty() && constraints.is_empty() {
                    None
                } else {
                    Some(GenericArgs::AngleBracketed { args: generic_args, constraints })
                }
            }
            syn::PathArguments::Parenthesized(p) => {
                let inputs: Vec<Type> = p.inputs.iter().map(|t| self.convert_type(t)).collect();
                let output = match &p.output {
                    syn::ReturnType::Default => None,
                    syn::ReturnType::Type(_, ty) => Some(self.convert_type(ty)),
                };
                Some(GenericArgs::Parenthesized { inputs, output })
            }
        }
    }

    fn convert_generic_arg(&mut self, arg: &syn::GenericArgument) -> Option<GenericArg> {
        match arg {
            syn::GenericArgument::Type(ty) => Some(GenericArg::Type(self.convert_type(ty))),
            syn::GenericArgument::Lifetime(lt) => {
                Some(GenericArg::Lifetime(format!("'{}", lt.ident)))
            }
            // `Const` args (e.g. `ArrayVec<u8, 32>`): encode as `GenericArg::Const` using
            // the stringified expression. We cannot evaluate the expression, but we preserve
            // the textual form for downstream consumers.
            syn::GenericArgument::Const(expr) => {
                let expr_str = syn_expr_to_string(expr);
                Some(GenericArg::Const(rustdoc_types::Constant {
                    expr: expr_str,
                    value: None,
                    is_literal: matches!(expr, syn::Expr::Lit(_)),
                }))
            }
            // `AssocType`, `AssocConst`, and `Constraint` are handled upstream in
            // `convert_generic_args` as `AssocItemConstraint` entries.
            syn::GenericArgument::AssocType(_)
            | syn::GenericArgument::AssocConst(_)
            | syn::GenericArgument::Constraint(_) => None,
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// ParseCtx-local helpers (free functions that take a mutable ParseCtx reference)
// ---------------------------------------------------------------------------

/// Converts `syn::AngleBracketedGenericArguments` to `Option<GenericArgs>`.
///
/// Returns `None` when the angle brackets are absent (via the `Option` wrapper
/// used in `syn::AssocType::generics` / `syn::AssocConst::generics`) or empty.
fn angle_bracketed_to_generic_args<F, G>(
    ctx: &mut ParseCtx<'_, F, G>,
    generics: &Option<syn::AngleBracketedGenericArguments>,
) -> Option<GenericArgs>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    let ab = generics.as_ref()?;
    let mut generic_args: Vec<GenericArg> = vec![];
    let mut constraints: Vec<AssocItemConstraint> = vec![];
    for arg in &ab.args {
        match arg {
            syn::GenericArgument::AssocType(assoc) => {
                let ty = ctx.convert_type(&assoc.ty);
                let inner_args = angle_bracketed_to_generic_args(ctx, &assoc.generics);
                constraints.push(AssocItemConstraint {
                    name: assoc.ident.to_string(),
                    args: inner_args.map(Box::new),
                    binding: AssocItemConstraintKind::Equality(rustdoc_types::Term::Type(ty)),
                });
            }
            syn::GenericArgument::Constraint(c) => {
                let bounds: Vec<GenericBound> = c
                    .bounds
                    .iter()
                    .filter_map(|b| match b {
                        syn::TypeParamBound::Trait(tb) => {
                            let trait_path = ctx.resolve_trait_bound_path(&tb.path);
                            Some(GenericBound::TraitBound {
                                trait_: trait_path,
                                generic_params: vec![],
                                modifier: TraitBoundModifier::None,
                            })
                        }
                        syn::TypeParamBound::Lifetime(lt) => {
                            Some(GenericBound::Outlives(format!("'{}", lt.ident)))
                        }
                        _ => None,
                    })
                    .collect();
                constraints.push(AssocItemConstraint {
                    name: c.ident.to_string(),
                    args: None,
                    binding: AssocItemConstraintKind::Constraint(bounds),
                });
            }
            _ => {
                if let Some(ga) = ctx.convert_generic_arg(arg) {
                    generic_args.push(ga);
                }
            }
        }
    }
    if generic_args.is_empty() && constraints.is_empty() {
        None
    } else {
        Some(GenericArgs::AngleBracketed { args: generic_args, constraints })
    }
}

/// Converts a `syn::Abi` to the corresponding `rustdoc_types::Abi`.
///
/// `None` (no `extern` keyword) → `Abi::Rust`.
/// `extern` without a name string → `Abi::C { unwind: false }` (C is the
/// implicit ABI for bare `extern fn`).
/// Known ABI name strings are mapped to their specific variants; any other
/// string falls through to `Abi::Other(name)`.
fn syn_abi_to_rustdoc_abi(abi: Option<&syn::Abi>) -> rustdoc_types::Abi {
    let Some(abi) = abi else {
        return rustdoc_types::Abi::Rust;
    };
    let name = match &abi.name {
        Some(lit) => lit.value(),
        // bare `extern` with no explicit string → defaults to C ABI
        None => return rustdoc_types::Abi::C { unwind: false },
    };
    match name.as_str() {
        "C" => rustdoc_types::Abi::C { unwind: false },
        "C-unwind" => rustdoc_types::Abi::C { unwind: true },
        "cdecl" => rustdoc_types::Abi::Cdecl { unwind: false },
        "cdecl-unwind" => rustdoc_types::Abi::Cdecl { unwind: true },
        "stdcall" => rustdoc_types::Abi::Stdcall { unwind: false },
        "stdcall-unwind" => rustdoc_types::Abi::Stdcall { unwind: true },
        "fastcall" => rustdoc_types::Abi::Fastcall { unwind: false },
        "fastcall-unwind" => rustdoc_types::Abi::Fastcall { unwind: true },
        "aapcs" => rustdoc_types::Abi::Aapcs { unwind: false },
        "aapcs-unwind" => rustdoc_types::Abi::Aapcs { unwind: true },
        "win64" => rustdoc_types::Abi::Win64 { unwind: false },
        "win64-unwind" => rustdoc_types::Abi::Win64 { unwind: true },
        "sysv64" => rustdoc_types::Abi::SysV64 { unwind: false },
        "sysv64-unwind" => rustdoc_types::Abi::SysV64 { unwind: true },
        "system" => rustdoc_types::Abi::System { unwind: false },
        "system-unwind" => rustdoc_types::Abi::System { unwind: true },
        other => rustdoc_types::Abi::Other(other.to_string()),
    }
}

/// Converts `syn::BoundLifetimes` (HRTB `for<'a, 'b>`) to a list of
/// `rustdoc_types::GenericParamDef` with `kind: Lifetime { outlives: [] }`.
///
/// Returns an empty `Vec` when `lifetimes` is `None`.
fn bound_lifetimes_to_generic_params(
    lifetimes: Option<&syn::BoundLifetimes>,
) -> Vec<GenericParamDef> {
    let Some(bl) = lifetimes else {
        return vec![];
    };
    bl.lifetimes
        .iter()
        .filter_map(|param| {
            if let syn::GenericParam::Lifetime(lt_param) = param {
                // `syn::Lifetime.ident` omits the leading apostrophe; rustdoc stores
                // lifetime strings WITH the apostrophe (e.g. `"'a"`, `"'_"`).
                // Re-prepend `'` so A-codec GenericParamDef names match C-side names.
                let outlives: Vec<String> =
                    lt_param.bounds.iter().map(|lt| format!("'{}", lt.ident)).collect();
                Some(GenericParamDef {
                    name: format!("'{}", lt_param.lifetime.ident),
                    kind: GenericParamDefKind::Lifetime { outlives },
                })
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds an "unresolved marker" `rustdoc_types::Type`.
///
/// Uses sentinel crate_id `u32::MAX` to mark identifiers that could not be
/// resolved at A-codec time (open-world). Closed-world validation in Phase 1
/// will reject any markers that remain after Delete processing.
#[must_use]
pub(crate) fn unresolved_type(name: &str) -> Type {
    Type::ResolvedPath(Path { path: name.to_string(), id: Id(UNRESOLVED_CRATE_ID), args: None })
}

/// Returns the canonical `std::module::TypeName` path for a short name that
/// appears in `STD_PRELUDE_TYPES`.
///
/// Using full canonical paths lets downstream consumers distinguish real
/// standard-library types (which have a known module hierarchy) from truly
/// unresolved identifiers that also carry `UNRESOLVED_CRATE_ID`.
#[must_use]
pub(crate) fn std_canonical_path(short_name: &str) -> String {
    match short_name {
        "Vec" => "std::vec::Vec",
        "Option" => "std::option::Option",
        "Result" => "std::result::Result",
        "String" => "std::string::String",
        "Box" => "std::boxed::Box",
        "Iterator" => "std::iter::Iterator",
        "Default" => "std::default::Default",
        "Clone" => "std::clone::Clone",
        "Copy" => "std::marker::Copy",
        "Debug" => "std::fmt::Debug",
        "Display" => "std::fmt::Display",
        "PartialEq" => "std::cmp::PartialEq",
        "Eq" => "std::cmp::Eq",
        "Hash" => "std::hash::Hash",
        "Ord" => "std::cmp::Ord",
        "PartialOrd" => "std::cmp::PartialOrd",
        "Send" => "std::marker::Send",
        "Sync" => "std::marker::Sync",
        "Sized" => "std::marker::Sized",
        "Unpin" => "std::marker::Unpin",
        "Drop" => "std::ops::Drop",
        "AsRef" => "std::convert::AsRef",
        "AsMut" => "std::convert::AsMut",
        "Deref" => "std::ops::Deref",
        "DerefMut" => "std::ops::DerefMut",
        "From" => "std::convert::From",
        "Into" => "std::convert::Into",
        "TryFrom" => "std::convert::TryFrom",
        "TryInto" => "std::convert::TryInto",
        "IntoIterator" => "std::iter::IntoIterator",
        "DoubleEndedIterator" => "std::iter::DoubleEndedIterator",
        "ExactSizeIterator" => "std::iter::ExactSizeIterator",
        "FnOnce" => "std::ops::FnOnce",
        "FnMut" => "std::ops::FnMut",
        "Fn" => "std::ops::Fn",
        "ToString" => "std::string::ToString",
        "ToOwned" => "std::borrow::ToOwned",
        "BorrowMut" => "std::borrow::BorrowMut",
        "Borrow" => "std::borrow::Borrow",
        "Pin" => "std::pin::Pin",
        "PhantomData" => "std::marker::PhantomData",
        "HashMap" => "std::collections::HashMap",
        "BTreeMap" => "std::collections::BTreeMap",
        "HashSet" => "std::collections::HashSet",
        "BTreeSet" => "std::collections::BTreeSet",
        "VecDeque" => "std::collections::VecDeque",
        "LinkedList" => "std::collections::LinkedList",
        "Arc" => "std::sync::Arc",
        "Rc" => "std::rc::Rc",
        "Mutex" => "std::sync::Mutex",
        "RwLock" => "std::sync::RwLock",
        // std::error
        "Error" => "std::error::Error",
        // std::io
        "Read" => "std::io::Read",
        "Write" => "std::io::Write",
        "Seek" => "std::io::Seek",
        "BufRead" => "std::io::BufRead",
        // std::fmt
        "Formatter" => "std::fmt::Formatter",
        // std::ops (arithmetic, bitwise, assignment, index)
        "Add" => "std::ops::Add",
        "Sub" => "std::ops::Sub",
        "Mul" => "std::ops::Mul",
        "Div" => "std::ops::Div",
        "Rem" => "std::ops::Rem",
        "Neg" => "std::ops::Neg",
        "Not" => "std::ops::Not",
        "BitAnd" => "std::ops::BitAnd",
        "BitOr" => "std::ops::BitOr",
        "BitXor" => "std::ops::BitXor",
        "Shl" => "std::ops::Shl",
        "Shr" => "std::ops::Shr",
        "Index" => "std::ops::Index",
        "IndexMut" => "std::ops::IndexMut",
        "AddAssign" => "std::ops::AddAssign",
        "SubAssign" => "std::ops::SubAssign",
        "MulAssign" => "std::ops::MulAssign",
        "DivAssign" => "std::ops::DivAssign",
        "RemAssign" => "std::ops::RemAssign",
        "BitAndAssign" => "std::ops::BitAndAssign",
        "BitOrAssign" => "std::ops::BitOrAssign",
        "BitXorAssign" => "std::ops::BitXorAssign",
        "ShlAssign" => "std::ops::ShlAssign",
        "ShrAssign" => "std::ops::ShrAssign",
        // std::iter (additional)
        "FromIterator" => "std::iter::FromIterator",
        "Extend" => "std::iter::Extend",
        "Sum" => "std::iter::Sum",
        "Product" => "std::iter::Product",
        // std::str
        "FromStr" => "std::str::FromStr",
        // std::hash
        "Hasher" => "std::hash::Hasher",
        "BuildHasher" => "std::hash::BuildHasher",
        // Fall back to `std::{name}` for any unknown entry.
        other => return format!("std::{other}"),
    }
    .to_string()
}

/// Maps a well-known `core` trait short name to its canonical rustdoc path.
///
/// `core` re-exports most of the same traits as `std`, but under `core::*`
/// module paths.  When a catalogue entry declares `origin_crate = "core"`,
/// the rustdoc JSON emitted for C-side impls will use `core::convert::From`
/// rather than `std::convert::From`.  This function mirrors `std_canonical_path`
/// but produces `core::*` paths so the S-side codec generates identity keys
/// that match the C-side rustdoc output.
///
/// Falls back to `"core::{name}"` for any trait not in the lookup table.
pub(crate) fn core_canonical_path(short_name: &str) -> String {
    match short_name {
        // core::convert
        "From" => "core::convert::From",
        "Into" => "core::convert::Into",
        "TryFrom" => "core::convert::TryFrom",
        "TryInto" => "core::convert::TryInto",
        "AsRef" => "core::convert::AsRef",
        "AsMut" => "core::convert::AsMut",
        // core::clone
        "Clone" => "core::clone::Clone",
        // core::marker
        "Copy" => "core::marker::Copy",
        "Send" => "core::marker::Send",
        "Sync" => "core::marker::Sync",
        "Sized" => "core::marker::Sized",
        "Unpin" => "core::marker::Unpin",
        "PhantomData" => "core::marker::PhantomData",
        // core::fmt
        "Debug" => "core::fmt::Debug",
        "Display" => "core::fmt::Display",
        "Formatter" => "core::fmt::Formatter",
        // core::cmp
        "PartialEq" => "core::cmp::PartialEq",
        "Eq" => "core::cmp::Eq",
        "Ord" => "core::cmp::Ord",
        "PartialOrd" => "core::cmp::PartialOrd",
        // core::hash
        "Hash" => "core::hash::Hash",
        "Hasher" => "core::hash::Hasher",
        "BuildHasher" => "core::hash::BuildHasher",
        // core::default
        "Default" => "core::default::Default",
        // core::iter
        "Iterator" => "core::iter::Iterator",
        "IntoIterator" => "core::iter::IntoIterator",
        "DoubleEndedIterator" => "core::iter::DoubleEndedIterator",
        "ExactSizeIterator" => "core::iter::ExactSizeIterator",
        "FromIterator" => "core::iter::FromIterator",
        "Extend" => "core::iter::Extend",
        "Sum" => "core::iter::Sum",
        "Product" => "core::iter::Product",
        // core::ops
        "Drop" => "core::ops::Drop",
        "Deref" => "core::ops::Deref",
        "DerefMut" => "core::ops::DerefMut",
        "FnOnce" => "core::ops::FnOnce",
        "FnMut" => "core::ops::FnMut",
        "Fn" => "core::ops::Fn",
        "Add" => "core::ops::Add",
        "Sub" => "core::ops::Sub",
        "Mul" => "core::ops::Mul",
        "Div" => "core::ops::Div",
        "Rem" => "core::ops::Rem",
        "Neg" => "core::ops::Neg",
        "Not" => "core::ops::Not",
        "BitAnd" => "core::ops::BitAnd",
        "BitOr" => "core::ops::BitOr",
        "BitXor" => "core::ops::BitXor",
        "Shl" => "core::ops::Shl",
        "Shr" => "core::ops::Shr",
        "Index" => "core::ops::Index",
        "IndexMut" => "core::ops::IndexMut",
        "AddAssign" => "core::ops::AddAssign",
        "SubAssign" => "core::ops::SubAssign",
        "MulAssign" => "core::ops::MulAssign",
        "DivAssign" => "core::ops::DivAssign",
        "RemAssign" => "core::ops::RemAssign",
        "BitAndAssign" => "core::ops::BitAndAssign",
        "BitOrAssign" => "core::ops::BitOrAssign",
        "BitXorAssign" => "core::ops::BitXorAssign",
        "ShlAssign" => "core::ops::ShlAssign",
        "ShrAssign" => "core::ops::ShrAssign",
        // core::str — rustdoc emits the full re-export path including the private
        // `traits` submodule: `core::str::traits::FromStr`.
        "FromStr" => "core::str::traits::FromStr",
        // core::borrow
        "Borrow" => "core::borrow::Borrow",
        "BorrowMut" => "core::borrow::BorrowMut",
        // core::pin
        "Pin" => "core::pin::Pin",
        // core::error (stable since Rust 1.81)
        "Error" => "core::error::Error",
        // Fall back to `core::{name}` for any unknown entry.
        other => return format!("core::{other}"),
    }
    .to_string()
}

/// Converts a `syn::Expr` to a textual representation.
///
/// Best-effort: literal integers and paths are rendered verbatim; other forms
/// fall back to `"<const_expr>"`.
#[must_use]
pub(crate) fn syn_expr_to_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Lit(lit_expr) => match &lit_expr.lit {
            syn::Lit::Int(i) => i.base10_digits().to_string(),
            syn::Lit::Str(s) => s.value(),
            syn::Lit::Bool(b) => b.value().to_string(),
            _ => "<const_expr>".to_string(),
        },
        syn::Expr::Path(path_expr) => path_expr
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        _ => "<const_expr>".to_string(),
    }
}

/// Converts a `syn::Expr` const array length to a string representation.
///
/// Preserves integer literals verbatim, named constants, and binary/unary
/// arithmetic expressions. Falls back to `"<const_len>"` only for forms that
/// cannot be represented as a simple token string.
#[must_use]
pub(crate) fn array_len_to_string(expr: &syn::Expr) -> String {
    expr_to_token_string(expr)
}

/// Renders a `syn::Expr` as a token string suitable for embedding in a type
/// description.
///
/// Handles literals, path constants, binary ops, unary ops, parenthesized
/// sub-expressions, and casts. Falls back to `"<const_expr>"` for forms that
/// are too complex to render without `quote!`.
fn expr_to_token_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Lit(lit_expr) => match &lit_expr.lit {
            syn::Lit::Int(i) => i.base10_digits().to_string(),
            syn::Lit::Str(s) => format!("{:?}", s.value()),
            syn::Lit::Bool(b) => b.value().to_string(),
            _ => "<const_expr>".to_string(),
        },
        syn::Expr::Path(path_expr) => path_expr
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        syn::Expr::Binary(bin_expr) => {
            let left = expr_to_token_string(&bin_expr.left);
            let right = expr_to_token_string(&bin_expr.right);
            let op = match &bin_expr.op {
                syn::BinOp::Add(_) => "+",
                syn::BinOp::Sub(_) => "-",
                syn::BinOp::Mul(_) => "*",
                syn::BinOp::Div(_) => "/",
                syn::BinOp::Rem(_) => "%",
                syn::BinOp::BitAnd(_) => "&",
                syn::BinOp::BitOr(_) => "|",
                syn::BinOp::BitXor(_) => "^",
                syn::BinOp::Shl(_) => "<<",
                syn::BinOp::Shr(_) => ">>",
                _ => "<op>",
            };
            format!("{left} {op} {right}")
        }
        syn::Expr::Unary(unary_expr) => {
            let inner = expr_to_token_string(&unary_expr.expr);
            let op = match &unary_expr.op {
                syn::UnOp::Neg(_) => "-",
                syn::UnOp::Not(_) => "!",
                _ => "<unary>",
            };
            format!("{op}{inner}")
        }
        syn::Expr::Paren(paren_expr) => {
            format!("({})", expr_to_token_string(&paren_expr.expr))
        }
        syn::Expr::Cast(cast_expr) => {
            // `N as usize` — preserve both the expression and the target type.
            let inner = expr_to_token_string(&cast_expr.expr);
            let target_ty = syn_type_to_string(&cast_expr.ty);
            format!("{inner} as {target_ty}")
        }
        _ => "<const_expr>".to_string(),
    }
}

/// Renders a `syn::Type` as a short string for use in cast expressions.
///
/// Only handles the common cases (primitives, simple paths); falls back to `_`
/// for complex forms.
fn syn_type_to_string(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(tp) if tp.qself.is_none() => {
            tp.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
        }
        syn::Type::Never(_) => "!".to_string(),
        _ => "_".to_string(),
    }
}

/// Parses a bound string (e.g. `"'static"`, `"Send"`, `"?Sized"`,
/// `"for<'a> Fn(&'a str)"`) into a `rustdoc_types::GenericBound`.
///
/// Unlike `parse_type_ref`, which uses `syn::parse_str::<syn::Type>()` and
/// rejects `?Trait`, lifetime bounds, and HRTB bounds, this function uses
/// `syn::parse_str::<syn::TypeParamBound>()` — the same parser that
/// `catalogue_document_codec`'s `validate_bound_str` uses — so the set of
/// accepted strings is identical between decode and encode.
///
/// Conversion rules:
/// - `'lifetime` → `GenericBound::Outlives("lifetime")`.
/// - `?Trait` → `GenericBound::TraitBound { modifier: Maybe, generic_params: [], ... }`.
/// - `for<'a> Trait<'a>` → `GenericBound::TraitBound { generic_params: [Lifetime('a)], ... }`.
/// - `Trait` / `Trait<T>` → `GenericBound::TraitBound { modifier: None, generic_params: [], ... }`.
///
/// # Errors
///
/// Returns `Err(String)` if `syn` cannot parse `bound_str` as a
/// `TypeParamBound`, or if the parsed bound is a form that cannot be
/// represented (e.g. `Verbatim` tokens from a proc-macro expansion).
pub(crate) fn parse_generic_bound<F, G>(
    bound_str: &str,
    resolve_local: &F,
    std_crate_id: u32,
    external_crate_ids: &HashMap<String, u32>,
    emit_external_crate: &mut G,
) -> Result<GenericBound, String>
where
    F: Fn(&str) -> Option<Id>,
    G: FnMut(String) -> u32,
{
    let syn_bound: syn::TypeParamBound =
        syn::parse_str(bound_str).map_err(|e| format!("syn parse error for `{bound_str}`: {e}"))?;

    let _ = std_crate_id; // kept for API symmetry with parse_type_ref
    let mut ctx = ParseCtx { resolve_local, external_crate_ids, emit_external_crate };

    match syn_bound {
        // `syn::Lifetime.ident` is the identifier part WITHOUT the leading apostrophe
        // (e.g. `'static` → `ident = "static"`).  `rustdoc_types::GenericBound::Outlives`
        // stores the full lifetime string WITH the apostrophe (e.g. `"'static"`, `"'a"`).
        // Re-prepend `'` so that A-codec Outlives strings compare equal to C-side strings.
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
        // `Verbatim` is produced by syn for future syntax forms (e.g. `use<'a, T>` precise
        // capture bounds from Rust 2024).  These cannot be round-tripped through the
        // `rustdoc_types::GenericBound` representation at this time, but we must not
        // fail the entire encode: return an unresolved-path TraitBound as a best-effort
        // placeholder so that downstream phases can at least report the bound as an
        // unresolved reference rather than crashing.
        _ => Ok(GenericBound::TraitBound {
            trait_: Path { path: bound_str.to_string(), id: Id(UNRESOLVED_CRATE_ID), args: None },
            generic_params: vec![],
            modifier: TraitBoundModifier::None,
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
#[path = "type_ref_parser_tests.rs"]
mod tests;
