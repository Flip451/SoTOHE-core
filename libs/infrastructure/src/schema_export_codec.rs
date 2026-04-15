//! Serde codec for `SchemaExport` → JSON (encode-only, no decode path).
//!
//! Replaces the `#[derive(Serialize)]` that was placed on domain types in
//! commit `a5e4c6b` (bridge01-export-schema). Domain types carry no serde
//! knowledge; this codec owns the BRIDGE-01 wire format.
//!
//! # Wire-format preservation
//!
//! - Field names match `domain::schema` / `domain::tddd::catalogue` types
//!   1:1 — no `#[serde(rename_all = ...)]`.
//! - Option fields have no `#[serde(skip_serializing_if = "Option::is_none")]`
//!   — the current BRIDGE-01 JSON emits `null` for absent values and the
//!   new DTOs must preserve that behaviour.
//! - `TypeKindDto` and `MemberDeclarationDto` use serde's default
//!   (externally-tagged) enum representation, matching the current domain
//!   `TypeKind` / `MemberDeclaration` representation.
//!
//! # Per-type visibility
//!
//! All 8 DTOs and the error type are `pub` so that rustdoc JSON exposes
//! them for TDDD signal evaluation. Private DTOs would not show up in the
//! rustdoc export and could never transition from Yellow to Blue in the
//! infrastructure catalogue.
//!
//! ADR: `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`

use domain::schema::{FunctionInfo, ImplInfo, SchemaExport, TraitInfo, TypeInfo, TypeKind};
use domain::tddd::catalogue::{MemberDeclaration, ParamDeclaration};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for schema export JSON serialization.
#[derive(Debug, thiserror::Error)]
pub enum SchemaExportCodecError {
    /// Underlying `serde_json` serialization error.
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// DTO types (Serialize-only — no Deserialize needed for encode-only path)
// ---------------------------------------------------------------------------

/// Top-level DTO for the schema export JSON.
///
/// Mirrors `domain::schema::SchemaExport` with identical field names.
#[derive(Debug, Serialize)]
pub struct SchemaExportDto {
    pub crate_name: String,
    pub types: Vec<TypeInfoDto>,
    pub functions: Vec<FunctionInfoDto>,
    pub traits: Vec<TraitInfoDto>,
    pub impls: Vec<ImplInfoDto>,
}

/// DTO mirror of `domain::schema::TypeKind`.
///
/// Uses serde default (externally-tagged, PascalCase variant names) so
/// that JSON output is `"Struct"` / `"Enum"` / `"TypeAlias"`, matching the
/// current BRIDGE-01 wire format produced by the domain enum's own
/// `Serialize` derive (now removed from the domain layer in T004).
#[derive(Debug, Serialize)]
pub enum TypeKindDto {
    Struct,
    Enum,
    TypeAlias,
}

/// DTO mirror of `domain::schema::TypeInfo`.
#[derive(Debug, Serialize)]
pub struct TypeInfoDto {
    pub name: String,
    pub kind: TypeKindDto,
    pub docs: Option<String>,
    pub members: Vec<MemberDeclarationDto>,
    pub module_path: Option<String>,
}

/// DTO mirror of `domain::tddd::catalogue::MemberDeclaration`.
///
/// Uses serde default (externally-tagged) for wire format preservation:
///
/// - `Variant("Name")` → `{"Variant": "Name"}`
/// - `Field { name, ty }` → `{"Field": {"name": "...", "ty": "..."}}`
#[derive(Debug, Serialize)]
pub enum MemberDeclarationDto {
    Variant(String),
    Field { name: String, ty: String },
}

/// DTO mirror of `domain::tddd::catalogue::ParamDeclaration` in the
/// schema-export context.
///
/// The `Schema` prefix distinguishes this from `catalogue_codec::ParamDto`
/// which carries L1 `::` enforcement for catalogue decode validation.
/// `schema_export_codec` is encode-only and does not need that constraint.
#[derive(Debug, Serialize)]
pub struct SchemaParamDto {
    pub name: String,
    pub ty: String,
}

/// DTO mirror of `domain::schema::FunctionInfo`.
#[derive(Debug, Serialize)]
pub struct FunctionInfoDto {
    pub name: String,
    pub docs: Option<String>,
    pub return_type_names: Vec<String>,
    pub has_self_receiver: bool,
    pub params: Vec<SchemaParamDto>,
    pub returns: String,
    pub receiver: Option<String>,
    pub is_async: bool,
}

/// DTO mirror of `domain::schema::TraitInfo`.
#[derive(Debug, Serialize)]
pub struct TraitInfoDto {
    pub name: String,
    pub docs: Option<String>,
    pub methods: Vec<FunctionInfoDto>,
}

/// DTO mirror of `domain::schema::ImplInfo`.
#[derive(Debug, Serialize)]
pub struct ImplInfoDto {
    pub target_type: String,
    pub trait_name: Option<String>,
    pub methods: Vec<FunctionInfoDto>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encodes a [`SchemaExport`] to a JSON string.
///
/// # Arguments
///
/// * `schema` — the domain export value to encode
/// * `pretty` — if `true`, produces indented JSON; otherwise compact JSON
///
/// # Errors
///
/// Returns [`SchemaExportCodecError::Json`] if `serde_json` serialization fails.
pub fn encode(schema: &SchemaExport, pretty: bool) -> Result<String, SchemaExportCodecError> {
    let dto = SchemaExportDto::from(schema);
    if pretty {
        serde_json::to_string_pretty(&dto).map_err(SchemaExportCodecError::Json)
    } else {
        serde_json::to_string(&dto).map_err(SchemaExportCodecError::Json)
    }
}

// ---------------------------------------------------------------------------
// From conversions (infallible — `SchemaExport` is a plain aggregate of
// primitives and `Vec`s, so every From is `clone` + `iter().map()`.)
// ---------------------------------------------------------------------------

impl From<&SchemaExport> for SchemaExportDto {
    fn from(s: &SchemaExport) -> Self {
        Self {
            crate_name: s.crate_name().to_owned(),
            types: s.types().iter().map(TypeInfoDto::from).collect(),
            functions: s.functions().iter().map(FunctionInfoDto::from).collect(),
            traits: s.traits().iter().map(TraitInfoDto::from).collect(),
            impls: s.impls().iter().map(ImplInfoDto::from).collect(),
        }
    }
}

impl From<&TypeKind> for TypeKindDto {
    fn from(k: &TypeKind) -> Self {
        match k {
            TypeKind::Struct => Self::Struct,
            TypeKind::Enum => Self::Enum,
            TypeKind::TypeAlias => Self::TypeAlias,
        }
    }
}

impl From<&TypeInfo> for TypeInfoDto {
    fn from(t: &TypeInfo) -> Self {
        Self {
            name: t.name().to_owned(),
            kind: TypeKindDto::from(t.kind()),
            docs: t.docs().map(str::to_owned),
            members: t.members().iter().map(MemberDeclarationDto::from).collect(),
            module_path: t.module_path().map(str::to_owned),
        }
    }
}

impl From<&MemberDeclaration> for MemberDeclarationDto {
    fn from(m: &MemberDeclaration) -> Self {
        match m {
            MemberDeclaration::Variant(name) => Self::Variant(name.clone()),
            MemberDeclaration::Field { name, ty } => {
                Self::Field { name: name.clone(), ty: ty.clone() }
            }
        }
    }
}

impl From<&ParamDeclaration> for SchemaParamDto {
    fn from(p: &ParamDeclaration) -> Self {
        Self { name: p.name().to_owned(), ty: p.ty().to_owned() }
    }
}

impl From<&FunctionInfo> for FunctionInfoDto {
    fn from(f: &FunctionInfo) -> Self {
        Self {
            name: f.name().to_owned(),
            docs: f.docs().map(str::to_owned),
            return_type_names: f.return_type_names().to_vec(),
            has_self_receiver: f.has_self_receiver(),
            params: f.params().iter().map(SchemaParamDto::from).collect(),
            returns: f.returns().to_owned(),
            receiver: f.receiver().map(str::to_owned),
            is_async: f.is_async(),
        }
    }
}

impl From<&TraitInfo> for TraitInfoDto {
    fn from(t: &TraitInfo) -> Self {
        Self {
            name: t.name().to_owned(),
            docs: t.docs().map(str::to_owned),
            methods: t.methods().iter().map(FunctionInfoDto::from).collect(),
        }
    }
}

impl From<&ImplInfo> for ImplInfoDto {
    fn from(i: &ImplInfo) -> Self {
        Self {
            target_type: i.target_type().to_owned(),
            trait_name: i.trait_name().map(str::to_owned),
            methods: i.methods().iter().map(FunctionInfoDto::from).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::schema::{FunctionInfo, ImplInfo, SchemaExport, TraitInfo, TypeInfo, TypeKind};
    use domain::tddd::catalogue::{MemberDeclaration, ParamDeclaration};

    fn empty_schema() -> SchemaExport {
        SchemaExport::new("empty_crate".to_string(), vec![], vec![], vec![], vec![])
    }

    #[test]
    fn encode_empty_schema_produces_valid_json_with_crate_name() {
        let schema = empty_schema();
        let json = encode(&schema, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["crate_name"], "empty_crate");
        assert_eq!(parsed["types"].as_array().map(Vec::len), Some(0));
        assert_eq!(parsed["functions"].as_array().map(Vec::len), Some(0));
        assert_eq!(parsed["traits"].as_array().map(Vec::len), Some(0));
        assert_eq!(parsed["impls"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn encode_single_entries_each_category() {
        let type_info = TypeInfo::new(
            "Foo".to_string(),
            TypeKind::Struct,
            Some("A struct".to_string()),
            vec![MemberDeclaration::field("x", "i32")],
        );
        let fn_info = FunctionInfo::new(
            "bar".to_string(),
            None,
            vec!["i32".to_string()],
            false,
            vec![ParamDeclaration::new("x", "i32")],
            "i32".to_string(),
            None,
            false,
        );
        let trait_info = TraitInfo::new("MyTrait".to_string(), None, vec![]);
        let impl_info = ImplInfo::new("Foo".to_string(), None, vec![]);

        let schema = SchemaExport::new(
            "test".to_string(),
            vec![type_info],
            vec![fn_info],
            vec![trait_info],
            vec![impl_info],
        );
        let json = encode(&schema, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["crate_name"], "test");
        assert_eq!(parsed["types"].as_array().map(Vec::len), Some(1));
        assert_eq!(parsed["functions"].as_array().map(Vec::len), Some(1));
        assert_eq!(parsed["traits"].as_array().map(Vec::len), Some(1));
        assert_eq!(parsed["impls"].as_array().map(Vec::len), Some(1));

        assert_eq!(parsed["types"][0]["name"], "Foo");
        assert_eq!(parsed["types"][0]["kind"], "Struct");
        assert_eq!(parsed["types"][0]["docs"], "A struct");
        assert_eq!(parsed["types"][0]["members"][0]["Field"]["name"], "x");
        assert_eq!(parsed["types"][0]["members"][0]["Field"]["ty"], "i32");
        assert!(parsed["types"][0]["module_path"].is_null());

        assert_eq!(parsed["functions"][0]["name"], "bar");
        assert_eq!(parsed["functions"][0]["params"][0]["name"], "x");
        assert_eq!(parsed["functions"][0]["params"][0]["ty"], "i32");
        assert_eq!(parsed["functions"][0]["returns"], "i32");
        assert!(parsed["functions"][0]["receiver"].is_null());
        assert_eq!(parsed["functions"][0]["is_async"], false);
        assert_eq!(parsed["functions"][0]["has_self_receiver"], false);

        assert_eq!(parsed["traits"][0]["name"], "MyTrait");
        assert_eq!(parsed["impls"][0]["target_type"], "Foo");
        assert!(parsed["impls"][0]["trait_name"].is_null());
    }

    #[test]
    fn encode_pretty_has_newlines_compact_does_not() {
        let schema = empty_schema();
        let compact = encode(&schema, false).unwrap();
        let pretty = encode(&schema, true).unwrap();
        assert!(!compact.contains('\n'), "compact JSON must not contain newlines");
        assert!(pretty.contains('\n'), "pretty JSON must contain newlines");
    }

    #[test]
    fn encode_member_declaration_variant_uses_externally_tagged_form() {
        let type_info = TypeInfo::new(
            "MyEnum".to_string(),
            TypeKind::Enum,
            None,
            vec![MemberDeclaration::variant("First"), MemberDeclaration::field("x", "u8")],
        );
        let schema = SchemaExport::new("x".to_string(), vec![type_info], vec![], vec![], vec![]);
        let json = encode(&schema, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let members = &parsed["types"][0]["members"];
        assert_eq!(members[0]["Variant"], "First");
        assert_eq!(members[1]["Field"]["name"], "x");
        assert_eq!(members[1]["Field"]["ty"], "u8");
    }

    #[test]
    fn encode_type_kind_uses_pascal_case_variants() {
        let type_info_struct = TypeInfo::new("A".to_string(), TypeKind::Struct, None, vec![]);
        let type_info_enum = TypeInfo::new("B".to_string(), TypeKind::Enum, None, vec![]);
        let type_info_alias = TypeInfo::new("C".to_string(), TypeKind::TypeAlias, None, vec![]);
        let schema = SchemaExport::new(
            "x".to_string(),
            vec![type_info_struct, type_info_enum, type_info_alias],
            vec![],
            vec![],
            vec![],
        );
        let json = encode(&schema, false).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["types"][0]["kind"], "Struct");
        assert_eq!(parsed["types"][1]["kind"], "Enum");
        assert_eq!(parsed["types"][2]["kind"], "TypeAlias");
    }
}
