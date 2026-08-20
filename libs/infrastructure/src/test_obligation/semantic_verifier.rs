//! Shared runtime for the obligation-fulfillment and waiver semantic verifiers.
//!
//! The two verifier adapters ([`super::fulfillment_verifier`] /
//! [`super::waiver_verifier`]) differ only in their verdict vocabulary and prompt
//! wording; the provider-resolution, subprocess invocation, and fail-closed
//! JSON-verdict decoding are identical. That responsibility-neutral core lives
//! here so neither adapter re-implements it (ADR D7: share the semantic-verdict
//! core rather than copy-pasting a third verifier).
//!
//! The default runners reuse the shared LLM subprocess pipeline
//! ([`crate::ref_verify::process_runner::make_agent_process_runner`]) — the same
//! "sotp CLI wrapper" mechanics every semantic verifier spawns. Only the OS-level
//! subprocess plumbing is shared; the obligation-fulfillment verdict / cache
//! types stay independent of ref-verify (ADR D6).
//!
//! Fulfillment and waiver differ in one dimension: the fulfillment verdict
//! carries a D6 fail `category` while waiver's shape matches ref-verify.
//! When codex is the routed provider, the two verifiers therefore need
//! different structured-output schemas — [`default_fulfillment_verifier_runner`]
//! uses [`crate::ref_verify::process_runner::CODEX_FULFILLMENT_OUTPUT_SCHEMA`],
//! and [`default_waiver_verifier_runner`] uses the narrower
//! [`crate::ref_verify::process_runner::CODEX_OUTPUT_SCHEMA`] shared with
//! ref-verify. Without that split, codex would strip `category` and force every
//! fulfillment fail into `central_unverified`, breaking the calibration probe
//! whose contradiction / substitution category must round-trip.

use std::path::PathBuf;
use std::sync::Arc;

use domain::ModelTier;
use domain::tddd::test_obligation::errors::SemanticVerifierError;
use usecase::dry_write_driver::CapabilityName;
use usecase::ref_verify::RefVerifyError;

use crate::agent_profiles::{AgentProfiles, ResolvedExecution, RoundType};
use crate::test_obligation::diagnostic;

/// Injectable seam for a semantic-verifier LLM call: takes a resolved
/// provider/model and a rendered prompt, returns the raw model output.
///
/// The production runner spawns the provider subprocess; unit tests inject a
/// closure returning canned output so no subprocess runs.
pub(crate) type SemanticVerifierRunner =
    dyn Fn(ResolvedExecution, String) -> Result<String, SemanticVerifierError> + Send + Sync;

/// Wire form of the verdict discriminator shared by both verifiers.
///
/// `pass` maps to the verifier-specific success variant (fulfilled / waived),
/// `fail` to a rejection, and `pending` to the fail-at-gate "could not confirm".
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VerdictKindWire {
    /// The claim holds; a citation is required (enforced by the caller).
    Pass,
    /// The claim does not hold; a reason is required (enforced by the caller).
    Fail,
    /// The verifier could not confirm; treated as fail at the gate.
    Pending,
}

/// Builds a [`SemanticVerifierError::VerifierPort`] from a diagnostic message.
pub(crate) fn semantic_verifier_error(message: &str) -> SemanticVerifierError {
    SemanticVerifierError::VerifierPort(diagnostic(message))
}

/// Maps the domain [`ModelTier`] to the profile-resolution [`RoundType`].
pub(crate) fn tier_to_round_type(tier: ModelTier) -> RoundType {
    match tier {
        ModelTier::Fast => RoundType::Fast,
        ModelTier::Final => RoundType::Final,
    }
}

/// Resolves the provider/model for `capability` at `round`, failing closed when
/// the capability is not defined in `agent-profiles.json`.
pub(crate) fn resolve_execution_or_err(
    profile: &AgentProfiles,
    capability: &str,
    round: RoundType,
) -> Result<ResolvedExecution, SemanticVerifierError> {
    let capability_name = CapabilityName::try_new(capability).map_err(|error| {
        semantic_verifier_error(&format!("invalid capability '{capability}': {error}"))
    })?;
    profile
        .resolve_execution(&capability_name, round)
        .map_err(|error| semantic_verifier_error(&error.to_string()))
}

/// Parses the verifier response as exactly one JSON verdict object of type `T`.
///
/// The response must be a single JSON object after trimming; prose, examples, or
/// trailing brace blocks fail closed rather than being scanned for a verdict.
pub(crate) fn extract_verdict_json<T: serde::de::DeserializeOwned>(
    raw: &str,
) -> Result<T, SemanticVerifierError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(semantic_verifier_error("semantic verifier returned empty output"));
    }
    serde_json::from_str::<T>(trimmed).map_err(|e| {
        semantic_verifier_error(&format!(
            "semantic verifier response must be exactly one verdict JSON object: {e}"
        ))
    })
}

/// Builds the production obligation-fulfillment runner.
///
/// Wraps the shared LLM subprocess pipeline with the fulfillment-specific
/// codex structured-output schema
/// ([`crate::ref_verify::process_runner::CODEX_FULFILLMENT_OUTPUT_SCHEMA`]) so a
/// codex-routed fulfillment verdict can carry its D6 `category` back through
/// calibration instead of being flattened to `central_unverified` by the
/// ref-verify schema.
pub(crate) fn default_fulfillment_verifier_runner(
    workspace_root: PathBuf,
) -> Arc<SemanticVerifierRunner> {
    build_semantic_verifier_runner(
        workspace_root,
        crate::ref_verify::process_runner::CODEX_FULFILLMENT_OUTPUT_SCHEMA,
        "obligation-fulfillment-verifier",
    )
}

/// Builds the production waiver runner.
///
/// The waiver verdict has no fail category (D8); its wire shape matches
/// ref-verify. Reuse the ref-verify codex schema unchanged so a codex-routed
/// waiver run keeps its historical structural constraint.
pub(crate) fn default_waiver_verifier_runner(
    workspace_root: PathBuf,
) -> Arc<SemanticVerifierRunner> {
    build_semantic_verifier_runner(
        workspace_root,
        crate::ref_verify::process_runner::CODEX_OUTPUT_SCHEMA,
        "waiver-verifier",
    )
}

/// Shared constructor for the fulfillment / waiver production runners.
///
/// Reuses the shared claude / codex / gemini subprocess pipeline
/// ([`crate::ref_verify::process_runner::make_agent_process_runner`]) with the
/// caller-supplied codex structured-output schema, and rewraps
/// [`RefVerifyError`] into [`SemanticVerifierError`] so the obligation gate
/// never surfaces a ref-verify error type. `workspace_root` anchors provider
/// subprocesses and transient provider output files independently of the
/// process CWD.
fn build_semantic_verifier_runner(
    workspace_root: PathBuf,
    codex_output_schema: &'static str,
    capability: &'static str,
) -> Arc<SemanticVerifierRunner> {
    let inner = crate::ref_verify::process_runner::make_agent_process_runner(
        workspace_root,
        codex_output_schema,
    );
    Arc::new(move |resolved, prompt| {
        crate::ref_verify::process_runner::with_ref_verifier_capability(capability, || {
            inner(resolved, prompt)
        })
        .map_err(|err| match err {
            RefVerifyError::VerifierPort { message } => semantic_verifier_error(&message),
            other => {
                semantic_verifier_error(&format!("semantic verifier subprocess failed: {other:?}"))
            }
        })
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn tier_to_round_type_maps_both_tiers() {
        assert_eq!(tier_to_round_type(ModelTier::Fast), RoundType::Fast);
        assert_eq!(tier_to_round_type(ModelTier::Final), RoundType::Final);
    }

    #[test]
    fn extract_verdict_json_rejects_empty_output() {
        let result: Result<serde_json::Value, _> = extract_verdict_json("   \n ");
        assert!(result.is_err());
    }

    #[test]
    fn extract_verdict_json_rejects_prose_wrapped_object() {
        let raw = r#"Here is my verdict: {"kind": "pass"} done."#;
        let result: Result<serde_json::Value, _> = extract_verdict_json(raw);
        assert!(result.is_err());
    }

    #[test]
    fn extract_verdict_json_parses_single_trimmed_object() {
        let value: serde_json::Value = extract_verdict_json("\n  {\"kind\": \"pass\"}\n").unwrap();
        assert_eq!(value.get("kind").and_then(serde_json::Value::as_str), Some("pass"));
    }

    #[test]
    fn resolve_execution_or_err_reports_missing_capability() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent-profiles.json");
        std::fs::write(&path, r#"{ "schema_version": 1, "providers": {}, "capabilities": {} }"#)
            .unwrap();
        let profile = AgentProfiles::load(dir.path(), &path).unwrap();
        let err =
            resolve_execution_or_err(&profile, "obligation-fulfillment-verifier", RoundType::Fast)
                .unwrap_err();
        let SemanticVerifierError::VerifierPort(message) = err;
        assert!(message.as_str().contains("obligation-fulfillment-verifier"));
    }
}
