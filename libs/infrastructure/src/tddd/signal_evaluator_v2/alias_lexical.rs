//! Lexical-signature serialization for type-alias generic comparison.
//!
//! Alias declarations are a document-level contract: parameter names, order,
//! and bound spelling are all observable, so Phase 2 compares them through a
//! JSON serialization of the rustdoc declaration rather than the
//! name-independent structural fingerprints used for functions and traits.
//! This module owns that serialization and the normalizations that keep it
//! aligned with rustdoc's own representation.

use serde::Serialize;

/// Encodes a rustdoc declaration while excluding graph-local `Id` values.
///
/// Alias generics are a lexical document contract: paths and nested generic
/// arguments must remain verbatim.  Rustdoc identifiers describe the graph
/// instance, not the declaration, so they are removed before comparison.
pub(super) fn type_alias_lexical_signature<T: Serialize>(
    value: &T,
) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_value(value)?;
    remove_rustdoc_ids(&mut json);
    normalize_placeholder_reference_lifetimes(&mut json);
    normalize_ambiguous_generic_arguments(&mut json);
    serde_json::to_string(&json)
}

/// Rustdoc treats an explicit placeholder lifetime on a reference (`&'_ str`)
/// as elided and emits `lifetime: null`, while the catalogue parser preserves
/// the written `'_` spelling. Both spellings denote the same declaration, so
/// the placeholder is normalized to the rustdoc representation before
/// comparison. Named lifetimes are left untouched.
fn normalize_placeholder_reference_lifetimes(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_placeholder_reference_lifetimes(value);
            }
        }
        serde_json::Value::Object(values) => {
            if let Some(borrowed) =
                values.get_mut("borrowed_ref").and_then(serde_json::Value::as_object_mut)
            {
                if borrowed.get("lifetime").and_then(serde_json::Value::as_str) == Some("'_") {
                    borrowed.insert("lifetime".to_owned(), serde_json::Value::Null);
                }
            }
            for value in values.values_mut() {
                normalize_placeholder_reference_lifetimes(value);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn normalize_ambiguous_generic_arguments(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_ambiguous_generic_arguments(value);
            }
        }
        serde_json::Value::Object(values) => {
            if let Some(serde_json::Value::Array(args)) = values.get_mut("args") {
                for arg in args {
                    normalize_generic_argument_lexeme(arg);
                }
            }
            if let Some(serde_json::Value::Array(constraints)) = values.get_mut("constraints") {
                for constraint in constraints {
                    if let Some(binding) =
                        constraint.as_object_mut().and_then(|object| object.get_mut("binding"))
                    {
                        normalize_term_lexeme(binding);
                    }
                }
            }
            for value in values.values_mut() {
                normalize_ambiguous_generic_arguments(value);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn normalize_generic_argument_lexeme(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if object.len() != 1 {
        return;
    }
    if let Some(type_value) = object.remove("type") {
        if let Some(lexeme) = bare_type_lexeme(&type_value) {
            *value = serde_json::json!({ "lexical": lexeme });
        } else {
            object.insert("type".to_owned(), type_value);
        }
    } else if let Some(const_value) = object.remove("const") {
        if let Some(lexeme) = bare_const_lexeme(&const_value) {
            *value = serde_json::json!({ "lexical": lexeme });
        } else {
            object.insert("const".to_owned(), const_value);
        }
    }
}

fn normalize_term_lexeme(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if let Some(term) = object.get_mut("equality") {
        normalize_term_lexeme(term);
        return;
    }
    if object.len() != 1 {
        return;
    }
    if let Some(type_value) = object.remove("type") {
        if let Some(lexeme) = bare_type_lexeme(&type_value) {
            *value = serde_json::json!({ "lexical": lexeme });
        } else {
            object.insert("type".to_owned(), type_value);
        }
    } else if let Some(const_value) = object.remove("constant") {
        if let Some(lexeme) = bare_const_lexeme(&const_value) {
            *value = serde_json::json!({ "lexical": lexeme });
        } else {
            object.insert("constant".to_owned(), const_value);
        }
    }
}

fn bare_type_lexeme(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(name) = object.get("generic").and_then(serde_json::Value::as_str) {
        return Some(name.to_owned());
    }
    if let Some(name) = object.get("primitive").and_then(serde_json::Value::as_str) {
        return Some(name.to_owned());
    }
    let path = object.get("resolved_path")?.as_object()?;
    let args = path.get("args");
    if args.is_some_and(|args| !args.is_null()) {
        return None;
    }
    path.get("path").and_then(serde_json::Value::as_str).map(ToOwned::to_owned)
}

fn bare_const_lexeme(value: &serde_json::Value) -> Option<String> {
    let expr = value.as_object()?.get("expr")?.as_str()?;
    let mut segments = expr.strip_prefix("::").unwrap_or(expr).split("::");
    if segments.next().is_none()
        || !segments.all(|segment| {
            let mut chars = segment.chars();
            matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
                && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        })
    {
        return None;
    }
    Some(expr.to_owned())
}

fn remove_rustdoc_ids(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                remove_rustdoc_ids(value);
            }
        }
        serde_json::Value::Object(values) => {
            values.remove("id");
            for value in values.values_mut() {
                remove_rustdoc_ids(value);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rustdoc_types::{
        AssocItemConstraint, AssocItemConstraintKind, Constant, GenericArg, GenericArgs, Id, Path,
        Term, Type,
    };

    use super::type_alias_lexical_signature;

    #[test]
    fn test_type_alias_lexical_signature_normalizes_bare_type_const_arguments() {
        let type_args = GenericArgs::AngleBracketed {
            args: vec![GenericArg::Type(Type::ResolvedPath(Path {
                path: "N".to_owned(),
                id: Id(1),
                args: None,
            }))],
            constraints: vec![],
        };
        let const_args = GenericArgs::AngleBracketed {
            args: vec![GenericArg::Const(Constant {
                expr: "N".to_owned(),
                value: None,
                is_literal: false,
            })],
            constraints: vec![],
        };
        assert_eq!(
            type_alias_lexical_signature(&type_args).unwrap(),
            type_alias_lexical_signature(&const_args).unwrap()
        );

        let type_constraint =
            GenericArgs::AngleBracketed {
                args: vec![],
                constraints: vec![AssocItemConstraint {
                    name: "FLAG".to_owned(),
                    args: None,
                    binding: AssocItemConstraintKind::Equality(Term::Type(Type::ResolvedPath(
                        Path { path: "N".to_owned(), id: Id(1), args: None },
                    ))),
                }],
            };
        let const_constraint = GenericArgs::AngleBracketed {
            args: vec![],
            constraints: vec![AssocItemConstraint {
                name: "FLAG".to_owned(),
                args: None,
                binding: AssocItemConstraintKind::Equality(Term::Constant(Constant {
                    expr: "N".to_owned(),
                    value: None,
                    is_literal: false,
                })),
            }],
        };
        assert_eq!(
            type_alias_lexical_signature(&type_constraint).unwrap(),
            type_alias_lexical_signature(&const_constraint).unwrap()
        );
    }
}
