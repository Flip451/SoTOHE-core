//! Grok provider-native capability-definition discovery and admission.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use domain::TrackId;
use serde::Deserialize;
use serde::de::{self, Visitor};
use usecase::capability_exec::{
    CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
    CapabilityFailureDetail, CapabilityProviderPort, GROK_PROVIDER_NAME, ModelName, ProviderName,
    ReasoningEffort,
};
use usecase::provider_session::ProviderSessionCachePort;

use crate::grok_common::{GrokOutputEnvelope, GrokSandbox};

use super::path_guard::capability_name_path_segment;
use super::process::ProviderProcessOutput;
use super::session::CapabilitySession;
use super::{
    ProviderProcessRunner, adapter_preflight_error, capability_prompt, dispatch_error,
    parse_provider_definition_front_matter, read_front_matter, read_utf8_file,
    system_process_runner,
};

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

    /// Discovers and admits the shared Grok adapter definition for `capability`.
    ///
    /// # Errors
    ///
    /// Returns an opaque diagnostic when discovery or admission fails closed.
    pub fn resolve(
        repo_root: &std::path::Path,
        capability: &str,
        profile_model: &ModelName,
    ) -> Result<Self, CapabilityFailureDetail> {
        resolve_grok_capability_definition(repo_root, capability, profile_model)
            .map_err(CapabilityFailureDetail::new)
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

/// Dispatches an orchestrator-output capability through an isolated Grok subprocess.
pub struct GrokCapabilityAdapter {
    repo_root: PathBuf,
    runtime_dir: PathBuf,
    provider: ProviderName,
    process_runner: Arc<dyn ProviderProcessRunner>,
    session_cache: Arc<dyn ProviderSessionCachePort>,
    track_id: Option<TrackId>,
}

impl GrokCapabilityAdapter {
    /// Creates a Grok adapter rooted at `repo_root` with logs under `runtime_dir`.
    #[must_use]
    pub fn new(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        session_cache: Arc<dyn ProviderSessionCachePort>,
        track_id: Option<TrackId>,
    ) -> GrokCapabilityAdapter {
        Self {
            repo_root,
            runtime_dir,
            provider: GROK_PROVIDER_NAME.clone(),
            process_runner: system_process_runner(),
            session_cache,
            track_id,
        }
    }

    #[cfg(test)]
    fn with_process_runner(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        process_runner: Arc<dyn ProviderProcessRunner>,
    ) -> Self {
        let session_cache = Arc::new(crate::provider_session::FsProviderSessionCacheAdapter::new(
            repo_root.clone(),
            runtime_dir.clone(),
        ));
        Self::with_process_runner_and_session_cache(
            repo_root,
            runtime_dir,
            process_runner,
            session_cache,
            None,
        )
    }

    #[cfg(test)]
    fn with_process_runner_and_session_cache(
        repo_root: PathBuf,
        runtime_dir: PathBuf,
        process_runner: Arc<dyn ProviderProcessRunner>,
        session_cache: Arc<dyn ProviderSessionCachePort>,
        track_id: Option<TrackId>,
    ) -> Self {
        Self {
            repo_root,
            runtime_dir,
            provider: GROK_PROVIDER_NAME.clone(),
            process_runner,
            session_cache,
            track_id,
        }
    }

    fn run_process(
        &self,
        args: &[OsString],
        timeout: Option<Duration>,
    ) -> Result<ProviderProcessOutput, CapabilityExecError> {
        self.process_runner
            .run(
                "grok",
                None,
                args,
                &self.repo_root,
                &self.runtime_dir,
                &self.provider,
                timeout,
                None,
            )
            .map_err(|error| match error {
                CapabilityExecError::DispatchFailed { .. } => error,
                other => dispatch_error(&self.provider, other.to_string()),
            })
    }

    fn dispatch_with_stdout(
        &self,
        request: &CapabilityDispatchRequest,
        passthrough: &mut impl Write,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        let definition = resolve_grok_capability_definition(
            &self.repo_root,
            request.request.capability.as_str(),
            &request.profile.model,
        )
        .map_err(|detail| adapter_preflight_error(request, &self.provider, detail))?;
        let sandbox = definition.sandbox.ok_or_else(|| {
            adapter_preflight_error(
                request,
                &self.provider,
                "Grok capability definition did not resolve a grok-sandbox permission",
            )
        })?;
        let prompt = capability_prompt(request);
        let session =
            CapabilitySession::new(request, self.track_id.as_ref(), self.session_cache.clone())
                .map_err(|error| {
                    adapter_preflight_error(request, &self.provider, error.to_string())
                })?;
        let resume_id = session.resumable_id(&request.request.resume);
        let timeout = request.request.timeout.map(|timeout| Duration::from_secs(timeout.as_secs()));
        let initial_args = build_grok_args(
            request.profile.model.as_str(),
            request.profile.effort,
            &sandbox,
            resume_id.as_deref(),
            &prompt,
        );
        let initial_result = self.run_process(&initial_args, timeout);
        let output = match (resume_id.as_deref(), initial_result) {
            (Some(_), Ok(output)) if resume_attempt_needs_fresh_session(&output) => {
                let fresh_args = build_grok_args(
                    request.profile.model.as_str(),
                    request.profile.effort,
                    &sandbox,
                    None,
                    &prompt,
                );
                self.run_process(&fresh_args, timeout)?
            }
            (Some(_), Err(_)) => {
                let fresh_args = build_grok_args(
                    request.profile.model.as_str(),
                    request.profile.effort,
                    &sandbox,
                    None,
                    &prompt,
                );
                self.run_process(&fresh_args, timeout)?
            }
            (_, Ok(output)) => output,
            (_, Err(error)) => return Err(error),
        };
        let structured_output = structured_output_from_process(&output, &self.provider)?;
        emit_capability_result_text(&structured_output, &self.provider, passthrough)?;
        if output.exit_code == 0 {
            session.save(output.session_id);
        }
        Ok(CapabilityDispatchOutcome::Executed {
            provider: self.provider.clone(),
            exit_code: output.exit_code,
        })
    }
}

impl CapabilityProviderPort for GrokCapabilityAdapter {
    fn provider(&self) -> &ProviderName {
        &self.provider
    }

    fn dispatch(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        self.dispatch_with_stdout(request, &mut std::io::stdout())
    }
}

/// Schema for generic orchestrator-output capabilities: a required free-form result text.
pub(crate) const GROK_STRUCTURED_OUTPUT_SCHEMA: &str =
    r#"{"type":"object","properties":{"result":{"type":"string"}},"required":["result"]}"#;

pub(crate) fn build_grok_args(
    model: &str,
    effort: ReasoningEffort,
    sandbox: &GrokSandbox,
    resume_id: Option<&str>,
    prompt: &str,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-p"),
        OsString::from(prompt),
        OsString::from("--model"),
        OsString::from(model),
        OsString::from("--reasoning-effort"),
        OsString::from(reasoning_effort_value(effort)),
        OsString::from("--sandbox"),
        OsString::from(grok_sandbox_value(sandbox)),
        OsString::from("--output-format"),
        OsString::from("json"),
        OsString::from("--json-schema"),
        OsString::from(GROK_STRUCTURED_OUTPUT_SCHEMA),
    ];
    if let Some(resume_id) = resume_id {
        args.extend([OsString::from("--resume"), OsString::from(resume_id)]);
    }
    args
}

fn grok_sandbox_value(sandbox: &GrokSandbox) -> &str {
    match sandbox {
        GrokSandbox::ReadOnly => "read-only",
        GrokSandbox::Workspace => "workspace",
        GrokSandbox::Strict => "strict",
        GrokSandbox::ProjectProfile(profile) => profile.as_str(),
    }
}

fn reasoning_effort_value(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}

fn resume_attempt_needs_fresh_session(output: &ProviderProcessOutput) -> bool {
    if output.exit_code != 0 {
        return true;
    }
    !matches!(
        output
            .final_message
            .as_deref()
            .and_then(|message| serde_json::from_slice::<GrokOutputEnvelope>(message).ok()),
        Some(GrokOutputEnvelope::Succeeded { .. })
    )
}

fn structured_output_from_process(
    output: &ProviderProcessOutput,
    provider: &ProviderName,
) -> Result<serde_json::Value, CapabilityExecError> {
    let message = output.final_message.as_deref().ok_or_else(|| {
        dispatch_error(provider, "Grok provider returned no structured-output envelope")
    })?;
    let envelope = serde_json::from_slice::<GrokOutputEnvelope>(message).map_err(|error| {
        dispatch_error(provider, format!("cannot decode Grok output envelope: {error}"))
    })?;
    envelope.into_structured_output().map_err(|error| dispatch_error(provider, error.to_string()))
}

fn emit_capability_result_text(
    structured_output: &serde_json::Value,
    provider: &ProviderName,
    passthrough: &mut impl Write,
) -> Result<(), CapabilityExecError> {
    let result =
        structured_output.get("result").and_then(serde_json::Value::as_str).ok_or_else(|| {
            dispatch_error(provider, "Grok structured output is missing a string result field")
        })?;
    passthrough.write_all(result.as_bytes()).map_err(|error| {
        dispatch_error(provider, format!("cannot write Grok structured output: {error}"))
    })?;
    passthrough.flush().map_err(|error| {
        dispatch_error(provider, format!("cannot flush Grok structured output: {error}"))
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::super::process::{ProviderProcessOutput, ProviderProcessRunner};
    use super::{
        GROK_STRUCTURED_OUTPUT_SCHEMA, GrokCapabilityAdapter, admit_grok_capability_definition,
        discover_grok_capability_definition, resolve_grok_capability_definition,
        resolve_grok_sandbox_for_diagnosis,
    };
    use crate::capability_exec::agent_profiles::AgentProfilesCapabilityAdapter;
    use crate::grok_common::GrokSandbox;
    use domain::TrackId;
    use usecase::capability_exec::{
        BriefingText, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
        CapabilityExecInteractor, CapabilityExecRequest, CapabilityExecService,
        CapabilityFailureDetail, CapabilityFilePath, CapabilityProfile, CapabilityProviderBinding,
        CapabilityProviderPort, CapabilityResumeRequest, CapabilitySourcePort, DisciplineText,
        ExecutionMode, GROK_PROVIDER_NAME, ModelName, ProviderName, ReasoningEffort,
        TargetArtifactPath, TargetArtifactSet,
    };
    use usecase::conventions_resolve::{
        ConventionResolution, ConventionResolveError, ConventionResolveService,
        ResolveConventionsQuery,
    };
    use usecase::dry_write_driver::CapabilityName;
    use usecase::provider_session::{
        ProviderSessionCacheEntry, ProviderSessionCacheError, ProviderSessionCacheKey,
        ProviderSessionCachePort, ProviderSessionId,
    };

    type RecordedInvocation = (String, Vec<OsString>, Option<Duration>);

    #[derive(Default)]
    struct RecordingProcessRunner {
        invocations: Mutex<Vec<RecordedInvocation>>,
        responses: Mutex<Vec<Result<ProviderProcessOutput, CapabilityExecError>>>,
    }

    impl ProviderProcessRunner for RecordingProcessRunner {
        fn run(
            &self,
            binary: &str,
            _path_prefix: Option<&Path>,
            args: &[OsString],
            _repo_root: &Path,
            _runtime_dir: &Path,
            _provider: &ProviderName,
            timeout: Option<Duration>,
            _output_last_message: Option<&Path>,
        ) -> Result<ProviderProcessOutput, CapabilityExecError> {
            self.invocations.lock().expect("process recorder lock").push((
                binary.to_owned(),
                args.to_vec(),
                timeout,
            ));
            self.responses
                .lock()
                .expect("process response lock")
                .pop()
                .unwrap_or_else(|| Ok(successful_process_output()))
        }
    }

    #[derive(Default)]
    struct MemorySessionCache {
        entries: Mutex<HashMap<ProviderSessionCacheKey, ProviderSessionCacheEntry>>,
    }

    impl ProviderSessionCachePort for MemorySessionCache {
        fn load(
            &self,
            key: &ProviderSessionCacheKey,
        ) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError> {
            Ok(self.entries.lock().expect("session cache lock").get(key).cloned())
        }

        fn save(
            &self,
            key: &ProviderSessionCacheKey,
            entry: &ProviderSessionCacheEntry,
        ) -> Result<(), ProviderSessionCacheError> {
            self.entries.lock().expect("session cache lock").insert(key.clone(), entry.clone());
            Ok(())
        }

        fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> {
            self.entries.lock().expect("session cache lock").remove(key);
            Ok(())
        }
    }

    fn successful_process_output() -> ProviderProcessOutput {
        ProviderProcessOutput {
            exit_code: 0,
            session_id: Some("new-grok-session".to_owned()),
            final_message: Some(br#"{"structured_output":{"result":"ok"}}"#.to_vec()),
        }
    }

    fn request(
        resume: CapabilityResumeRequest,
    ) -> Result<CapabilityDispatchRequest, Box<dyn std::error::Error>> {
        Ok(CapabilityDispatchRequest {
            request: CapabilityExecRequest {
                capability: CapabilityName::try_new("implementer")?,
                host: Some(GROK_PROVIDER_NAME.clone()),
                briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                timeout: None,
                resume,
            },
            profile: CapabilityProfile {
                provider: CapabilityProviderBinding::Standard(GROK_PROVIDER_NAME.clone()),
                model: ModelName::try_new("grok-4")?,
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            briefing: BriefingText::try_new("briefing".to_owned())?,
            discipline: DisciplineText::try_new("discipline".to_owned())?,
        })
    }

    fn write_grok_adapter(
        root: &Path,
        model: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = root.join(".agents/skills/implementer");
        fs::create_dir_all(&directory)?;
        let model_line = model.map_or_else(String::new, |value| format!("model: {value}\n"));
        fs::write(
            directory.join("SKILL.md"),
            format!(
                "---\nname: implementer\ndescription: Shared adapter fixture.\ngrok-sandbox: workspace\n{model_line}---\nshared body\n"
            ),
        )?;
        Ok(())
    }

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

    fn assert_explicit_grok_settings(args: &[OsString], sandbox: &str) {
        assert!(args.windows(2).any(|pair| pair == ["--model", "grok-4"]));
        assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", sandbox]));
        assert!(args.windows(2).any(|pair| pair == ["--output-format", "json"]));
        assert!(
            args.windows(2).any(|pair| pair == ["--json-schema", GROK_STRUCTURED_OUTPUT_SCHEMA])
        );
    }

    struct StaticCapabilitySource;

    impl CapabilitySourcePort for StaticCapabilitySource {
        fn load_briefing(
            &self,
            _path: &CapabilityFilePath,
        ) -> Result<BriefingText, CapabilityExecError> {
            Ok(BriefingText::try_new("briefing".to_owned()).expect("test briefing is valid"))
        }

        fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError> {
            Ok(DisciplineText::try_new("discipline".to_owned()).expect("test discipline is valid"))
        }
    }

    struct EmptyConventionResolver;

    impl ConventionResolveService for EmptyConventionResolver {
        fn resolve(
            &self,
            _query: ResolveConventionsQuery,
        ) -> Result<ConventionResolution, ConventionResolveError> {
            Ok(ConventionResolution::default())
        }
    }

    fn write_agent_profiles(
        root: &Path,
        capabilities: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = root.join("agent-profiles.json");
        let content = format!(
            r#"{{
  "schema_version": 1,
  "providers": {{
    "grok": {{
      "label": "Grok CLI",
      "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"]
    }}
  }},
  "capabilities": {capabilities}
}}"#
        );
        fs::write(&path, content)?;
        Ok(path)
    }

    fn capability_exec_request(
        capability: &str,
    ) -> Result<CapabilityExecRequest, Box<dyn std::error::Error>> {
        Ok(CapabilityExecRequest {
            capability: CapabilityName::try_new(capability)?,
            host: Some(GROK_PROVIDER_NAME.clone()),
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            timeout: None,
            resume: CapabilityResumeRequest::Fresh,
        })
    }

    fn capability_interactor(
        root: &Path,
        profiles_path: &Path,
        runner: Arc<RecordingProcessRunner>,
    ) -> CapabilityExecInteractor {
        let adapter = GrokCapabilityAdapter::with_process_runner(
            root.to_owned(),
            root.join("runtime"),
            runner,
        );
        CapabilityExecInteractor::new(
            Arc::new(AgentProfilesCapabilityAdapter::new(
                root.to_owned(),
                profiles_path.to_owned(),
            )),
            Arc::new(StaticCapabilitySource),
            Arc::new(EmptyConventionResolver),
            vec![Arc::new(adapter)],
            root.to_owned(),
        )
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

    #[test]
    fn test_grok_capability_adapter_subprocess_launch_with_explicit_model_and_effort()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut stdout = Vec::new();

        let outcome =
            adapter.dispatch_with_stdout(&request(CapabilityResumeRequest::Fresh)?, &mut stdout)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 0 }
                if provider.as_str() == "grok"
        ));
        assert_eq!(adapter.provider().as_str(), "grok");
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let (binary, args, timeout) = invocations.first().expect("fresh invocation recorded");
        assert_eq!(binary, "grok");
        assert_eq!(*timeout, None);
        assert!(args.windows(2).any(|pair| pair == ["--model", "grok-4"]));
        assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", "high"]));
        assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]));
        assert!(args.windows(2).any(|pair| pair == ["--output-format", "json"]));
        assert!(
            args.windows(2).any(|pair| pair == ["--json-schema", GROK_STRUCTURED_OUTPUT_SCHEMA])
        );
        assert!(!args.iter().any(|arg| arg == "agent" || arg == "--leader"));
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_dispatch_accepts_briefing_file_in_prompt()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(&request(CapabilityResumeRequest::Fresh)?, &mut stdout)?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        let args = &invocations.first().expect("fresh invocation recorded").1;
        let prompt = args.windows(2).find_map(|pair| match pair {
            [flag, value] if flag == "-p" => Some(value.to_string_lossy().into_owned()),
            _ => None,
        });
        let prompt = prompt.expect("dispatch must pass -p");
        assert!(
            prompt.contains("briefing"),
            "dispatch prompt must include the accepted briefing file contents: {prompt:?}"
        );
        Ok(())
    }

    #[test]
    fn test_grok_capability_provider_dispatch_uses_profile_capability_name_shared_definition_and_valid_sandbox()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let capability = "reviewer-fixture";
        write_shared_adapter(directory.path(), capability, Some("strict"), None)?;
        let mut dispatch_request = request(CapabilityResumeRequest::Fresh)?;
        dispatch_request.request.capability = CapabilityName::try_new(capability)?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let outcome = CapabilityProviderPort::dispatch(&adapter, &dispatch_request)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 0 }
                if provider.as_str() == "grok"
        ));
        assert_eq!(adapter.provider(), &*GROK_PROVIDER_NAME);
        let shared_definition =
            directory.path().join(".agents/skills").join(capability).join("SKILL.md");
        assert!(shared_definition.is_file());
        assert!(!directory.path().join(".grok").exists());
        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let (binary, args, timeout) = invocations.first().expect("trait dispatch recorded");
        assert_eq!(binary, "grok");
        assert_eq!(*timeout, None);
        assert_explicit_grok_settings(args, "strict");
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_dispatch_missing_shared_definition_fails_closed_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let error =
            CapabilityProviderPort::dispatch(&adapter, &request(CapabilityResumeRequest::Fresh)?)
                .expect_err("missing shared adapter must stop dispatch before spawning Grok");

        assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
        assert!(error.to_string().contains("cannot"));
        assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_dispatch_undeclared_sandbox_fails_closed_after_read_only_diagnosis()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_shared_adapter(directory.path(), "implementer", None, None)?;
        let definition = discover_grok_capability_definition(directory.path(), "implementer")?;
        assert_eq!(definition.sandbox(), None);
        assert_eq!(resolve_grok_sandbox_for_diagnosis(&definition), GrokSandbox::ReadOnly);

        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );
        let error =
            CapabilityProviderPort::dispatch(&adapter, &request(CapabilityResumeRequest::Fresh)?)
                .expect_err("diagnostic read-only fallback must not authorize dispatch");

        assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
        assert!(error.to_string().contains("grok-sandbox"));
        assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_dispatch_reserved_sandbox_values_fail_closed_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        for sandbox in ["off", "workspace-write"] {
            let directory = tempfile::tempdir()?;
            write_shared_adapter(directory.path(), "implementer", Some(sandbox), None)?;
            let runner = Arc::new(RecordingProcessRunner::default());
            let adapter = GrokCapabilityAdapter::with_process_runner(
                directory.path().to_owned(),
                directory.path().join("runtime"),
                runner.clone(),
            );

            let error = CapabilityProviderPort::dispatch(
                &adapter,
                &request(CapabilityResumeRequest::Fresh)?,
            )
            .expect_err("reserved sandbox must stop dispatch before spawning Grok");

            assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
            assert!(error.to_string().contains("reserved sandbox value"));
            assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        }
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_dispatch_invalid_declared_model_fails_closed_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_shared_adapter_with_model_line(
            directory.path(),
            "implementer",
            Some("workspace"),
            Some("model: \"\""),
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let error =
            CapabilityProviderPort::dispatch(&adapter, &request(CapabilityResumeRequest::Fresh)?)
                .expect_err("invalid model projection must stop dispatch before spawning Grok");

        assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
        assert!(error.to_string().contains("model"));
        assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_dispatch_mismatched_declared_model_fails_closed_before_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_shared_adapter(
            directory.path(),
            "implementer",
            Some("workspace"),
            Some("grok-4-mini"),
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
        );

        let error =
            CapabilityProviderPort::dispatch(&adapter, &request(CapabilityResumeRequest::Fresh)?)
                .expect_err("mismatched model projection must stop dispatch before spawning Grok");

        assert!(matches!(error, CapabilityExecError::AdapterPreflight { .. }));
        assert!(error.to_string().contains("does not match profile model"));
        assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_unresolved_effort_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profiles_path = write_agent_profiles(
            directory.path(),
            r#"{
    "implementer": {
      "provider": "grok",
      "model": "grok-4",
      "execution_mode": "orchestrator-output"
    }
  }"#,
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let interactor = capability_interactor(directory.path(), &profiles_path, runner.clone());

        let error = interactor
            .execute(capability_exec_request("implementer")?)
            .expect_err("an unresolved profile effort must reject dispatch");

        assert!(
            matches!(error, CapabilityExecError::EffortMissing(capability) if capability.as_str() == "implementer")
        );
        assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_absent_profile_capability_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let profiles_path = write_agent_profiles(directory.path(), "{}")?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let interactor = capability_interactor(directory.path(), &profiles_path, runner.clone());

        let error = interactor
            .execute(capability_exec_request("absent-profile-capability")?)
            .expect_err("a capability absent from the profile must reject dispatch");

        assert!(matches!(
            error,
            CapabilityExecError::ProfileResolution { ref capability, .. }
                if capability.as_str() == "absent-profile-capability"
        ));
        assert!(error.to_string().contains("not declared in agent-profiles.json"));
        assert!(runner.invocations.lock().expect("process recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_structured_envelope_success_extracts_structured_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(
                    br#"{"structured_output":{"result":"from-structured-output"},"text":"ignore-me"}"#
                        .to_vec(),
                ),
            })]),
            ..Default::default()
        });
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner,
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(&request(CapabilityResumeRequest::Fresh)?, &mut stdout)?;

        assert_eq!(stdout, b"from-structured-output");
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_missing_structured_output_fails_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(br#"{"text":"not-a-return-channel"}"#.to_vec()),
            })]),
            ..Default::default()
        });
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner,
        );
        let mut stdout = Vec::new();

        let error = adapter
            .dispatch_with_stdout(&request(CapabilityResumeRequest::Fresh)?, &mut stdout)
            .expect_err("missing structured output must fail closed");

        assert!(error.to_string().contains("structured output is missing"));
        assert!(stdout.is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_missing_structured_output_reports_envelope_failure_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![Ok(ProviderProcessOutput {
                exit_code: 0,
                session_id: None,
                final_message: Some(
                    br#"{"failure_reason":"provider declined structured output","text":"ignore-me"}"#
                        .to_vec(),
                ),
            })]),
            ..Default::default()
        });
        let adapter = GrokCapabilityAdapter::with_process_runner(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner,
        );
        let mut stdout = Vec::new();

        let error = adapter
            .dispatch_with_stdout(&request(CapabilityResumeRequest::Fresh)?, &mut stdout)
            .expect_err("provider failure envelope must fail dispatch");

        assert!(error.to_string().contains("provider declined structured output"));
        assert!(!error.to_string().contains("ignore-me"));
        assert!(stdout.is_empty());
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_resume_failure_starts_new_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let track_id = TrackId::try_new("track-a")?;
        let cache = Arc::new(MemorySessionCache::default());
        let request = request(CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
            TargetArtifactPath::try_new(PathBuf::from("track/items/a/spec.json"))?,
        ])?))?;
        let cache_key = ProviderSessionCacheKey::TrackCapability {
            track_id: track_id.clone(),
            capability: request.request.capability.clone(),
        };
        cache.save(
            &cache_key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("expired-session".to_owned())?,
                GROK_PROVIDER_NAME.clone(),
                request.profile.model.clone(),
                request.profile.effort,
            ),
        )?;
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![
                Ok(successful_process_output()),
                Ok(ProviderProcessOutput {
                    exit_code: 0,
                    session_id: None,
                    final_message: Some(br#"{"failure_reason":"resume session expired"}"#.to_vec()),
                }),
            ]),
            ..Default::default()
        });
        let adapter = GrokCapabilityAdapter::with_process_runner_and_session_cache(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
            cache,
            Some(track_id),
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(&request, &mut stdout)?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 2);
        let first_args = &invocations.first().expect("resume invocation recorded").1;
        let second_args = &invocations.get(1).expect("fresh fallback recorded").1;
        assert!(first_args.windows(2).any(|pair| pair == ["--resume", "expired-session"]));
        assert!(!second_args.iter().any(|arg| arg == "--resume"));
        for args in [first_args, second_args] {
            assert!(args.windows(2).any(|pair| pair == ["--model", "grok-4"]));
            assert!(args.windows(2).any(|pair| pair == ["--reasoning-effort", "high"]));
            assert!(args.windows(2).any(|pair| pair == ["--sandbox", "workspace"]));
        }
        assert_eq!(stdout, b"ok");
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_resume_unavailable_starts_fresh_session_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let track_id = TrackId::try_new("track-unavailable")?;
        let cache = Arc::new(MemorySessionCache::default());
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner_and_session_cache(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
            cache,
            Some(track_id),
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(
            &request(CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("track/items/unavailable/spec.json"))?,
            ])?))?,
            &mut stdout,
        )?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let args = &invocations.first().expect("fresh invocation recorded").1;
        assert!(!args.iter().any(|arg| arg == "--resume"));
        assert_explicit_grok_settings(args, "workspace");
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_failed_resume_starts_fresh_session_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let track_id = TrackId::try_new("track-failed")?;
        let cache = Arc::new(MemorySessionCache::default());
        let dispatch_request =
            request(CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("track/items/failed/spec.json"))?,
            ])?))?;
        let cache_key = ProviderSessionCacheKey::TrackCapability {
            track_id: track_id.clone(),
            capability: dispatch_request.request.capability.clone(),
        };
        cache.save(
            &cache_key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("failed-resume-session".to_owned())?,
                GROK_PROVIDER_NAME.clone(),
                dispatch_request.profile.model.clone(),
                dispatch_request.profile.effort,
            ),
        )?;
        let runner = Arc::new(RecordingProcessRunner {
            responses: Mutex::new(vec![
                Ok(successful_process_output()),
                Err(CapabilityExecError::DispatchFailed {
                    provider: GROK_PROVIDER_NAME.clone(),
                    detail: CapabilityFailureDetail::new("resume process failed"),
                }),
            ]),
            ..Default::default()
        });
        let adapter = GrokCapabilityAdapter::with_process_runner_and_session_cache(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
            cache,
            Some(track_id),
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(&dispatch_request, &mut stdout)?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 2);
        let first_args = &invocations.first().expect("resume invocation recorded").1;
        let second_args = &invocations.get(1).expect("fresh fallback recorded").1;
        assert!(first_args.windows(2).any(|pair| pair == ["--resume", "failed-resume-session"]));
        assert!(!second_args.iter().any(|arg| arg == "--resume"));
        assert_explicit_grok_settings(first_args, "workspace");
        assert_explicit_grok_settings(second_args, "workspace");
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_provider_mismatched_resume_starts_fresh_session_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let track_id = TrackId::try_new("track-provider-mismatch")?;
        let cache = Arc::new(MemorySessionCache::default());
        let dispatch_request =
            request(CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from(
                    "track/items/provider-mismatch/spec.json",
                ))?,
            ])?))?;
        let cache_key = ProviderSessionCacheKey::TrackCapability {
            track_id: track_id.clone(),
            capability: dispatch_request.request.capability.clone(),
        };
        cache.save(
            &cache_key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("provider-mismatched-session".to_owned())?,
                ProviderName::try_new("other-provider")?,
                dispatch_request.profile.model.clone(),
                dispatch_request.profile.effort,
            ),
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner_and_session_cache(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
            cache,
            Some(track_id),
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(&dispatch_request, &mut stdout)?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let args = &invocations.first().expect("fresh invocation recorded").1;
        assert!(!args.iter().any(|arg| arg == "--resume"));
        assert_explicit_grok_settings(args, "workspace");
        Ok(())
    }

    #[test]
    fn test_grok_capability_adapter_model_mismatched_resume_starts_fresh_session_with_explicit_settings()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        write_grok_adapter(directory.path(), None)?;
        let track_id = TrackId::try_new("track-model-mismatch")?;
        let cache = Arc::new(MemorySessionCache::default());
        let dispatch_request =
            request(CapabilityResumeRequest::Resume(TargetArtifactSet::try_new(vec![
                TargetArtifactPath::try_new(PathBuf::from("track/items/model-mismatch/spec.json"))?,
            ])?))?;
        let cache_key = ProviderSessionCacheKey::TrackCapability {
            track_id: track_id.clone(),
            capability: dispatch_request.request.capability.clone(),
        };
        cache.save(
            &cache_key,
            &ProviderSessionCacheEntry::new(
                ProviderSessionId::try_new("model-mismatched-session".to_owned())?,
                GROK_PROVIDER_NAME.clone(),
                ModelName::try_new("other-model")?,
                dispatch_request.profile.effort,
            ),
        )?;
        let runner = Arc::new(RecordingProcessRunner::default());
        let adapter = GrokCapabilityAdapter::with_process_runner_and_session_cache(
            directory.path().to_owned(),
            directory.path().join("runtime"),
            runner.clone(),
            cache,
            Some(track_id),
        );
        let mut stdout = Vec::new();

        adapter.dispatch_with_stdout(&dispatch_request, &mut stdout)?;

        let invocations = runner.invocations.lock().expect("process recorder lock");
        assert_eq!(invocations.len(), 1);
        let args = &invocations.first().expect("fresh invocation recorded").1;
        assert!(!args.iter().any(|arg| arg == "--resume"));
        assert_explicit_grok_settings(args, "workspace");
        Ok(())
    }
}
