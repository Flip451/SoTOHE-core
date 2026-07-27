//! Validated, per-layer Cargo feature declarations for TDDD extraction.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::LayerId;
use crate::tddd::test_obligation::ids::DiagnosticMessage;

/// A validated Cargo feature token.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CargoFeatureName(String);

impl CargoFeatureName {
    /// Validates and creates a Cargo feature token.
    ///
    /// # Errors
    ///
    /// Returns [`CargoFeatureNameError::InvalidFeatureName`] when the token does not match
    /// Cargo's feature-name grammar.
    pub fn try_new(value: String) -> Result<Self, CargoFeatureNameError> {
        if is_valid_cargo_feature_name(&value) {
            return Ok(Self(value));
        }
        Err(CargoFeatureNameError::InvalidFeatureName(diagnostic(value)))
    }

    /// Borrows the validated feature token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Construction failure for [`CargoFeatureName`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoFeatureNameError {
    /// The supplied value is not a supported Cargo feature token.
    InvalidFeatureName(DiagnosticMessage),
}

impl fmt::Display for CargoFeatureNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFeatureName(reason) => {
                write!(formatter, "invalid Cargo feature name: {}", reason.as_str())
            }
        }
    }
}

impl std::error::Error for CargoFeatureNameError {}

/// A validated, total mapping from TDDD layers to their Cargo features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TdddFeatureDeclaration {
    layers: BTreeMap<LayerId, Vec<CargoFeatureName>>,
}

impl TdddFeatureDeclaration {
    /// Validates a declaration against its required layer set.
    ///
    /// # Errors
    ///
    /// Returns [`TdddFeatureDeclarationError`] when a required layer is absent,
    /// an unexpected layer is present, or a layer declares the same feature twice.
    pub fn try_new(
        layers: BTreeMap<LayerId, Vec<CargoFeatureName>>,
        required_layers: &[LayerId],
    ) -> Result<Self, TdddFeatureDeclarationError> {
        let required: BTreeSet<_> = required_layers.iter().cloned().collect();
        for layer in &required {
            if !layers.contains_key(layer) {
                return Err(TdddFeatureDeclarationError::MissingLayer(layer.clone()));
            }
        }
        for layer in layers.keys() {
            if !required.contains(layer) {
                return Err(TdddFeatureDeclarationError::UnexpectedLayer(layer.clone()));
            }
        }
        for (layer, features) in &layers {
            let mut seen = BTreeSet::new();
            for feature in features {
                if !seen.insert(feature.clone()) {
                    return Err(TdddFeatureDeclarationError::DuplicateFeature {
                        layer: layer.clone(),
                        feature: feature.clone(),
                    });
                }
            }
        }
        Ok(Self { layers })
    }

    /// Returns the declared features for one layer.
    ///
    /// # Errors
    ///
    /// Returns [`TdddFeatureLookupError::MissingLayer`] if the layer is absent.
    pub fn features_for(
        &self,
        layer: &LayerId,
    ) -> Result<&[CargoFeatureName], TdddFeatureLookupError> {
        self.layers
            .get(layer)
            .map(Vec::as_slice)
            .ok_or_else(|| TdddFeatureLookupError::MissingLayer(layer.clone()))
    }

    /// Borrows the complete validated declaration.
    #[must_use]
    pub fn layers(&self) -> &BTreeMap<LayerId, Vec<CargoFeatureName>> {
        &self.layers
    }
}

/// Construction failures for [`TdddFeatureDeclaration`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TdddFeatureDeclarationError {
    /// A TDDD-enabled layer was not declared.
    MissingLayer(LayerId),
    /// A declaration named a layer not enabled for the operation.
    UnexpectedLayer(LayerId),
    /// One layer declared the same feature more than once.
    DuplicateFeature { layer: LayerId, feature: CargoFeatureName },
}

impl fmt::Display for TdddFeatureDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLayer(layer) => {
                write!(formatter, "feature declaration is missing layer '{layer}'")
            }
            Self::UnexpectedLayer(layer) => {
                write!(formatter, "feature declaration has unexpected layer '{layer}'")
            }
            Self::DuplicateFeature { layer, feature } => write!(
                formatter,
                "feature declaration repeats feature '{}' for layer '{layer}'",
                feature.as_str()
            ),
        }
    }
}

impl std::error::Error for TdddFeatureDeclarationError {}

/// Lookup failure for [`TdddFeatureDeclaration::features_for`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TdddFeatureLookupError {
    /// The requested layer is absent from the declaration.
    MissingLayer(LayerId),
}

impl fmt::Display for TdddFeatureLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLayer(layer) => {
                write!(formatter, "feature declaration is missing layer '{layer}'")
            }
        }
    }
}

impl std::error::Error for TdddFeatureLookupError {}

fn is_valid_cargo_feature_name(value: &str) -> bool {
    let mut characters = value.chars();
    match characters.next() {
        Some(character)
            if unicode_ident::is_xid_start(character)
                || character == '_'
                || character.is_ascii_digit() => {}
        _ => return false,
    }
    characters.all(|character| {
        unicode_ident::is_xid_continue(character) || matches!(character, '-' | '+' | '.')
    })
}

fn diagnostic(value: String) -> DiagnosticMessage {
    let mut text = if value.trim().is_empty() {
        "feature name must not be empty".to_owned()
    } else {
        format!("'{value}' is not a valid Cargo feature token")
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            Err(_) => text = "invalid Cargo feature token".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn layer(value: &str) -> LayerId {
        LayerId::try_new(value).unwrap()
    }

    fn feature(value: &str) -> CargoFeatureName {
        CargoFeatureName::try_new(value.to_owned()).unwrap()
    }

    #[test]
    fn test_cargo_feature_name_with_valid_token_succeeds() {
        assert_eq!(feature("semantic-dup").as_str(), "semantic-dup");
    }

    #[test]
    fn test_cargo_feature_name_with_invalid_token_returns_error() {
        assert!(matches!(
            CargoFeatureName::try_new("not/a-feature".to_owned()),
            Err(CargoFeatureNameError::InvalidFeatureName(_))
        ));
    }

    #[test]
    fn test_cargo_feature_name_with_leading_hyphen_returns_error() {
        assert!(matches!(
            CargoFeatureName::try_new("-foo".to_owned()),
            Err(CargoFeatureNameError::InvalidFeatureName(_))
        ));
    }

    #[test]
    fn test_cargo_feature_name_with_plus_sign_succeeds() {
        assert_eq!(feature("foo+bar").as_str(), "foo+bar");
    }

    #[test]
    fn test_cargo_feature_name_with_period_succeeds() {
        assert_eq!(feature("foo.bar").as_str(), "foo.bar");
    }

    #[test]
    fn test_cargo_feature_name_with_unicode_xid_character_succeeds() {
        assert_eq!(feature("\u{65e5}\u{672c}").as_str(), "\u{65e5}\u{672c}");
    }

    #[test]
    fn test_cargo_feature_name_with_empty_value_returns_error() {
        assert!(matches!(
            CargoFeatureName::try_new(String::new()),
            Err(CargoFeatureNameError::InvalidFeatureName(reason))
                if reason.as_str() == "feature name must not be empty"
        ));
    }

    #[test]
    fn test_tddd_feature_declaration_with_total_layers_succeeds() {
        let declaration = TdddFeatureDeclaration::try_new(
            BTreeMap::from([
                (layer("domain"), vec![]),
                (layer("infrastructure"), vec![feature("semantic-dup")]),
            ]),
            &[layer("domain"), layer("infrastructure")],
        )
        .unwrap();
        assert_eq!(
            declaration.features_for(&layer("infrastructure")).unwrap(),
            &[feature("semantic-dup")]
        );
        assert!(declaration.layers().contains_key(&layer("domain")));
        assert!(declaration.layers().get(&layer("domain")).is_some_and(Vec::is_empty));
    }

    #[test]
    fn test_tddd_feature_declaration_with_missing_layer_returns_error() {
        let result = TdddFeatureDeclaration::try_new(
            BTreeMap::from([(layer("domain"), vec![])]),
            &[layer("domain"), layer("usecase")],
        );
        assert!(
            matches!(result, Err(TdddFeatureDeclarationError::MissingLayer(missing)) if missing == layer("usecase"))
        );
    }

    #[test]
    fn test_tddd_feature_declaration_with_unexpected_layer_returns_error() {
        let result = TdddFeatureDeclaration::try_new(
            BTreeMap::from([(layer("domain"), vec![]), (layer("usecase"), vec![])]),
            &[layer("domain")],
        );
        assert!(
            matches!(result, Err(TdddFeatureDeclarationError::UnexpectedLayer(unexpected)) if unexpected == layer("usecase"))
        );
    }

    #[test]
    fn test_tddd_feature_declaration_with_duplicate_feature_returns_error() {
        let result = TdddFeatureDeclaration::try_new(
            BTreeMap::from([(
                layer("domain"),
                vec![feature("semantic-dup"), feature("semantic-dup")],
            )]),
            &[layer("domain")],
        );
        assert!(matches!(result, Err(TdddFeatureDeclarationError::DuplicateFeature { .. })));
    }

    #[test]
    fn test_tddd_feature_declaration_lookup_for_missing_layer_returns_error() {
        let declaration = TdddFeatureDeclaration::try_new(
            BTreeMap::from([(layer("domain"), vec![])]),
            &[layer("domain")],
        )
        .unwrap();
        assert!(
            matches!(declaration.features_for(&layer("usecase")), Err(TdddFeatureLookupError::MissingLayer(missing)) if missing == layer("usecase"))
        );
    }
}
