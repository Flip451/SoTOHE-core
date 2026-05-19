//! Serde DTO types for [`CatalogueDocument`] wire format (schema_version = 3).
//!
//! All types in this module are infrastructure-private (`pub(super)`).
//! The domain layer is serialization-free.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de};

use crate::tddd::spec_ground_codec::{InformalGroundRefDto, SpecRefDto};

use super::StrictMap;

// ---------------------------------------------------------------------------
// Minimal version probe DTO
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(super) struct SchemaVersionProbe {
    pub(super) schema_version: u32,
}

// ---------------------------------------------------------------------------
// Top-level document DTO
// ---------------------------------------------------------------------------

/// Top-level catalogue document wire format (schema_version = 3).
///
/// The `types`, `traits`, and `functions` fields are **required** (they must be
/// present as JSON objects even when empty). Omitting any of them is a decode
/// error, not a silent empty-map default. This prevents a truncated catalogue
/// from decoding as a valid zero-entry document.
///
/// Each map uses [`StrictMap`] so that duplicate JSON keys are rejected rather
/// than silently collapsed via last-wins semantics.
#[derive(Debug, Serialize)]
pub(super) struct CatalogueDocumentDto {
    pub(super) schema_version: u32,
    pub(super) crate_name: String,
    pub(super) layer: String,
    pub(super) types: BTreeMap<String, TypeEntryDto>,
    pub(super) traits: BTreeMap<String, TraitEntryDto>,
    pub(super) functions: BTreeMap<String, FunctionEntryDto>,
    /// Inherent impl block declarations. Omitted from JSON when empty
    /// (`skip_serializing_if`) so legacy catalogues stay byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) inherent_impls: Vec<InherentImplDeclDto>,
}

/// Manual `Deserialize` for [`CatalogueDocumentDto`] that uses [`StrictMap`]
/// for the three entry maps instead of the derived `BTreeMap` deserializer.
///
/// The three entry maps (`types`, `traits`, `functions`) are required fields
/// and reject duplicate keys.
impl<'de> Deserialize<'de> for CatalogueDocumentDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            SchemaVersion,
            CrateName,
            Layer,
            Types,
            Traits,
            Functions,
            InherentImpls,
            #[serde(other)]
            Unknown,
        }

        struct DtoVisitor;

        impl<'de> de::Visitor<'de> for DtoVisitor {
            type Value = CatalogueDocumentDto;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("CatalogueDocumentDto")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut schema_version: Option<u32> = None;
                let mut crate_name: Option<String> = None;
                let mut layer: Option<String> = None;
                let mut types: Option<StrictMap<String, TypeEntryDto>> = None;
                let mut traits: Option<StrictMap<String, TraitEntryDto>> = None;
                let mut functions: Option<StrictMap<String, FunctionEntryDto>> = None;
                let mut inherent_impls: Option<Vec<InherentImplDeclDto>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::SchemaVersion => {
                            if schema_version.is_some() {
                                return Err(de::Error::duplicate_field("schema_version"));
                            }
                            schema_version = Some(map.next_value()?);
                        }
                        Field::CrateName => {
                            if crate_name.is_some() {
                                return Err(de::Error::duplicate_field("crate_name"));
                            }
                            crate_name = Some(map.next_value()?);
                        }
                        Field::Layer => {
                            if layer.is_some() {
                                return Err(de::Error::duplicate_field("layer"));
                            }
                            layer = Some(map.next_value()?);
                        }
                        Field::Types => {
                            if types.is_some() {
                                return Err(de::Error::duplicate_field("types"));
                            }
                            types = Some(map.next_value()?);
                        }
                        Field::Traits => {
                            if traits.is_some() {
                                return Err(de::Error::duplicate_field("traits"));
                            }
                            traits = Some(map.next_value()?);
                        }
                        Field::Functions => {
                            if functions.is_some() {
                                return Err(de::Error::duplicate_field("functions"));
                            }
                            functions = Some(map.next_value()?);
                        }
                        Field::InherentImpls => {
                            if inherent_impls.is_some() {
                                return Err(de::Error::duplicate_field("inherent_impls"));
                            }
                            inherent_impls = Some(map.next_value()?);
                        }
                        Field::Unknown => {
                            // Consume the value so the deserializer is in a clean
                            // state, then reject the unknown field to fail closed.
                            // `#[serde(other)]` does not preserve the original key
                            // name; FIELDS lists the accepted keys so the error
                            // message is actionable.
                            let _ = map.next_value::<de::IgnoredAny>()?;
                            return Err(de::Error::unknown_field("(unrecognised field)", FIELDS));
                        }
                    }
                }

                let schema_version =
                    schema_version.ok_or_else(|| de::Error::missing_field("schema_version"))?;
                let crate_name =
                    crate_name.ok_or_else(|| de::Error::missing_field("crate_name"))?;
                let layer = layer.ok_or_else(|| de::Error::missing_field("layer"))?;
                let types = types.ok_or_else(|| de::Error::missing_field("types"))?;
                let traits = traits.ok_or_else(|| de::Error::missing_field("traits"))?;
                let functions = functions.ok_or_else(|| de::Error::missing_field("functions"))?;
                // `inherent_impls` defaults to empty Vec when absent (backward compat).
                let inherent_impls = inherent_impls.unwrap_or_default();

                Ok(CatalogueDocumentDto {
                    schema_version,
                    crate_name,
                    layer,
                    types: types.0,
                    traits: traits.0,
                    functions: functions.0,
                    inherent_impls,
                })
            }
        }

        const FIELDS: &[&str] = &[
            "schema_version",
            "crate_name",
            "layer",
            "types",
            "traits",
            "functions",
            "inherent_impls",
        ];
        deserializer.deserialize_struct("CatalogueDocumentDto", FIELDS, DtoVisitor)
    }
}

// ---------------------------------------------------------------------------
// TypeEntry DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TypeEntryDto {
    #[serde(default = "default_action")]
    pub(super) action: String,
    pub(super) role: String,
    pub(super) kind: TypeKindDto,
    #[serde(default)]
    pub(super) methods: Vec<MethodDeclarationDto>,
    #[serde(default)]
    pub(super) trait_impls: Vec<TraitImplDto>,
    #[serde(default)]
    pub(super) module_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
    /// SoT Chain ② references to spec.json elements. Always emitted (even empty).
    #[serde(default)]
    pub(super) spec_refs: Vec<SpecRefDto>,
    /// Informal ground citations. Always emitted (even empty).
    #[serde(default)]
    pub(super) informal_grounds: Vec<InformalGroundRefDto>,
}

/// Wire format for `TypeKindV2`.
///
/// Uses `#[serde(tag = "kind")]` so JSON looks like:
/// ```json
/// { "kind": "unit_struct" }
/// { "kind": "tuple_struct", "fields": [...], "has_stripped_fields": true }
/// { "kind": "plain_struct", "fields": [...], "has_stripped_fields": false, "typestate": { ... } }
/// { "kind": "enum", "variants": [...] }
/// { "kind": "type_alias", "target": "..." }
/// ```
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(super) enum TypeKindDto {
    /// A unit struct (`pub struct Foo;`). No fields.
    #[serde(rename = "unit_struct")]
    UnitStruct,
    /// A tuple struct (positional fields — unnamed, referenced by `.0`, `.1`, …).
    #[serde(rename = "tuple_struct")]
    TupleStruct {
        /// Positional field types as plain strings (e.g. `["String", "i32"]`).
        /// No `name` key — tuple fields are unnamed.
        #[serde(default)]
        fields: Vec<String>,
        /// `true` when the struct has at least one private field rustdoc omits.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        has_stripped_fields: bool,
    },
    /// A plain named-field struct.
    #[serde(rename = "plain_struct")]
    PlainStruct {
        #[serde(default)]
        fields: Vec<FieldDeclDto>,
        /// `true` when the struct has at least one private field rustdoc omits.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        has_stripped_fields: bool,
        /// Optional typestate membership marker.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        typestate: Option<TypestateMarkerDto>,
    },
    /// A sum / enum type.
    #[serde(rename = "enum")]
    Enum {
        #[serde(default)]
        variants: Vec<VariantDeclDto>,
    },
    /// A type alias.
    #[serde(rename = "type_alias")]
    TypeAlias { target: String },
}

/// Wire format for `TypestateMarker`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TypestateMarkerDto {
    /// The name of the typestate machine this state belongs to.
    pub(super) state_name: String,
    /// Transition method names.
    #[serde(default)]
    pub(super) transition_methods: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FieldDeclDto {
    pub(super) name: String,
    pub(super) ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VariantDeclDto {
    pub(super) name: String,
    #[serde(default)]
    pub(super) payload: VariantPayloadDto,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(super) enum VariantPayloadDto {
    #[default]
    #[serde(rename = "unit")]
    Unit,
    #[serde(rename = "tuple")]
    Tuple { fields: Vec<String> },
    #[serde(rename = "struct")]
    Struct { fields: Vec<FieldDeclDto> },
}

// ---------------------------------------------------------------------------
// TraitEntry DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TraitEntryDto {
    #[serde(default = "default_action")]
    pub(super) action: String,
    pub(super) role: String,
    #[serde(default)]
    pub(super) methods: Vec<MethodDeclarationDto>,
    /// Supertrait bounds (e.g. `["Send", "Sync"]` for `trait Foo: Send + Sync`).
    /// Default empty for backward compatibility with catalogues that predate this field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) supertrait_bounds: Vec<String>,
    #[serde(default)]
    pub(super) module_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
    /// SoT Chain ② references to spec.json elements. Always emitted (even empty).
    #[serde(default)]
    pub(super) spec_refs: Vec<SpecRefDto>,
    /// Informal ground citations. Always emitted (even empty).
    #[serde(default)]
    pub(super) informal_grounds: Vec<InformalGroundRefDto>,
}

// ---------------------------------------------------------------------------
// FunctionEntry DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FunctionEntryDto {
    #[serde(default = "default_action")]
    pub(super) action: String,
    pub(super) role: String,
    #[serde(default)]
    pub(super) params: Vec<ParamDto>,
    pub(super) returns: String,
    #[serde(default)]
    pub(super) is_async: bool,
    /// Generic type parameters on this function. Default empty for catalogues
    /// that predate this field. (ADR `2026-05-08-0248` D14)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) generics: Vec<MethodGenericParamDto>,
    /// `where`-clause bound predicates on this function's generics. Default empty.
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D2)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) where_predicates: Vec<WherePredicateDeclDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
    /// SoT Chain ② references to spec.json elements. Always emitted (even empty).
    #[serde(default)]
    pub(super) spec_refs: Vec<SpecRefDto>,
    /// Informal ground citations. Always emitted (even empty).
    #[serde(default)]
    pub(super) informal_grounds: Vec<InformalGroundRefDto>,
}

// ---------------------------------------------------------------------------
// Shared sub-types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MethodGenericParamDto {
    pub(super) name: String,
    #[serde(default)]
    pub(super) bounds: Vec<String>,
}

/// JSON-shape mirror of `domain::tddd::catalogue_v2::WherePredicateDecl`.
///
/// Wire format uses `"lhs"` / `"rhs"` / `"operator"` fields.
/// Legacy catalogues that used `"type"` / `"bounds"` are supported via
/// `#[serde(alias)]` for backward compatibility (CN-01 / OS-04).
/// The `"operator"` field defaults to `"Bound"` when absent.
///
/// (ADR `2026-05-18-1223-make-catalogue-schema-permissive` D1 — supersedes
/// the 2-field form from `2026-05-13-1153-tddd-where-form-generics-normalization` D2)
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WherePredicateDeclDto {
    /// Left-hand side of the where predicate. Legacy alias: `"type"`.
    #[serde(alias = "type")]
    pub(super) lhs: String,
    /// Right-hand side bounds. Legacy alias: `"bounds"`.
    #[serde(default, alias = "bounds")]
    pub(super) rhs: Vec<String>,
    /// The predicate operator. Defaults to `"Bound"` when absent.
    #[serde(default)]
    pub(super) operator: BoundOpDto,
}

/// Serde-serializable mirror of `domain::tddd::catalogue_v2::BoundOp`.
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub(super) enum BoundOpDto {
    #[default]
    Bound,
    Equal,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MethodDeclarationDto {
    pub(super) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) receiver: Option<String>,
    #[serde(default)]
    pub(super) params: Vec<ParamDto>,
    pub(super) returns: String,
    #[serde(default)]
    pub(super) is_async: bool,
    /// Whether this trait method declaration carries a default implementation
    /// (`provided_trait_methods` in rustdoc). Default false for backward compatibility
    /// with catalogues that predate this field. (ADR `2026-05-08-0248` D13)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) has_default_impl: bool,
    /// Generic type parameters on this method.
    /// Default empty for backward compatibility with catalogues that predate this field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) generics: Vec<MethodGenericParamDto>,
    /// `where`-clause bound predicates on this method's generics.
    /// Default empty for backward compatibility.
    /// (ADR `2026-05-13-1153-tddd-where-form-generics-normalization` D2)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) where_predicates: Vec<WherePredicateDeclDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) docs: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ParamDto {
    pub(super) name: String,
    pub(super) ty: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TraitImplDto {
    pub(super) trait_name: String,
    pub(super) origin_crate: String,
    /// Generic argument string (e.g. `"CatalogueLoaderError"` for `From<CatalogueLoaderError>`).
    /// Optional — absent in catalogues that predate the `generic_args` field extension.
    /// Defaults to `None` for backward compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) generic_args: Option<String>,
}

// ---------------------------------------------------------------------------
// InherentImplDeclV2 DTO
// ---------------------------------------------------------------------------

/// Wire format for `InherentImplDeclV2`.
///
/// Represents one inherent `impl` block for a named type. Multiple entries
/// with the same `type_name` represent multiple inherent impl blocks for one
/// struct (the primary design constraint of IN-05 / IN-08).
///
/// `type_name` is required (no `serde(default)`). All `Vec` fields default
/// to empty when absent for backward compatibility.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InherentImplDeclDto {
    /// The name of the type this impl block belongs to (required).
    pub(super) type_name: String,
    /// Impl-block-level generic type parameters (type parameters only).
    /// Default empty for catalogues that have no impl-level generics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) impl_generics: Vec<MethodGenericParamDto>,
    /// Impl-block-level where-clause predicates. Default empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) impl_where_predicates: Vec<WherePredicateDeclDto>,
    /// Method declarations inside this impl block. Default empty.
    #[serde(default)]
    pub(super) methods: Vec<MethodDeclarationDto>,
}

pub(super) fn default_action() -> String {
    "add".to_owned()
}
