//! Typed YAML front-matter parsing for provider-native definitions.

use std::collections::BTreeMap;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer};

pub(crate) fn read_front_matter(content: &str) -> Result<Option<&str>, String> {
    let Some(after_open) = content.strip_prefix("---\n") else {
        return Ok(None);
    };
    let Some((front_matter, _)) = after_open.split_once("\n---\n") else {
        return Err("adapter definition has an unclosed YAML front matter block".to_owned());
    };
    Ok(Some(front_matter))
}

/// Structurally validated YAML front matter from a provider-native definition.
///
/// Fields whose values influence provider dispatch are decoded only from the
/// top-level mapping. Additional definition fields must be scalar, which
/// prevents a nested mapping from hiding an identity, `sandbox`, `model`, or
/// `tools` declaration that a line-oriented parser might accidentally accept.
#[derive(Debug, Deserialize)]
pub(crate) struct ProviderDefinitionFrontMatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    sandbox: SandboxDeclaration,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Option<ToolsDeclaration>,
    #[serde(flatten)]
    _other_fields: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ToolsDeclaration {
    Scalar(String),
    List(Vec<String>),
}

#[derive(Debug, Default)]
enum SandboxDeclaration {
    #[default]
    Absent,
    Declared(String),
}

impl<'de> Deserialize<'de> for SandboxDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SandboxVisitor;

        impl Visitor<'_> for SandboxVisitor {
            type Value = SandboxDeclaration;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a non-null sandbox declaration string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SandboxDeclaration::Declared(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(SandboxDeclaration::Declared(value))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("sandbox declaration must not be null"))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Err(E::custom("sandbox declaration must not be null"))
            }
        }

        deserializer.deserialize_any(SandboxVisitor)
    }
}

impl ProviderDefinitionFrontMatter {
    /// Ensures this definition is discoverable and registered for the expected capability.
    ///
    /// Both supported provider formats require `name` and `description` for discovery.
    pub(crate) fn validate_identity(
        &self,
        expected_capability: &str,
        definition_kind: &str,
    ) -> Result<(), String> {
        let name = self
            .name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("{definition_kind} must declare a non-empty name field"))?;
        if name != expected_capability {
            return Err(format!(
                "{definition_kind} name '{name}' does not match requested capability '{expected_capability}'"
            ));
        }
        if self.description.as_deref().is_none_or(|value| value.trim().is_empty()) {
            return Err(format!("{definition_kind} must declare a non-empty description field"));
        }
        Ok(())
    }

    pub(crate) fn sandbox(&self) -> Result<Option<&str>, String> {
        match &self.sandbox {
            SandboxDeclaration::Absent => Ok(None),
            SandboxDeclaration::Declared(value) if value.trim().is_empty() => {
                Err("Codex skill sandbox declaration must not be empty".to_owned())
            }
            SandboxDeclaration::Declared(value) => Ok(Some(value)),
        }
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_deref().filter(|value| !value.trim().is_empty())
    }

    /// Returns the explicitly declared Claude tool surface.
    pub(crate) fn tools(&self) -> Option<Vec<&str>> {
        match self.tools.as_ref() {
            Some(ToolsDeclaration::Scalar(value)) if !value.trim().is_empty() => {
                Some(vec![value.as_str()])
            }
            Some(ToolsDeclaration::List(values))
                if !values.is_empty() && values.iter().all(|value| !value.trim().is_empty()) =>
            {
                Some(values.iter().map(String::as_str).collect())
            }
            Some(ToolsDeclaration::Scalar(_)) | Some(ToolsDeclaration::List(_)) | None => None,
        }
    }
}

pub(crate) fn parse_provider_definition_front_matter(
    front_matter: &str,
) -> Result<ProviderDefinitionFrontMatter, String> {
    serde_yaml::from_str(front_matter)
        .map_err(|error| format!("invalid YAML front matter: {error}"))
}
