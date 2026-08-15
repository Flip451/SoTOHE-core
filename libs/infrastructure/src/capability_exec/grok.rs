//! Grok provider-native capability-definition discovery and admission.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{self, Visitor};
use usecase::capability_exec::ModelName;

use crate::grok_common::GrokSandbox;

use super::path_guard::capability_name_path_segment;
use super::{parse_provider_definition_front_matter, read_front_matter, read_utf8_file};

const SHARED_CAPABILITY_DEFINITION_ROOT: &str = ".agents/skills";
const SHARED_CAPABILITY_DEFINITION_FILE: &str = "SKILL.md";

/// The Grok-specific fields declared by a shared capability adapter definition.
///
/// The definition itself remains in the shared `.agents/` surface. Grok reads
/// only its provider-specific `grok-sandbox` permission and the optional model
/// projection; the shared name and description are validated before this DTO
/// is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrokCapabilityDefinition {
    /// Optional model projection declared on the shared adapter definition.
    pub model: Option<ModelName>,
    /// Optional Grok sandbox declared as `grok-sandbox`.
    pub sandbox: Option<GrokSandbox>,
}

impl GrokCapabilityDefinition {
    /// Returns the optional model projection declared by the adapter definition.
    #[must_use]
    pub fn model(&self) -> Option<&ModelName> {
        self.model.as_ref()
    }

    /// Returns the optional Grok sandbox declaration.
    ///
    /// `None` is intentionally preserved for diagnostic resolution. Dispatch
    /// admission must call the admission helper and reject that state instead
    /// of treating the diagnostic fallback as permission.
    #[must_use]
    pub fn sandbox(&self) -> Option<&GrokSandbox> {
        self.sandbox.as_ref()
    }
}

#[derive(Debug, Deserialize)]
struct GrokCapabilityDefinitionFields {
    #[serde(default)]
    model: ModelDeclaration,
    #[serde(default, rename = "grok-sandbox")]
    sandbox: Option<GrokSandbox>,
}

#[derive(Debug, Default)]
enum ModelDeclaration {
    #[default]
    Absent,
    Declared(String),
}

impl<'de> Deserialize<'de> for ModelDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ModelDeclarationVisitor;

        impl Visitor<'_> for ModelDeclarationVisitor {
            type Value = ModelDeclaration;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a non-null model declaration string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ModelDeclaration::Declared(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(ModelDeclaration::Declared(value))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Err(E::custom("model declaration must not be null"))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Err(E::custom("model declaration must not be null"))
            }
        }

        deserializer.deserialize_any(ModelDeclarationVisitor)
    }
}

impl<'de> Deserialize<'de> for GrokCapabilityDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fields = GrokCapabilityDefinitionFields::deserialize(deserializer)?;
        let model = match fields.model {
            ModelDeclaration::Absent => None,
            ModelDeclaration::Declared(value) => {
                Some(ModelName::try_new(value).map_err(serde::de::Error::custom)?)
            }
        };
        Ok(Self { model, sandbox: fields.sandbox })
    }
}

/// Discovers and admits a Grok capability definition from the shared adapter surface.
///
/// The shared definition must identify `capability` and declare a valid
/// `grok-sandbox`. A missing definition, malformed front matter, identity
/// mismatch, missing permission, unsupported permission, or a declared model
/// that differs from `profile_model` is rejected before any provider process
/// can be started. An omitted definition model is resolved from `profile_model`.
///
/// # Errors
///
/// Returns a diagnostic string describing the fail-closed discovery or
/// admission failure.
#[allow(dead_code)]
pub(crate) fn resolve_grok_capability_definition(
    repo_root: &Path,
    capability: &str,
    profile_model: &ModelName,
) -> Result<GrokCapabilityDefinition, String> {
    let definition = discover_grok_capability_definition(repo_root, capability)?;
    admit_grok_capability_definition(definition, profile_model)
}

/// Reads and parses a shared Grok adapter definition without applying dispatch admission.
///
/// This deliberately keeps an absent `grok-sandbox` as `None`: diagnostics may
/// resolve that absence to read-only, but dispatch must use the admission helper
/// to reject it.
///
/// # Errors
///
/// Returns a diagnostic string when the capability name is not a safe path
/// segment, the shared definition cannot be read, its front matter is invalid,
/// its identity does not match, or its Grok fields cannot be decoded.
fn discover_grok_capability_definition(
    repo_root: &Path,
    capability: &str,
) -> Result<GrokCapabilityDefinition, String> {
    let capability = capability_name_path_segment(capability)?;
    let path = shared_capability_definition_path(repo_root, capability);
    let definition = read_utf8_file(&path, repo_root)?;
    grok_capability_definition_from_definition(&definition, capability)
}

fn shared_capability_definition_path(repo_root: &Path, capability: &str) -> PathBuf {
    repo_root
        .join(SHARED_CAPABILITY_DEFINITION_ROOT)
        .join(capability)
        .join(SHARED_CAPABILITY_DEFINITION_FILE)
}

fn grok_capability_definition_from_definition(
    definition: &str,
    expected_capability: &str,
) -> Result<GrokCapabilityDefinition, String> {
    let front_matter = read_front_matter(definition)?
        .ok_or_else(|| "Grok capability definition has no YAML front matter".to_owned())?;
    let shared_front_matter = parse_provider_definition_front_matter(front_matter)?;
    shared_front_matter.validate_identity(expected_capability, "Grok capability definition")?;
    serde_yaml::from_str(front_matter)
        .map_err(|error| format!("invalid Grok capability definition YAML: {error}"))
}

fn admit_grok_capability_definition(
    definition: GrokCapabilityDefinition,
    profile_model: &ModelName,
) -> Result<GrokCapabilityDefinition, String> {
    if definition.sandbox.is_none() {
        return Err(
            "Grok capability definition must declare a non-empty grok-sandbox field".to_owned()
        );
    }
    if let Some(declared_model) = definition.model.as_ref()
        && declared_model != profile_model
    {
        return Err(format!(
            "Grok capability definition model '{}' does not match profile model '{}'",
            declared_model.as_str(),
            profile_model.as_str()
        ));
    }
    Ok(definition)
}

#[allow(dead_code)]
fn resolve_grok_sandbox_for_diagnosis(definition: &GrokCapabilityDefinition) -> GrokSandbox {
    definition.sandbox.clone().unwrap_or(GrokSandbox::ReadOnly)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;

    use super::{
        admit_grok_capability_definition, discover_grok_capability_definition,
        resolve_grok_capability_definition, resolve_grok_sandbox_for_diagnosis,
    };
    use crate::grok_common::GrokSandbox;
    use usecase::capability_exec::ModelName;

    fn write_shared_adapter(
        root: &std::path::Path,
        capability: &str,
        grok_sandbox: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model_line = model.map(|value| format!("model: \"{value}\""));
        write_shared_adapter_with_model_line(root, capability, grok_sandbox, model_line.as_deref())
    }

    fn write_shared_adapter_with_model_line(
        root: &std::path::Path,
        capability: &str,
        grok_sandbox: Option<&str>,
        model_line: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = root.join(".agents/skills").join(capability);
        fs::create_dir_all(&directory)?;
        let sandbox =
            grok_sandbox.map_or_else(String::new, |value| format!("grok-sandbox: {value}\n"));
        let model = model_line.map_or_else(String::new, |line| format!("{line}\n"));
        let definition = format!(
            "---\nname: {capability}\ndescription: Shared adapter fixture.\nsandbox: workspace-write\n{sandbox}{model}---\nshared capability body\n"
        );
        fs::write(directory.join("SKILL.md"), definition)?;
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_valid_shared_adapter_with_optional_model_is_admitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let profile_model = ModelName::try_new("gpt-5")?;
        for model in [Some("gpt-5"), None] {
            let directory = tempfile::tempdir()?;
            write_shared_adapter(directory.path(), "implementer", Some("workspace"), model)?;

            let definition = resolve_grok_capability_definition(
                directory.path(),
                "implementer",
                &profile_model,
            )?;

            assert_eq!(definition.model().map(|value| value.as_str()), model);
            assert_eq!(definition.sandbox(), Some(&GrokSandbox::Workspace));
            assert!(admit_grok_capability_definition(definition, &profile_model).is_ok());
        }
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_missing_shared_adapter_is_rejected() {
        let directory = tempfile::tempdir().expect("test repository is created");
        let profile_model = ModelName::try_new("gpt-5").expect("profile model is valid");

        let error =
            resolve_grok_capability_definition(directory.path(), "implementer", &profile_model)
                .expect_err("missing shared adapter is rejected");

        assert!(error.contains("cannot"));
    }

    #[test]
    fn test_grok_capability_definition_missing_grok_sandbox_is_diagnostic_only()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_model = ModelName::try_new("gpt-5")?;
        write_shared_adapter(directory.path(), "implementer", None, None)?;

        let definition = discover_grok_capability_definition(directory.path(), "implementer")?;

        assert_eq!(definition.sandbox(), None);
        assert_eq!(resolve_grok_sandbox_for_diagnosis(&definition), GrokSandbox::ReadOnly);
        let error = admit_grok_capability_definition(definition, &profile_model)
            .expect_err("diagnostic fallback must not admit dispatch");
        assert!(error.contains("grok-sandbox"));
        assert!(
            resolve_grok_capability_definition(directory.path(), "implementer", &profile_model)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_empty_or_invalid_model_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        for model_line in ["model: \"\"", "model: \"   \"", "model: []"] {
            let directory = tempfile::tempdir()?;
            write_shared_adapter_with_model_line(
                directory.path(),
                "implementer",
                Some("workspace"),
                Some(model_line),
            )?;

            let error = discover_grok_capability_definition(directory.path(), "implementer")
                .expect_err("empty or invalid model declaration is rejected during parsing");

            assert!(error.contains("model"), "unexpected model error: {error}");
        }
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_declared_model_matching_profile_projection_is_admitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_model = ModelName::try_new("gpt-5")?;
        write_shared_adapter(directory.path(), "implementer", Some("workspace"), Some("gpt-5"))?;

        let definition =
            resolve_grok_capability_definition(directory.path(), "implementer", &profile_model)?;

        assert_eq!(definition.model(), Some(&profile_model));
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_declared_model_mismatching_profile_projection_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_model = ModelName::try_new("gpt-5")?;
        write_shared_adapter(
            directory.path(),
            "implementer",
            Some("workspace"),
            Some("gpt-5-mini"),
        )?;

        let error =
            resolve_grok_capability_definition(directory.path(), "implementer", &profile_model)
                .expect_err("declared model must match the profile projection");

        assert!(error.contains("does not match profile model"));
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_off_sandbox_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_model = ModelName::try_new("gpt-5")?;
        write_shared_adapter(directory.path(), "implementer", Some("off"), None)?;

        let error =
            resolve_grok_capability_definition(directory.path(), "implementer", &profile_model)
                .expect_err("unrestricted sandbox is rejected");

        assert!(error.contains("reserved sandbox value"));
        Ok(())
    }

    #[test]
    fn test_grok_capability_definition_workspace_write_sandbox_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profile_model = ModelName::try_new("gpt-5")?;
        write_shared_adapter(directory.path(), "implementer", Some("workspace-write"), None)?;

        let error =
            resolve_grok_capability_definition(directory.path(), "implementer", &profile_model)
                .expect_err("Codex sandbox vocabulary is rejected");

        assert!(error.contains("reserved sandbox value"));
        Ok(())
    }
}
