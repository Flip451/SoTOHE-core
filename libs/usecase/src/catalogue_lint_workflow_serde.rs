//! Private serde wire conversion for catalogue-lint rule DTOs.

use domain::tddd::catalogue_v2::identifiers::TypeRef;
use domain::tddd::catalogue_v2::roles::NonEmptyVec;
use domain::tddd::layer_id::LayerId;
use domain::tddd::primitive_occurrence_scanner::{PrimitiveName, PrimitiveOccurrencePosition};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    CatalogueLintError, LintRuleKind, parse_primitive_occurrence_position, parse_role_kind,
    parse_role_payload_field, parse_self_receiver,
};

/// Serde-only representation of [`LintRuleKind`].
///
/// The config file remains a string-based JSON format, while the public
/// usecase DTO retains domain value objects after decoding. Keeping this
/// representation private prevents wire primitives from leaking into the
/// usecase API.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum LintRuleKindWire {
    FieldEmpty { target_field: String },
    FieldNonEmpty { target_field: String },
    KindLayerConstraint { permitted_layers: Vec<String> },
    ReferencedRoleConstraint { target_field: String, expected_role: String },
    TraitImplRequired { required_traits: Vec<String> },
    NoRoleInMethodSignature { forbidden_roles: Vec<String> },
    NoLayerInMethodSignature { forbidden_layers: Vec<String> },
    MethodReferenceSignature { target_field: String },
    AccessorSignatureRequired { target_field: String },
    FieldElementUniqueAcrossEntries { target_field: String },
    NoExternalReferenceInMethods { target_field: String },
    NoPublicField,
    ForbiddenMethodReceiver { forbidden_receiver: String },
    ForbidPrimitiveInTypes { primitives: Vec<String>, layers: Vec<String>, positions: Vec<String> },
    CompositionRootPureDi,
}

impl<'de> Deserialize<'de> for LintRuleKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = LintRuleKindWire::deserialize(deserializer)?;
        lint_rule_kind_from_wire(wire).map_err(serde::de::Error::custom)
    }
}

impl Serialize for LintRuleKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        LintRuleKindWire::from(self).serialize(serializer)
    }
}

impl From<&LintRuleKind> for LintRuleKindWire {
    fn from(kind: &LintRuleKind) -> Self {
        match kind {
            LintRuleKind::FieldEmpty { target_field } => {
                Self::FieldEmpty { target_field: target_field.to_string() }
            }
            LintRuleKind::FieldNonEmpty { target_field } => {
                Self::FieldNonEmpty { target_field: target_field.to_string() }
            }
            LintRuleKind::KindLayerConstraint { permitted_layers } => Self::KindLayerConstraint {
                permitted_layers: permitted_layers
                    .as_slice()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            },
            LintRuleKind::ReferencedRoleConstraint { target_field, expected_role } => {
                Self::ReferencedRoleConstraint {
                    target_field: target_field.to_string(),
                    expected_role: expected_role.variant_name().to_owned(),
                }
            }
            LintRuleKind::TraitImplRequired { required_traits } => Self::TraitImplRequired {
                required_traits: required_traits
                    .as_slice()
                    .iter()
                    .map(|value| value.as_str().to_owned())
                    .collect(),
            },
            LintRuleKind::NoRoleInMethodSignature { forbidden_roles } => {
                Self::NoRoleInMethodSignature {
                    forbidden_roles: forbidden_roles
                        .as_slice()
                        .iter()
                        .map(|role| role.variant_name().to_owned())
                        .collect(),
                }
            }
            LintRuleKind::NoLayerInMethodSignature { forbidden_layers } => {
                Self::NoLayerInMethodSignature {
                    forbidden_layers: forbidden_layers
                        .as_slice()
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                }
            }
            LintRuleKind::MethodReferenceSignature { target_field } => {
                Self::MethodReferenceSignature { target_field: target_field.to_string() }
            }
            LintRuleKind::AccessorSignatureRequired { target_field } => {
                Self::AccessorSignatureRequired { target_field: target_field.to_string() }
            }
            LintRuleKind::FieldElementUniqueAcrossEntries { target_field } => {
                Self::FieldElementUniqueAcrossEntries { target_field: target_field.to_string() }
            }
            LintRuleKind::NoExternalReferenceInMethods { target_field } => {
                Self::NoExternalReferenceInMethods { target_field: target_field.to_string() }
            }
            LintRuleKind::NoPublicField => Self::NoPublicField,
            LintRuleKind::ForbiddenMethodReceiver { forbidden_receiver } => {
                Self::ForbiddenMethodReceiver { forbidden_receiver: forbidden_receiver.to_string() }
            }
            LintRuleKind::ForbidPrimitiveInTypes { primitives, layers, positions } => {
                Self::ForbidPrimitiveInTypes {
                    primitives: primitives
                        .as_slice()
                        .iter()
                        .map(|value| value.as_str().to_owned())
                        .collect(),
                    layers: layers.as_slice().iter().map(ToString::to_string).collect(),
                    positions: positions
                        .as_slice()
                        .iter()
                        .map(primitive_occurrence_position_name)
                        .map(str::to_owned)
                        .collect(),
                }
            }
            LintRuleKind::CompositionRootPureDi => Self::CompositionRootPureDi,
        }
    }
}

fn primitive_occurrence_position_name(position: &PrimitiveOccurrencePosition) -> &'static str {
    match position {
        PrimitiveOccurrencePosition::NamedField => "named_field",
        PrimitiveOccurrencePosition::VariantField => "variant_field",
        PrimitiveOccurrencePosition::Param => "param",
        PrimitiveOccurrencePosition::Return => "return",
        PrimitiveOccurrencePosition::Bound => "bound",
        PrimitiveOccurrencePosition::TypeAliasTarget => "type_alias_target",
        PrimitiveOccurrencePosition::ResultErr => "result_err",
    }
}

fn parse_non_empty<T>(
    values: Vec<String>,
    field_name: &str,
    parse: impl FnMut(String) -> Result<T, CatalogueLintError>,
) -> Result<NonEmptyVec<T>, CatalogueLintError> {
    let parsed = values.into_iter().map(parse).collect::<Result<Vec<_>, _>>()?;
    NonEmptyVec::try_new(parsed)
        .map_err(|_| CatalogueLintError(format!("{field_name} must not be empty")))
}

fn lint_rule_kind_from_wire(wire: LintRuleKindWire) -> Result<LintRuleKind, CatalogueLintError> {
    match wire {
        LintRuleKindWire::FieldEmpty { target_field } => {
            Ok(LintRuleKind::FieldEmpty { target_field: parse_role_payload_field(&target_field)? })
        }
        LintRuleKindWire::FieldNonEmpty { target_field } => Ok(LintRuleKind::FieldNonEmpty {
            target_field: parse_role_payload_field(&target_field)?,
        }),
        LintRuleKindWire::KindLayerConstraint { permitted_layers } => {
            let permitted_layers =
                parse_non_empty(permitted_layers, "permitted_layers", |value| {
                    LayerId::try_new(value.clone()).map_err(|error| {
                        CatalogueLintError(format!("invalid layer_id '{value}': {error}"))
                    })
                })?;
            Ok(LintRuleKind::KindLayerConstraint { permitted_layers })
        }
        LintRuleKindWire::ReferencedRoleConstraint { target_field, expected_role } => {
            Ok(LintRuleKind::ReferencedRoleConstraint {
                target_field: parse_role_payload_field(&target_field)?,
                expected_role: parse_role_kind(&expected_role)?,
            })
        }
        LintRuleKindWire::TraitImplRequired { required_traits } => {
            let required_traits = parse_non_empty(required_traits, "required_traits", |value| {
                TypeRef::new(value.clone()).map_err(|error| {
                    CatalogueLintError(format!("invalid required trait '{value}': {error}"))
                })
            })?;
            Ok(LintRuleKind::TraitImplRequired { required_traits })
        }
        LintRuleKindWire::NoRoleInMethodSignature { forbidden_roles } => {
            let forbidden_roles = parse_non_empty(forbidden_roles, "forbidden_roles", |value| {
                parse_role_kind(&value)
            })?;
            Ok(LintRuleKind::NoRoleInMethodSignature { forbidden_roles })
        }
        LintRuleKindWire::NoLayerInMethodSignature { forbidden_layers } => {
            let forbidden_layers =
                parse_non_empty(forbidden_layers, "forbidden_layers", |value| {
                    LayerId::try_new(value.clone()).map_err(|error| {
                        CatalogueLintError(format!("invalid layer_id '{value}': {error}"))
                    })
                })?;
            Ok(LintRuleKind::NoLayerInMethodSignature { forbidden_layers })
        }
        LintRuleKindWire::MethodReferenceSignature { target_field } => {
            Ok(LintRuleKind::MethodReferenceSignature {
                target_field: parse_role_payload_field(&target_field)?,
            })
        }
        LintRuleKindWire::AccessorSignatureRequired { target_field } => {
            Ok(LintRuleKind::AccessorSignatureRequired {
                target_field: parse_role_payload_field(&target_field)?,
            })
        }
        LintRuleKindWire::FieldElementUniqueAcrossEntries { target_field } => {
            Ok(LintRuleKind::FieldElementUniqueAcrossEntries {
                target_field: parse_role_payload_field(&target_field)?,
            })
        }
        LintRuleKindWire::NoExternalReferenceInMethods { target_field } => {
            Ok(LintRuleKind::NoExternalReferenceInMethods {
                target_field: parse_role_payload_field(&target_field)?,
            })
        }
        LintRuleKindWire::NoPublicField => Ok(LintRuleKind::NoPublicField),
        LintRuleKindWire::ForbiddenMethodReceiver { forbidden_receiver } => {
            Ok(LintRuleKind::ForbiddenMethodReceiver {
                forbidden_receiver: parse_self_receiver(&forbidden_receiver)?,
            })
        }
        LintRuleKindWire::ForbidPrimitiveInTypes { primitives, layers, positions } => {
            let primitives = parse_non_empty(primitives, "primitives", |value| {
                PrimitiveName::new(value.clone()).map_err(|error| {
                    CatalogueLintError(format!("invalid primitive '{value}': {error}"))
                })
            })?;
            let layers = parse_non_empty(layers, "layers", |value| {
                LayerId::try_new(value.clone()).map_err(|error| {
                    CatalogueLintError(format!("invalid layer_id '{value}': {error}"))
                })
            })?;
            let positions = parse_non_empty(positions, "positions", |value| {
                parse_primitive_occurrence_position(&value)
            })?;
            Ok(LintRuleKind::ForbidPrimitiveInTypes { primitives, layers, positions })
        }
        LintRuleKindWire::CompositionRootPureDi => Ok(LintRuleKind::CompositionRootPureDi),
    }
}
