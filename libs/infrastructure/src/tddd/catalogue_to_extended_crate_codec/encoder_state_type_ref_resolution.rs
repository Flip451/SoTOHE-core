//! `EncoderState` methods for local identity resolution and external-id
//! resolution.
//!
//! Extracted from `encoder_state_type_ref_parsing` to keep the TypeRef codec
//! responsibilities in cohesive modules within the 700-line limit.

use crate::tddd::canonical_type_identity::{
    CanonicalTypeIdentity, canonicalize_catalogue_type_ref,
};
use crate::tddd::type_ref_parser::{STD_PRELUDE_TYPES, UNRESOLVED_CRATE_ID, std_canonical_path};
use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{CatalogueItemNamespace, Identifier, TypeRef};
use domain::tddd::catalogue_v2::roles::NonEmptyVec;
use rustdoc_types::{GenericArg, GenericArgs, Id, Path, Type};

use super::encoder::EncoderState;
use super::invalid_type_ref;

impl EncoderState {
    pub(super) fn local_id_for_identity_in_namespace(
        &self,
        identity: &CanonicalTypeIdentity,
        namespace: CatalogueItemNamespace,
    ) -> Result<Option<Id>, NewTypeGraphCodecError> {
        let Some(entries) = self.local_identity_to_id.get(identity) else {
            return Ok(None);
        };
        let namespace_entries =
            entries.iter().filter(|(path, _)| path.namespace() == namespace).collect::<Vec<_>>();
        match namespace_entries.as_slice() {
            [] => Ok(None),
            [(_, id)] => Ok(Some(*id)),
            [(first_path, _), rest @ ..] => {
                let identifier =
                    Identifier::new(first_path.name().as_str().to_owned()).map_err(|_| {
                        super::invalid_type_ref(identity.as_str(), "invalid identity name")
                    })?;
                let candidates = rest.iter().map(|(path, _)| (*path).clone()).collect::<Vec<_>>();
                Err(NewTypeGraphCodecError::AmbiguousIdentifier(
                    identifier,
                    NonEmptyVec::new((*first_path).clone(), candidates),
                ))
            }
        }
    }

    /// Resolves a parsed path against the catalogue's full-path identity index.
    ///
    /// A bare name is accepted only when it has one candidate. Qualified aliases are
    /// resolved against the canonical `crate::module::name` forms without falling back to
    /// a short name. `None` means the path belongs to an open-world external crate.
    pub(super) fn local_id_for_path(
        &self,
        path: &str,
        namespace: CatalogueItemNamespace,
    ) -> Result<Option<Id>, NewTypeGraphCodecError> {
        let lookup = path.strip_prefix("::").unwrap_or(path);
        let type_ref = TypeRef::new(lookup.to_owned())
            .map_err(|_| invalid_type_ref(lookup, "empty TypeRef path"))?;
        let namespace_paths = self.resolution_paths_for_namespace(namespace);
        match canonicalize_catalogue_type_ref(&type_ref, &self.crate_name, &namespace_paths, &[]) {
            Ok(identity) => self.local_id_for_identity_in_namespace(&identity, namespace),
            Err(error) => Err(error),
        }
    }

    fn local_id_namespace(&self, id: Id) -> Option<CatalogueItemNamespace> {
        self.local_identity_to_id
            .values()
            .flat_map(|entries| entries.iter())
            .find_map(|(path, candidate)| (*candidate == id).then_some(path.namespace()))
    }

    pub(super) fn resolve_trait_ref_for_top_level_in_trait_namespace(
        &mut self,
        trait_ref: &str,
        generic_names: &[&str],
    ) -> Result<Path, NewTypeGraphCodecError> {
        let previous = self.pending_root_namespace.replace(CatalogueItemNamespace::Trait);
        let result = self.resolve_trait_ref_for_top_level(trait_ref, generic_names);
        self.pending_root_namespace = previous;
        result
    }

    pub(super) fn record_resolution_error(&mut self, error: NewTypeGraphCodecError) {
        if self.resolution_error.is_none() {
            self.resolution_error = Some(error);
        }
    }

    /// Post-processes a `Type` tree returned by `parse_type_ref`, replacing the
    /// `UNRESOLVED_CRATE_ID` sentinel with fresh synthetic item ids for
    /// identifiers that are **known externals** (std prelude or crate-prefixed with a
    /// registered external crate).
    ///
    /// Truly-unresolved identifiers (single-segment names that have no `"::"` and
    /// are not registered) keep `Id(UNRESOLVED_CRATE_ID)` so Phase 1 can detect
    /// and reject them.
    ///
    /// ADR D11 / D10: std prelude and crate-prefixed refs must not be flagged as
    /// Phase 1 errors.  Allocating a `paths` entry for them lets the S-construction
    /// algorithm identify them as valid externals without string-pattern heuristics.
    pub(super) fn resolve_external_type_ids(&mut self, ty: Type) -> Type {
        let namespace = self.pending_root_namespace.take().unwrap_or(CatalogueItemNamespace::Type);
        self.resolve_external_type_ids_with_root_namespace(ty, namespace)
    }

    fn resolve_external_type_ids_with_root_namespace(
        &mut self,
        ty: Type,
        root_namespace: CatalogueItemNamespace,
    ) -> Type {
        match ty {
            // `ResolvedPath` — delegate to the shared path helper which fixes up the id
            // and recurses into generic args so nested externals are also corrected.
            Type::ResolvedPath(p) => {
                Type::ResolvedPath(self.resolve_external_type_ids_in_path(p, root_namespace))
            }
            // Recurse into container types.
            Type::Tuple(elems) => {
                Type::Tuple(elems.into_iter().map(|t| self.resolve_external_type_ids(t)).collect())
            }
            Type::Slice(inner) => Type::Slice(Box::new(self.resolve_external_type_ids(*inner))),
            Type::Array { type_, len } => {
                Type::Array { type_: Box::new(self.resolve_external_type_ids(*type_)), len }
            }
            Type::Pat { type_, __pat_unstable_do_not_use } => Type::Pat {
                type_: Box::new(self.resolve_external_type_ids(*type_)),
                __pat_unstable_do_not_use,
            },
            Type::BorrowedRef { lifetime, is_mutable, type_ } => Type::BorrowedRef {
                lifetime,
                is_mutable,
                type_: Box::new(self.resolve_external_type_ids(*type_)),
            },
            Type::RawPointer { is_mutable, type_ } => Type::RawPointer {
                is_mutable,
                type_: Box::new(self.resolve_external_type_ids(*type_)),
            },
            Type::ImplTrait(bounds) => Type::ImplTrait(
                bounds
                    .into_iter()
                    .map(|b| self.resolve_external_type_ids_in_generic_bound(b))
                    .collect(),
            ),
            // `dyn Trait + Trait2` — fix up each bound's trait path and generic args.
            Type::DynTrait(dyn_trait) => {
                let new_traits = dyn_trait
                    .traits
                    .into_iter()
                    .map(|pt| {
                        let new_trait_path = self.resolve_external_type_ids_in_path(
                            pt.trait_,
                            CatalogueItemNamespace::Trait,
                        );
                        rustdoc_types::PolyTrait {
                            trait_: new_trait_path,
                            generic_params: pt.generic_params,
                        }
                    })
                    .collect();
                Type::DynTrait(rustdoc_types::DynTrait {
                    traits: new_traits,
                    lifetime: dyn_trait.lifetime,
                })
            }
            Type::QualifiedPath { name, args, self_type, trait_ } => Type::QualifiedPath {
                name,
                args: args
                    .map(|boxed| Box::new(self.resolve_external_type_ids_in_generic_args(*boxed))),
                self_type: Box::new(self.resolve_external_type_ids(*self_type)),
                trait_: trait_.map(|path| {
                    self.resolve_external_type_ids_in_path(path, CatalogueItemNamespace::Trait)
                }),
            },
            // `fn(A, B) -> C` function pointers — fix up input and output types.
            Type::FunctionPointer(fp) => {
                let new_inputs = fp
                    .sig
                    .inputs
                    .into_iter()
                    .map(|(name, t)| (name, self.resolve_external_type_ids(t)))
                    .collect();
                let new_output = fp.sig.output.map(|t| self.resolve_external_type_ids(t));
                Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
                    sig: rustdoc_types::FunctionSignature {
                        inputs: new_inputs,
                        output: new_output,
                        is_c_variadic: fp.sig.is_c_variadic,
                    },
                    generic_params: fp.generic_params,
                    header: fp.header,
                }))
            }
            // `Primitive`, `Generic`, `Infer`, and any future variants need no id fix-up.
            other => other,
        }
    }

    /// Resolves external type ids inside a `Path` value (used for trait bound paths).
    pub(super) fn resolve_external_type_ids_in_path(
        &mut self,
        path: Path,
        namespace: CatalogueItemNamespace,
    ) -> Path {
        // A preserving-spelling path may retain an absolute `::` prefix for
        // lexical comparison.  Strip that prefix only for external-crate
        // lookup and synthetic path registration; the emitted `Path.path`
        // remains byte-for-byte faithful to the catalogue spelling.
        let lookup_path = path.path.strip_prefix("::").unwrap_or(path.path.as_str());
        // The preserving parser carries the std crate id as a temporary marker
        // for a bare prelude spelling (`Clone`, `Iterator`, ...).  It must be
        // converted to the same synthetic item id used by canonical paths;
        // otherwise Phase 1 would treat the bare path as a local unresolved
        // reference. A catalogue item named like a prelude type is resolved
        // before this marker is emitted; the canonical resolver below remains
        // the authority for the local-vs-prelude decision.
        let is_preserved_std_marker = self.ext_name_to_id.get("std").is_some_and(|&std_id| {
            path.id == Id(std_id) && STD_PRELUDE_TYPES.contains(&lookup_path)
        });
        let new_id = if is_preserved_std_marker {
            match self.local_id_for_path(lookup_path, namespace) {
                Ok(Some(id)) => id,
                Ok(None) => self.ensure_external_type_id(&std_canonical_path(lookup_path), "std"),
                Err(NewTypeGraphCodecError::UnresolvedIdentifier(_)) => {
                    self.ensure_external_type_id(&std_canonical_path(lookup_path), "std")
                }
                Err(error) => {
                    self.record_resolution_error(error);
                    Id(UNRESOLVED_CRATE_ID)
                }
            }
        } else if path.id == Id(UNRESOLVED_CRATE_ID) {
            match self.local_id_for_path(lookup_path, namespace) {
                Ok(Some(id)) => id,
                Err(error) => {
                    self.record_resolution_error(error);
                    Id(UNRESOLVED_CRATE_ID)
                }
                Ok(None) => {
                    if let Some(colon_pos) = lookup_path.find("::") {
                        let first_seg = &lookup_path[..colon_pos];
                        if self.ext_name_to_id.contains_key(first_seg) {
                            self.ensure_external_type_id(lookup_path, first_seg)
                        } else {
                            Id(UNRESOLVED_CRATE_ID)
                        }
                    } else if STD_PRELUDE_TYPES.contains(&lookup_path) {
                        // Alias bounds retain their catalogue spelling (for example,
                        // `Clone`) for lexical comparison.  Their bare spelling still
                        // denotes a known std external, so register the canonical path
                        // and retain the short spelling on the emitted `Path`.
                        self.ensure_external_type_id(&std_canonical_path(lookup_path), "std")
                    } else {
                        Id(UNRESOLVED_CRATE_ID)
                    }
                }
            }
        } else {
            match self.local_id_namespace(path.id) {
                None => path.id,
                Some(_) => match self.local_id_for_path(lookup_path, namespace) {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        self.record_resolution_error(invalid_type_ref(
                            lookup_path,
                            "local path has no identity in the requested namespace",
                        ));
                        Id(UNRESOLVED_CRATE_ID)
                    }
                    Err(error) => {
                        self.record_resolution_error(error);
                        Id(UNRESOLVED_CRATE_ID)
                    }
                },
            }
        };
        let new_args =
            path.args.map(|boxed| Box::new(self.resolve_external_type_ids_in_generic_args(*boxed)));
        Path { path: path.path, id: new_id, args: new_args }
    }

    /// Resolves external type ids inside a `GenericArgs` value.
    pub(super) fn resolve_external_type_ids_in_generic_args(
        &mut self,
        args: GenericArgs,
    ) -> GenericArgs {
        match args {
            GenericArgs::AngleBracketed { args: ga_args, constraints } => {
                let new_args = ga_args
                    .into_iter()
                    .map(|ga| match ga {
                        GenericArg::Type(t) => GenericArg::Type(self.resolve_external_type_ids(t)),
                        other => other,
                    })
                    .collect();
                let new_constraints = constraints
                    .into_iter()
                    .map(|c| {
                        use rustdoc_types::{AssocItemConstraint, AssocItemConstraintKind, Term};
                        let binding = match c.binding {
                            AssocItemConstraintKind::Equality(Term::Type(t)) => {
                                AssocItemConstraintKind::Equality(Term::Type(
                                    self.resolve_external_type_ids(t),
                                ))
                            }
                            AssocItemConstraintKind::Constraint(bounds) => {
                                AssocItemConstraintKind::Constraint(
                                    bounds
                                        .into_iter()
                                        .map(|b| self.resolve_external_type_ids_in_generic_bound(b))
                                        .collect(),
                                )
                            }
                            other => other,
                        };
                        AssocItemConstraint { binding, ..c }
                    })
                    .collect();
                GenericArgs::AngleBracketed { args: new_args, constraints: new_constraints }
            }
            GenericArgs::Parenthesized { inputs, output } => {
                let new_inputs =
                    inputs.into_iter().map(|t| self.resolve_external_type_ids(t)).collect();
                let new_output = output.map(|t| self.resolve_external_type_ids(t));
                GenericArgs::Parenthesized { inputs: new_inputs, output: new_output }
            }
            // `ReturnTypeNotation` (e.g. `Trait(..)`): no type args to fix up.
            other @ GenericArgs::ReturnTypeNotation => other,
        }
    }

    /// Resolves external type ids inside a `GenericBound` value.
    pub(super) fn resolve_external_type_ids_in_generic_bound(
        &mut self,
        bound: rustdoc_types::GenericBound,
    ) -> rustdoc_types::GenericBound {
        use rustdoc_types::GenericBound;
        match bound {
            GenericBound::TraitBound { trait_, generic_params, modifier } => {
                // A GenericBound::TraitBound is always a trait-position path.
                // The parser can assign a local id before the surrounding syntax
                // is available, so the post-processing pass must revalidate it in
                // the trait namespace rather than preserving a type id.
                let new_trait =
                    self.resolve_external_type_ids_in_path(trait_, CatalogueItemNamespace::Trait);
                GenericBound::TraitBound { trait_: new_trait, generic_params, modifier }
            }
            other => other,
        }
    }
}
