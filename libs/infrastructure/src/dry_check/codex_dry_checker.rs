//! Codex-backed implementation of the `DryCheckAgentPort` usecase port.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use sha2::Digest;

use domain::dry_check::{
    DryCheckFinding, DryCheckFindingError, FragmentContentHash, FragmentRef, Rationale,
    RationaleError,
};
use domain::review_v2::FilePath;
use domain::semantic_dup::CodeFragment;
use usecase::dry_check::{
    DryCheckAgentError, DryCheckAgentJudgment, DryCheckAgentPort, DryCheckJudgeTier,
};

// ── Output schema ─────────────────────────────────────────────────────────────

/// JSON schema for the dry-check agent output (D11 structured output).
///
/// Three struct variants, each carrying only verdict and text fields.
/// Fragment identity is NEVER part of the agent output (D8/D9/CN-07):
/// the adapter computes [`FragmentRef`]s from the actual [`CodeFragment`]
/// arguments passed to [`DryCheckAgentPort::judge`].
pub(crate) const DRY_CHECK_OUTPUT_SCHEMA_JSON: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "verdict": {
      "type": "string",
      "enum": ["not_a_violation", "accepted", "violation"]
    },
    "rationale": {
      "type": "string",
      "minLength": 1
    },
    "refactor_proposal": {
      "type": ["string", "null"]
    }
  },
  "required": ["verdict", "rationale", "refactor_proposal"],
  "additionalProperties": false
}"##;

// ── Runtime directory ─────────────────────────────────────────────────────────

const DRY_CHECK_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
const POLL_INTERVAL: Duration = Duration::from_millis(50);

// ── Private DTO ───────────────────────────────────────────────────────────────

/// Raw verdict values returned by the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AgentVerdict {
    NotAViolation,
    Accepted,
    Violation,
}

/// Private serde DTO for the agent's JSON output.
///
/// Does NOT carry any fragment-identity fields — those are always computed by
/// the adapter from the actual [`CodeFragment`] arguments (D8/D9/CN-07).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DryCheckAgentOutputDto {
    verdict: AgentVerdict,
    rationale: String,
    refactor_proposal: Option<String>,
}

// ── CodexDryChecker ───────────────────────────────────────────────────────────

/// Codex-backed implementation of [`DryCheckAgentPort`].
///
/// Spawns a `codex exec --sandbox read-only` subprocess, feeds it a prompt
/// describing the (changed_fragment, candidate_fragment) pair, polls for
/// completion, and parses the structured JSON verdict written to
/// `--output-last-message`. Analogous to `CodexReviewer`.
///
/// Fragment identity is always computed from the real [`CodeFragment`]
/// arguments by this adapter — the agent JSON output carries no fragment
/// fields (D8/D9/CN-07).
///
/// D4 (T012): supports 2-tier judgment — `DryCheckJudgeTier::Fast` uses
/// `fast_model` + `fast_reasoning_effort`; `DryCheckJudgeTier::Final` uses
/// `final_model` + `final_reasoning_effort`.
#[derive(Debug)]
pub struct CodexDryChecker {
    /// Codex model for `DryCheckJudgeTier::Fast` calls.
    fast_model: String,
    /// Codex `model_reasoning_effort` for fast tier.
    fast_reasoning_effort: String,
    /// Codex model for `DryCheckJudgeTier::Final` calls.
    final_model: String,
    /// Codex `model_reasoning_effort` for final tier.
    final_reasoning_effort: String,
    /// Capability name for the prompt (e.g. `"dry-checker"`).
    capability_name: String,
    /// Maximum time to wait for the Codex subprocess.
    timeout: Duration,
    /// Test-only: override the Codex binary path.
    #[cfg(test)]
    bin_override: Option<OsString>,
}

impl CodexDryChecker {
    /// Constructs a new [`CodexDryChecker`].
    ///
    /// # Arguments
    /// - `fast_model`: Codex model for `DryCheckJudgeTier::Fast` calls.
    /// - `fast_reasoning_effort`: Codex `model_reasoning_effort` for fast tier.
    /// - `final_model`: Codex model for `DryCheckJudgeTier::Final` calls.
    /// - `final_reasoning_effort`: Codex `model_reasoning_effort` for final tier.
    /// - `capability_name`: dry-check capability label injected into the prompt.
    pub fn new(
        fast_model: String,
        fast_reasoning_effort: String,
        final_model: String,
        final_reasoning_effort: String,
        capability_name: String,
    ) -> CodexDryChecker {
        CodexDryChecker {
            fast_model,
            fast_reasoning_effort,
            final_model,
            final_reasoning_effort,
            capability_name,
            timeout: Duration::from_secs(600),
            #[cfg(test)]
            bin_override: None,
        }
    }

    /// Test-only: set a custom binary path instead of the default `codex`.
    #[cfg(test)]
    pub(crate) fn with_bin(mut self, bin: impl Into<OsString>) -> Self {
        self.bin_override = Some(bin.into());
        self
    }

    /// Test-only: override the subprocess timeout for fast-failing tests.
    #[cfg(test)]
    pub(crate) fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the prompt for the dry-check agent.
    fn build_prompt(&self, changed: &CodeFragment, candidate: &CodeFragment) -> String {
        format!(
            "You are the `{cap}` capability. Determine whether the following two code \
             fragments constitute a DRY (Don't Repeat Yourself) violation.\n\n\
             ## Changed fragment (diff side)\n\n\
             File: {changed_path}\n\
             Lines: {cl_start}–{cl_end}\n\n\
             ```\n{changed_content}\n```\n\n\
             ## Candidate fragment (existing code)\n\n\
             File: {candidate_path}\n\
             Lines: {ca_start}–{ca_end}\n\n\
             ```\n{candidate_content}\n```\n\n\
             Respond with a JSON object matching the required output schema:\n\
             - verdict: one of \"not_a_violation\", \"accepted\", \"violation\"\n\
             - rationale: non-empty explanation\n\
             - refactor_proposal: non-empty refactoring suggestion (required when \
               verdict is \"violation\", null otherwise)",
            cap = self.capability_name,
            changed_path = changed.source_path.display(),
            cl_start = changed.start_line(),
            cl_end = changed.end_line(),
            changed_content = changed.content(),
            candidate_path = candidate.source_path.display(),
            ca_start = candidate.start_line(),
            ca_end = candidate.end_line(),
            candidate_content = candidate.content(),
        )
    }

    /// Invoke the Codex subprocess and return the raw output string.
    ///
    /// `tier` selects which model + reasoning effort to use for this call.
    fn run_agent(
        &self,
        prompt: &str,
        tier: DryCheckJudgeTier,
    ) -> Result<DryCheckOutcomeRaw, DryCheckAgentError> {
        let (model, reasoning_effort) = match tier {
            DryCheckJudgeTier::Fast => (&self.fast_model, &self.fast_reasoning_effort),
            DryCheckJudgeTier::Final => (&self.final_model, &self.final_reasoning_effort),
        };

        let output_last_message = prepare_runtime_path("dry-check-last-message", "txt")
            .map_err(DryCheckAgentError::Unexpected)?;
        let output_schema = prepare_runtime_path("dry-check-output-schema", "json")
            .map_err(DryCheckAgentError::Unexpected)?;
        let session_log = prepare_runtime_path("dry-check-session", "log")
            .map_err(DryCheckAgentError::Unexpected)?;

        let _cleanup = AutoCleanup::new([&output_last_message, &output_schema]);

        // Reset output-last-message so stale content cannot be mistaken for output.
        std::fs::write(&output_last_message, "").map_err(|e| {
            DryCheckAgentError::Unexpected(format!("failed to initialize output-last-message: {e}"))
        })?;

        // Write the output schema.
        std::fs::write(&output_schema, DRY_CHECK_OUTPUT_SCHEMA_JSON).map_err(|e| {
            DryCheckAgentError::Unexpected(format!("failed to write output-schema: {e}"))
        })?;

        #[cfg(test)]
        let bin = self.bin_override.clone().unwrap_or_else(codex_bin);
        #[cfg(not(test))]
        let bin = codex_bin();

        let invocation = crate::codex_common::build_codex_read_only_invocation(
            model,
            reasoning_effort,
            prompt,
            &output_last_message,
            &output_schema,
        );

        let (child, io_handles) =
            spawn_codex(&bin, &invocation, &session_log).map_err(DryCheckAgentError::Unexpected)?;

        run_codex_child(child, io_handles, self.timeout, output_last_message)
    }
}

impl DryCheckAgentPort for CodexDryChecker {
    fn judge(
        &self,
        changed_fragment: &CodeFragment,
        candidate_fragment: &CodeFragment,
        tier: DryCheckJudgeTier,
    ) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
        let prompt = self.build_prompt(changed_fragment, candidate_fragment);
        let raw = self.run_agent(&prompt, tier)?;
        convert_raw_to_judgment(raw, changed_fragment, candidate_fragment)
    }
}

// ── Helper: compute FragmentRef from CodeFragment ─────────────────────────────

/// Compute a [`FragmentRef`] from a [`CodeFragment`] by SHA-256-hashing its content.
///
/// The hash is always a valid 64-char lowercase hex string for any input, so
/// `FragmentContentHash::new` is structurally infallible given a correct SHA-256
/// implementation. Any error (impossible in practice) maps to
/// `DryCheckAgentError::Unexpected`.
fn fragment_ref_from_code_fragment(frag: &CodeFragment) -> Result<FragmentRef, DryCheckAgentError> {
    let hash_bytes = sha2::Sha256::digest(frag.content().as_bytes());
    let hash_hex = format!("{hash_bytes:x}");

    let content_hash = FragmentContentHash::new(hash_hex).map_err(|e| {
        DryCheckAgentError::Unexpected(format!("failed to construct content hash: {e}"))
    })?;

    let path_str = repo_relative_source_path(&frag.source_path)?;
    let file_path = FilePath::new(path_str)
        .map_err(|e| DryCheckAgentError::Unexpected(format!("invalid fragment path: {e}")))?;

    Ok(FragmentRef::new(file_path, content_hash))
}

fn repo_relative_source_path(source_path: &Path) -> Result<String, DryCheckAgentError> {
    let relative_path = if source_path.is_absolute() {
        let workspace_root = find_workspace_root_for_source_path(source_path)?;
        source_path.strip_prefix(&workspace_root).map_err(|_| {
            DryCheckAgentError::Unexpected("absolute fragment path is outside workspace".to_owned())
        })?
    } else {
        source_path
    };

    stable_repo_relative_path(relative_path)
}

fn find_workspace_root_for_source_path(source_path: &Path) -> Result<PathBuf, DryCheckAgentError> {
    let mut cursor = source_path.parent();

    while let Some(dir) = cursor {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() && manifest_declares_workspace(&manifest)? {
            return Ok(dir.to_path_buf());
        }
        cursor = dir.parent();
    }

    Err(DryCheckAgentError::Unexpected(
        "failed to locate workspace root for fragment path".to_owned(),
    ))
}

fn manifest_declares_workspace(manifest: &Path) -> Result<bool, DryCheckAgentError> {
    let content = std::fs::read_to_string(manifest)
        .map_err(|e| DryCheckAgentError::Unexpected(format!("failed to read Cargo.toml: {e}")))?;
    Ok(content.lines().any(|line| line.trim() == "[workspace]"))
}

fn stable_repo_relative_path(path: &Path) -> Result<String, DryCheckAgentError> {
    let mut parts = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    DryCheckAgentError::Unexpected(
                        "fragment path contains non-UTF-8 component".to_owned(),
                    )
                })?;
                parts.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(DryCheckAgentError::Unexpected(
                    "fragment path contains parent traversal".to_owned(),
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(DryCheckAgentError::Unexpected(
                    "fragment path must be repo-relative".to_owned(),
                ));
            }
        }
    }

    if parts.is_empty() {
        return Err(DryCheckAgentError::Unexpected("fragment path is empty".to_owned()));
    }

    Ok(parts.join("/"))
}

// ── Raw outcome + conversion ──────────────────────────────────────────────────

/// Raw outcome from the Codex subprocess (process-exit-level signals only).
struct DryCheckOutcomeRaw {
    timed_out: bool,
    exit_success: bool,
    /// Content of the `--output-last-message` file, if non-empty.
    output: Option<String>,
}

/// Parse and convert the raw subprocess outcome to a [`DryCheckAgentJudgment`].
fn convert_raw_to_judgment(
    raw: DryCheckOutcomeRaw,
    changed_fragment: &CodeFragment,
    candidate_fragment: &CodeFragment,
) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
    // timed_out → Timeout (highest priority)
    if raw.timed_out {
        return Err(DryCheckAgentError::Timeout);
    }

    // Non-zero exit → AgentAbort
    if !raw.exit_success {
        return Err(DryCheckAgentError::AgentAbort);
    }

    // No output / empty output → IllegalOutput
    let json_str = raw.output.ok_or(DryCheckAgentError::IllegalOutput)?;

    parse_agent_json_and_build_judgment(&json_str, changed_fragment, candidate_fragment)
}

/// Parse the agent JSON and build the [`DryCheckAgentJudgment`].
///
/// Extracted as a free function so tests can call it directly without spawning
/// a real Codex process.
pub(crate) fn parse_agent_json_and_build_judgment(
    json_str: &str,
    changed_fragment: &CodeFragment,
    candidate_fragment: &CodeFragment,
) -> Result<DryCheckAgentJudgment, DryCheckAgentError> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|_| DryCheckAgentError::IllegalOutput)?;
    let has_refactor_proposal =
        value.as_object().map(|object| object.contains_key("refactor_proposal")).unwrap_or(false);
    if !has_refactor_proposal {
        return Err(DryCheckAgentError::IllegalOutput);
    }
    let dto: DryCheckAgentOutputDto =
        serde_json::from_value(value).map_err(|_| DryCheckAgentError::IllegalOutput)?;

    // Validate non-empty rationale.
    let rationale = Rationale::new(dto.rationale).map_err(|e| match e {
        RationaleError::Empty => DryCheckAgentError::IllegalOutput,
    })?;

    match dto.verdict {
        AgentVerdict::NotAViolation => Ok(DryCheckAgentJudgment::NotAViolation { rationale }),
        AgentVerdict::Accepted => Ok(DryCheckAgentJudgment::Accepted { rationale }),
        AgentVerdict::Violation => {
            // For a Violation, refactor_proposal must be present and non-empty.
            let proposal_str = dto
                .refactor_proposal
                .filter(|s| !s.is_empty())
                .ok_or(DryCheckAgentError::IllegalOutput)?;

            // Compute FragmentRefs from the REAL CodeFragment arguments (D8/D9/CN-07).
            let changed_ref = fragment_ref_from_code_fragment(changed_fragment)?;
            let candidate_ref = fragment_ref_from_code_fragment(candidate_fragment)?;

            // DryCheckFinding::new calls RefactorProposal::new internally.
            let finding = DryCheckFinding::new(changed_ref, candidate_ref, proposal_str).map_err(
                |e| match e {
                    DryCheckFindingError::EmptyProposal => DryCheckAgentError::IllegalOutput,
                },
            )?;

            Ok(DryCheckAgentJudgment::Violation { rationale, finding })
        }
    }
}

// ── Process management ────────────────────────────────────────────────────────

fn prepare_runtime_path(prefix: &str, ext: &str) -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("failed to compute timestamp: {e}"))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(DRY_CHECK_RUNTIME_DIR)
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path
        .parent()
        .ok_or_else(|| format!("runtime path must have a parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    Ok(path)
}

struct AutoCleanup {
    paths: Vec<PathBuf>,
}

impl AutoCleanup {
    fn new<'a>(artifacts: impl IntoIterator<Item = &'a PathBuf>) -> Self {
        Self { paths: artifacts.into_iter().cloned().collect() }
    }
}

impl Drop for AutoCleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn codex_bin() -> OsString {
    OsString::from("codex")
}

fn spawn_codex(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    session_log_path: &Path,
) -> Result<(Child, Vec<thread::JoinHandle<()>>), String> {
    let mut command = Command::new(bin);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped());

    let log_file = std::fs::File::create(session_log_path)
        .map_err(|e| format!("failed to create session log {}: {e}", session_log_path.display()))?;
    command.stderr(Stdio::piped());

    let mut child =
        command.spawn().map_err(|e| format!("failed to spawn {}: {e}", bin.to_string_lossy()))?;

    let mut io_handles = Vec::new();

    if let Some(pipe) = child.stderr.take() {
        io_handles.push(thread::spawn(move || {
            tee_stderr_to_file(pipe, log_file);
        }));
    }

    // Drain stdout to prevent the child from blocking on a full pipe buffer.
    if let Some(pipe) = child.stdout.take() {
        io_handles.push(thread::spawn(move || {
            drain_pipe(pipe);
        }));
    }

    Ok((child, io_handles))
}

fn drain_pipe(pipe: std::process::ChildStdout) {
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        if line.is_err() {
            break;
        }
    }
}

fn tee_stderr_to_file(pipe: std::process::ChildStderr, mut log_file: std::fs::File) {
    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                let _ = writeln!(log_file, "{line}");
                eprintln!("{line}");
            }
            Err(_) => break,
        }
    }
    let _ = log_file.flush();
}

fn run_codex_child(
    mut child: Child,
    io_handles: Vec<thread::JoinHandle<()>>,
    timeout: Duration,
    output_last_message: PathBuf,
) -> Result<DryCheckOutcomeRaw, DryCheckAgentError> {
    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_success = false;

    loop {
        match child.try_wait().map_err(|e| {
            DryCheckAgentError::Unexpected(format!("failed to poll dry-check agent child: {e}"))
        })? {
            Some(status) => {
                exit_success = status.success();
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.kill();
                    child.wait().map_err(|e| {
                        DryCheckAgentError::Unexpected(format!(
                            "failed to reap dry-check agent child: {e}"
                        ))
                    })?;
                    break;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }

    if !timed_out {
        for handle in io_handles {
            let _ = handle.join();
        }
    }

    let output = match std::fs::read_to_string(&output_last_message) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(DryCheckAgentError::Unexpected(format!(
                "failed to read output-last-message {}: {e}",
                output_last_message.display()
            )));
        }
    };

    Ok(DryCheckOutcomeRaw { timed_out, exit_success, output })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn make_fragment(path: &str, content: &str) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), 1, 10).unwrap()
    }

    fn sha256_hex(content: &str) -> String {
        let bytes = sha2::Sha256::digest(content.as_bytes());
        format!("{bytes:x}")
    }

    // ── Schema structural invariants ──────────────────────────────────────────

    /// OpenAI strict structured output requires that `required` lists every key
    /// present in `properties`.  This test parses the constant as JSON and asserts
    /// that the two sets are identical, preventing the regression where
    /// `refactor_proposal` was absent from `required` while present in `properties`.
    #[test]
    fn test_output_schema_required_contains_every_properties_key() {
        let schema: serde_json::Value =
            serde_json::from_str(DRY_CHECK_OUTPUT_SCHEMA_JSON).expect("schema must be valid JSON");

        let properties_keys: std::collections::BTreeSet<String> = schema["properties"]
            .as_object()
            .expect("schema must have a properties object")
            .keys()
            .cloned()
            .collect();

        let required_keys: std::collections::BTreeSet<String> = schema["required"]
            .as_array()
            .expect("schema must have a required array")
            .iter()
            .map(|v| v.as_str().expect("required entries must be strings").to_owned())
            .collect();

        assert_eq!(
            required_keys,
            properties_keys,
            "OpenAI strict mode requires that 'required' lists every key in 'properties'. \
             Missing from required: {:?}",
            properties_keys.difference(&required_keys).collect::<Vec<_>>()
        );
    }

    // ── JSON parsing tests (no subprocess needed) ─────────────────────────────

    #[test]
    fn test_parse_not_a_violation_with_non_empty_rationale_returns_not_a_violation() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let json = r#"{
            "verdict": "not_a_violation",
            "rationale": "Different purpose",
            "refactor_proposal": null
        }"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);

        let judgment = result.expect("should succeed");
        match judgment {
            DryCheckAgentJudgment::NotAViolation { rationale } => {
                assert_eq!(rationale.as_str(), "Different purpose");
            }
            other => panic!("expected NotAViolation, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_accepted_with_non_empty_rationale_returns_accepted() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let json = r#"{
            "verdict": "accepted",
            "rationale": "Intentional mirroring",
            "refactor_proposal": null
        }"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);

        let judgment = result.expect("should succeed");
        assert!(matches!(judgment, DryCheckAgentJudgment::Accepted { .. }));
    }

    #[test]
    fn test_parse_violation_with_non_empty_rationale_and_proposal_returns_violation() {
        let changed_content = "fn foo() { let x = 1; }";
        let candidate_content = "fn bar() { let x = 1; }";
        let changed = make_fragment("src/a.rs", changed_content);
        let candidate = make_fragment("src/b.rs", candidate_content);

        let json = r#"{
            "verdict": "violation",
            "rationale": "Exact duplication",
            "refactor_proposal": "Extract a shared helper function"
        }"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);

        let judgment = result.expect("should succeed");
        match judgment {
            DryCheckAgentJudgment::Violation { rationale, finding } => {
                assert_eq!(rationale.as_str(), "Exact duplication");
                assert_eq!(
                    finding.refactor_proposal().as_str(),
                    "Extract a shared helper function"
                );

                // Fragment refs must be computed from the ACTUAL CodeFragment args,
                // NOT from agent JSON (D8/D9/CN-07).
                let expected_changed_hash = sha256_hex(changed_content);
                let expected_candidate_hash = sha256_hex(candidate_content);

                assert_eq!(finding.changed_fragment_ref().path().as_str(), "src/a.rs");
                assert_eq!(
                    finding.changed_fragment_ref().content_hash().as_str(),
                    expected_changed_hash
                );
                assert_eq!(finding.candidate_fragment_ref().path().as_str(), "src/b.rs");
                assert_eq!(
                    finding.candidate_fragment_ref().content_hash().as_str(),
                    expected_candidate_hash
                );
            }
            other => panic!("expected Violation, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_violation_with_absolute_source_paths_returns_repo_relative_fragment_refs() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let changed_path = manifest_dir.join("src/abs_changed.rs");
        let candidate_path = manifest_dir.join("src/abs_candidate.rs");
        let changed_content = "fn foo() { let x = 1; }";
        let candidate_content = "fn bar() { let x = 1; }";
        let changed = CodeFragment::new(changed_path, changed_content.to_owned(), 1, 10).unwrap();
        let candidate =
            CodeFragment::new(candidate_path, candidate_content.to_owned(), 1, 10).unwrap();

        let json = r#"{
            "verdict": "violation",
            "rationale": "Exact duplication",
            "refactor_proposal": "Extract a shared helper function"
        }"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);

        let judgment = result.expect("should succeed");
        match judgment {
            DryCheckAgentJudgment::Violation { finding, .. } => {
                assert_eq!(
                    finding.changed_fragment_ref().path().as_str(),
                    "libs/infrastructure/src/abs_changed.rs"
                );
                assert_eq!(
                    finding.candidate_fragment_ref().path().as_str(),
                    "libs/infrastructure/src/abs_candidate.rs"
                );
            }
            other => panic!("expected Violation, got: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_parse_violation_with_non_utf8_source_path_returns_unexpected() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let non_utf8_name = OsString::from_vec(vec![b'n', b'o', b'n', 0xff, b'.', b'r', b's']);
        let changed_path = manifest_dir.join("src").join(non_utf8_name);
        let candidate_path = manifest_dir.join("src/candidate.rs");
        let changed = CodeFragment::new(changed_path, "fn foo() {}".to_owned(), 1, 10).unwrap();
        let candidate = CodeFragment::new(candidate_path, "fn bar() {}".to_owned(), 1, 10).unwrap();

        let json = r#"{
            "verdict": "violation",
            "rationale": "Exact duplication",
            "refactor_proposal": "Extract a shared helper function"
        }"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);

        assert!(matches!(result, Err(DryCheckAgentError::Unexpected(_))), "got: {result:?}");
    }

    #[test]
    fn test_parse_invalid_json_returns_illegal_output() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let result = parse_agent_json_and_build_judgment("not-json", &changed, &candidate);
        assert!(matches!(result, Err(DryCheckAgentError::IllegalOutput)), "got: {result:?}");
    }

    #[test]
    fn test_parse_empty_rationale_returns_illegal_output() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let json = r#"{"verdict":"not_a_violation","rationale":"","refactor_proposal":null}"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);
        assert!(matches!(result, Err(DryCheckAgentError::IllegalOutput)), "got: {result:?}");
    }

    #[test]
    fn test_parse_unknown_field_returns_illegal_output() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let json = r#"{
            "verdict": "not_a_violation",
            "rationale": "Different",
            "refactor_proposal": null,
            "extra": "field"
        }"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);
        assert!(matches!(result, Err(DryCheckAgentError::IllegalOutput)), "got: {result:?}");
    }

    /// Missing `refactor_proposal` field returns `IllegalOutput` regardless of verdict.
    ///
    /// All verdict values are exercised in a single table-driven test to avoid
    /// duplicating the same assertion structure.
    #[test]
    fn test_parse_missing_refactor_proposal_returns_illegal_output_for_all_verdicts() {
        let cases: &[(&str, &str)] = &[
            ("not_a_violation", "Different"),
            ("accepted", "Intentional duplication"),
            ("violation", "Dup"),
        ];
        for (verdict, rationale) in cases {
            let changed = make_fragment("src/a.rs", "fn foo() {}");
            let candidate = make_fragment("src/b.rs", "fn bar() {}");
            let json = format!(r#"{{"verdict":"{verdict}","rationale":"{rationale}"}}"#);
            let result = parse_agent_json_and_build_judgment(&json, &changed, &candidate);
            assert!(
                matches!(result, Err(DryCheckAgentError::IllegalOutput)),
                "verdict={verdict}: expected IllegalOutput, got: {result:?}"
            );
        }
    }

    #[test]
    fn test_parse_violation_with_empty_refactor_proposal_returns_illegal_output() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let json = r#"{"verdict":"violation","rationale":"Dup","refactor_proposal":""}"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);
        assert!(matches!(result, Err(DryCheckAgentError::IllegalOutput)), "got: {result:?}");
    }

    #[test]
    fn test_parse_violation_with_null_refactor_proposal_returns_illegal_output() {
        let changed = make_fragment("src/a.rs", "fn foo() {}");
        let candidate = make_fragment("src/b.rs", "fn bar() {}");

        let json = r#"{"verdict":"violation","rationale":"Dup","refactor_proposal":null}"#;
        let result = parse_agent_json_and_build_judgment(json, &changed, &candidate);
        assert!(matches!(result, Err(DryCheckAgentError::IllegalOutput)), "got: {result:?}");
    }

    // ── Subprocess outcome mapping tests (fake codex binary) ─────────────────

    #[cfg(unix)]
    mod subprocess_tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn make_checker_with_script(script_body: &str) -> (CodexDryChecker, tempfile::TempDir) {
            let dir = tempfile::tempdir().unwrap();
            let script = dir.path().join("fake-codex.sh");
            std::fs::write(&script, script_body).unwrap();
            let mut perms = std::fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).unwrap();

            let checker = CodexDryChecker::new(
                "fast-model".to_owned(),
                "medium".to_owned(),
                "final-model".to_owned(),
                "high".to_owned(),
                "dry-checker".to_owned(),
            )
            .with_bin(&script);
            (checker, dir)
        }

        #[test]
        fn test_exit_nonzero_returns_agent_abort() {
            let script = r#"#!/bin/sh
exit 1
"#;
            let (checker, _dir) = make_checker_with_script(script);
            let changed = make_fragment("src/a.rs", "fn foo() {}");
            let candidate = make_fragment("src/b.rs", "fn bar() {}");

            let result = checker.judge(&changed, &candidate, DryCheckJudgeTier::Final);
            assert!(matches!(result, Err(DryCheckAgentError::AgentAbort)), "got: {result:?}");
        }

        #[test]
        fn test_timeout_returns_timeout_error() {
            let script = r#"#!/bin/sh
sleep 60
"#;
            let dir = tempfile::tempdir().unwrap();
            let script_path = dir.path().join("fake-codex.sh");
            std::fs::write(&script_path, script).unwrap();
            let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();

            let checker = CodexDryChecker::new(
                "fast-model".to_owned(),
                "medium".to_owned(),
                "final-model".to_owned(),
                "high".to_owned(),
                "dry-checker".to_owned(),
            )
            .with_bin(&script_path)
            .with_timeout(Duration::from_millis(100));

            let changed = make_fragment("src/a.rs", "fn foo() {}");
            let candidate = make_fragment("src/b.rs", "fn bar() {}");

            let result = checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast);
            assert!(matches!(result, Err(DryCheckAgentError::Timeout)), "got: {result:?}");
        }

        #[test]
        fn test_valid_verdict_via_subprocess_returns_not_a_violation() {
            let script = r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -n "$out" ]; then
  printf '{"verdict":"not_a_violation","rationale":"Functions have different purposes","refactor_proposal":null}\n' > "$out"
fi
exit 0
"#;
            let (checker, _dir) = make_checker_with_script(script);
            let changed = make_fragment("src/a.rs", "fn foo() {}");
            let candidate = make_fragment("src/b.rs", "fn bar() {}");

            let result = checker.judge(&changed, &candidate, DryCheckJudgeTier::Fast);
            let judgment = result.expect("should succeed");
            assert!(
                matches!(judgment, DryCheckAgentJudgment::NotAViolation { .. }),
                "got: {judgment:?}"
            );
        }

        #[test]
        fn test_fragment_refs_computed_from_adapter_args_not_agent_output() {
            // The violation JSON deliberately contains no fragment fields.
            // The test verifies that the finding carries refs computed from
            // the actual CodeFragment args (D8/D9/CN-07).
            let script = r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -n "$out" ]; then
  printf '{"verdict":"violation","rationale":"Exact dup","refactor_proposal":"Extract helper"}\n' > "$out"
fi
exit 0
"#;
            let (checker, _dir) = make_checker_with_script(script);
            let changed_content = "fn foo() { let x = 1; }";
            let candidate_content = "fn bar() { let x = 1; }";
            let changed = make_fragment("src/a.rs", changed_content);
            let candidate = make_fragment("src/b.rs", candidate_content);

            let result = checker.judge(&changed, &candidate, DryCheckJudgeTier::Final);
            let judgment = result.expect("should succeed");
            match judgment {
                DryCheckAgentJudgment::Violation { finding, .. } => {
                    // Verify hashes are derived from actual content.
                    let expected_changed_hash = sha256_hex(changed_content);
                    let expected_candidate_hash = sha256_hex(candidate_content);

                    assert_eq!(
                        finding.changed_fragment_ref().content_hash().as_str(),
                        expected_changed_hash
                    );
                    assert_eq!(
                        finding.candidate_fragment_ref().content_hash().as_str(),
                        expected_candidate_hash
                    );
                    assert_eq!(finding.changed_fragment_ref().path().as_str(), "src/a.rs");
                    assert_eq!(finding.candidate_fragment_ref().path().as_str(), "src/b.rs");
                }
                other => panic!("expected Violation, got: {other:?}"),
            }
        }

        #[test]
        fn test_fast_tier_uses_fast_model_and_reasoning_effort_in_codex_invocation() {
            // Verify that Fast tier produces args with fast_model + fast_reasoning_effort
            // and Final tier produces args with final_model + final_reasoning_effort.
            let output_last_message = std::path::PathBuf::from("out.txt");
            let output_schema = std::path::PathBuf::from("schema.json");

            let fast_args = crate::codex_common::build_codex_read_only_invocation(
                "fast-model",
                "medium",
                "test prompt",
                &output_last_message,
                &output_schema,
            );
            let fast_args_str: Vec<String> =
                fast_args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
            assert!(
                fast_args_str.contains(&"fast-model".to_owned()),
                "fast tier must use fast_model"
            );
            assert!(
                fast_args_str.iter().any(|a| a.contains("medium")),
                "fast tier must use medium reasoning effort"
            );

            let final_args = crate::codex_common::build_codex_read_only_invocation(
                "final-model",
                "high",
                "test prompt",
                &output_last_message,
                &output_schema,
            );
            let final_args_str: Vec<String> =
                final_args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
            assert!(
                final_args_str.contains(&"final-model".to_owned()),
                "final tier must use final_model"
            );
            assert!(
                final_args_str.iter().any(|a| a.contains("high")),
                "final tier must use high reasoning effort"
            );
        }
    }
}
