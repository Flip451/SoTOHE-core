//! Spec document operations (approve, etc.).

use std::process::ExitCode;

use clap::Subcommand;

use crate::CliError;

#[derive(Subcommand)]
pub enum SpecCommand {
    /// Approve a spec, recording a content hash and timestamp.
    Approve {
        /// Path to the track directory (e.g., track/items/<id>).
        track_dir: String,
    },
}

pub fn execute(cmd: SpecCommand) -> ExitCode {
    match cmd {
        SpecCommand::Approve { track_dir } => match approve(&track_dir) {
            Ok(()) => {
                println!("[OK] Spec approved: {track_dir}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                e.exit_code()
            }
        },
    }
}

/// Reject a path containing any symlink component or resolving outside cwd.
fn validate_path(path: &std::path::Path, label: &str) -> Result<(), CliError> {
    // Walk every prefix to catch symlinks in ancestor directories.
    let mut accumulated = std::path::PathBuf::new();
    for component in path.components() {
        accumulated.push(component);
        if accumulated.is_symlink() {
            return Err(CliError::Message(format!(
                "{label} contains a symlink at {}",
                accumulated.display()
            )));
        }
    }
    // Ensure resolved path stays under cwd.
    if path.exists() {
        let canonical = path
            .canonicalize()
            .map_err(|e| CliError::Message(format!("failed to resolve {label}: {e}")))?;
        let cwd = std::env::current_dir()
            .and_then(|p| p.canonicalize())
            .map_err(|e| CliError::Message(format!("failed to get cwd: {e}")))?;
        if !canonical.starts_with(&cwd) {
            return Err(CliError::Message(format!("{label} resolves outside repository")));
        }
    }
    Ok(())
}

fn approve(track_dir: &str) -> Result<(), CliError> {
    if track_dir.is_empty() {
        return Err(CliError::Message("track_dir must not be empty".to_owned()));
    }
    if track_dir.contains("..") {
        return Err(CliError::Message(format!("track_dir must not contain '..': {track_dir}")));
    }

    // Reject symlinks at directory or file level.
    let dir_path = std::path::Path::new(track_dir);
    validate_path(dir_path, track_dir)?;
    let spec_path_buf = dir_path.join("spec.json");
    validate_path(&spec_path_buf, "spec.json")?;
    let spec_md_buf = dir_path.join("spec.md");
    if spec_md_buf.exists() {
        validate_path(&spec_md_buf, "spec.md")?;
    }

    let spec_path = spec_path_buf
        .to_str()
        .ok_or_else(|| CliError::Message(format!("non-UTF-8 path: {track_dir}/spec.json")))?;
    let content = std::fs::read_to_string(spec_path)
        .map_err(|e| CliError::Message(format!("failed to read {spec_path}: {e}")))?;

    let mut doc = infrastructure::spec::codec::decode(&content)
        .map_err(|e| CliError::Message(format!("failed to decode {spec_path}: {e}")))?;

    let hash = infrastructure::spec::codec::compute_content_hash(&doc)
        .map_err(|e| CliError::Message(format!("failed to compute content hash: {e}")))?;

    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let timestamp = domain::Timestamp::new(&now)
        .map_err(|e| CliError::Message(format!("failed to create timestamp: {e}")))?;

    doc.approve(timestamp, hash)
        .map_err(|e| CliError::Message(format!("failed to approve: {e}")))?;

    let json = infrastructure::spec::codec::encode(&doc)
        .map_err(|e| CliError::Message(format!("failed to encode {spec_path}: {e}")))?;

    infrastructure::track::atomic_write::atomic_write_file(
        &spec_path_buf,
        format!("{json}\n").as_bytes(),
    )
    .map_err(|e| CliError::Message(format!("failed to write {spec_path}: {e}")))?;

    // Regenerate spec.md rendered view. On failure, spec.md may be stale —
    // run `cargo make track-sync-views` to fix.
    let rendered = infrastructure::spec::render::render_spec(&doc);
    let spec_md_path = std::path::Path::new(track_dir).join("spec.md");
    infrastructure::track::atomic_write::atomic_write_file(&spec_md_path, rendered.as_bytes())
        .map_err(|e| {
            CliError::Message(format!(
                "failed to write spec.md (spec.json is approved; run `cargo make track-sync-views` to fix spec.md): {e}"
            ))
        })?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_approve_updates_spec_json() {
        // Use absolute path under cwd so test works regardless of thread cwd changes.
        let base = std::env::current_dir().unwrap().join("tmp");
        std::fs::create_dir_all(&base).unwrap();
        let dir = tempfile::tempdir_in(&base).unwrap();
        let track_dir = dir.path().to_str().unwrap();

        let spec_json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Test Feature",
  "goal": ["Test goal"],
  "scope": {"in_scope": [{"text": "item", "sources": ["PRD"]}], "out_of_scope": []},
  "constraints": [],
  "domain_states": [],
  "acceptance_criteria": [{"text": "AC", "sources": ["PRD"]}]
}
"#;
        std::fs::write(format!("{track_dir}/spec.json"), spec_json).unwrap();

        approve(track_dir).unwrap();

        let result = std::fs::read_to_string(format!("{track_dir}/spec.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(parsed["status"], "approved");
        assert!(parsed["approved_at"].as_str().is_some());
        assert!(
            parsed["content_hash"].as_str().unwrap().starts_with("sha256:"),
            "content_hash should start with sha256:"
        );

        // Verify spec.md was regenerated with approved status
        let spec_md = std::fs::read_to_string(format!("{track_dir}/spec.md")).unwrap();
        assert!(spec_md.contains("status: approved"), "spec.md should contain approved status");
        assert!(spec_md.contains("approved_at:"), "spec.md should contain approved_at");

        // dir is cleaned up automatically by tempfile::TempDir::drop
    }

    #[test]
    fn test_approve_nonexistent_dir_returns_error() {
        let result = approve("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_rejects_path_traversal() {
        let result = approve("track/items/../../etc");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains(".."), "error should mention path traversal: {err}");
    }
}
