//! Infrastructure adapter implementing `usecase::merge_gate::TrackBlobReader`
//! via `git show` on the local git repository.
//!
//! This is the bridge between the pure usecase layer (`merge_gate` /
//! `task_completion`) and the low-level git primitives in
//! `crate::git_cli::show`. The adapter:
//!
//! 1. Uses `fetch_blob_safe` to retrieve raw bytes from `origin/<branch>:<path>`
//!    with symlink / submodule rejection baked in.
//! 2. Applies strict UTF-8 decode (`String::from_utf8`) — non-UTF-8 bytes
//!    produce `BlobFetchResult::FetchError` (fail-closed, ADR §D4).
//! 3. Decodes the JSON into a domain aggregate via the existing
//!    `spec::codec` / `tddd::catalogue_codec` / `track::codec` modules.
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D5.3.

use std::path::PathBuf;

use domain::TrackMetadata;
use domain::TypeCatalogueDocument;
use domain::spec::SpecDocument;
use usecase::merge_gate::{BlobFetchResult, TrackBlobReader};

use crate::git_cli::show::{BlobResult, fetch_blob_safe};

/// Adapter that reads track documents from the local git repository via
/// `git show origin/<branch>:<path>`.
///
/// Construct with `GitShowTrackBlobReader::new(repo_root)`. The adapter
/// is stateless apart from the repo root path, so a single instance can
/// be shared across multiple usecase calls (e.g. merge_gate +
/// task_completion from the same `pr.rs::wait_and_merge` invocation).
pub struct GitShowTrackBlobReader {
    repo_root: PathBuf,
}

impl GitShowTrackBlobReader {
    /// Creates a new adapter rooted at the given repository path.
    #[must_use]
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    /// Common path assembly: `track/items/<track_id>/<filename>`.
    fn blob_path(track_id: &str, filename: &str) -> String {
        format!("track/items/{track_id}/{filename}")
    }

    /// Fetches a blob and applies strict UTF-8 decode.
    ///
    /// Maps `BlobResult` → `Result<String, BlobFetchResult<T>>` where the
    /// error variant is already the final port outcome to return (NotFound
    /// or FetchError). Callers use `match` / `?`-style to chain into JSON
    /// decode.
    fn fetch_string<T>(&self, branch: &str, blob_path: &str) -> Result<String, BlobFetchResult<T>> {
        match fetch_blob_safe(&self.repo_root, branch, blob_path) {
            BlobResult::Found(bytes) => String::from_utf8(bytes).map_err(|e| {
                BlobFetchResult::FetchError(format!(
                    "{blob_path}: non-UTF-8 bytes in blob contents: {e}"
                ))
            }),
            BlobResult::NotFound => Err(BlobFetchResult::NotFound),
            BlobResult::CommandFailed(msg) => Err(BlobFetchResult::FetchError(msg)),
        }
    }

    /// Resolves the catalogue filename for `layer_id` by reading
    /// `architecture-rules.json` from the PR branch.
    ///
    /// Returns the explicit `catalogue_file` override if present, the
    /// default `<layer_id>-types.json` when the rules file is absent
    /// (preserving the legacy fallback for non-migrated repos), or an
    /// `Err(msg)` describing a fetch/parse failure when the rules file
    /// is present but unreadable. The fail-closed path prevents a
    /// silent downgrade to the default filename on such failures.
    ///
    /// The error variant is a plain `String` (the diagnostic to embed in a
    /// `BlobFetchResult::FetchError` at the call site). `NotFound` is
    /// absorbed as the legacy fallback, and `Found` is impossible from a
    /// filename lookup, so a generic error carrying those variants would
    /// force callers to handle impossible cases.
    fn resolve_catalogue_filename(&self, branch: &str, layer_id: &str) -> Result<String, String> {
        let text =
            match self.fetch_string::<TypeCatalogueDocument>(branch, "architecture-rules.json") {
                Ok(s) => s,
                Err(BlobFetchResult::NotFound) => {
                    // Legacy fallback: no rules file on the branch → use the
                    // conventional default. This is a NotFound case, not a
                    // fetch failure, so the gate's per-layer NotFound semantic
                    // is still meaningful.
                    return Ok(format!("{layer_id}-types.json"));
                }
                Err(BlobFetchResult::FetchError(msg)) => {
                    // Fetch error on a rules file that exists → fail-closed.
                    return Err(msg);
                }
                Err(BlobFetchResult::Found(_)) => {
                    // fetch_string never returns Err(Found); match exhaustively
                    // to keep the code robust against enum expansion.
                    return Err("internal: fetch_string returned Found in the Err arm".to_owned());
                }
            };
        match super::tddd_layers::parse_tddd_layers(&text) {
            Ok(bindings) => Ok(super::tddd_layers::find_binding(&bindings, layer_id)
                .map(|b| b.catalogue_file().to_owned())
                .unwrap_or_else(|| format!("{layer_id}-types.json"))),
            Err(e) => Err(format!(
                "architecture-rules.json parse error while resolving catalogue file for \
                 layer '{layer_id}': {e}"
            )),
        }
    }
}

impl TrackBlobReader for GitShowTrackBlobReader {
    fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument> {
        let path = Self::blob_path(track_id, "spec.json");
        let text = match self.fetch_string::<SpecDocument>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::spec::codec::decode(&text) {
            Ok(doc) => BlobFetchResult::Found(doc),
            Err(e) => BlobFetchResult::FetchError(format!("{path}: spec.json decode error: {e}")),
        }
    }

    fn read_type_catalogue(
        &self,
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
        // T007: resolve the catalogue filename from the PR branch's
        // `architecture-rules.json` so that layers with an explicit
        // `tddd.catalogue_file` override are handled consistently between
        // the CI path (`verify_from_spec_json`) and the merge gate.
        // Fall back to `<layer_id>-types.json` when the rules file is absent
        // (NotFound); fail-closed when the rules file is present but
        // unreadable or unparseable.
        let filename = match self.resolve_catalogue_filename(branch, layer_id) {
            Ok(name) => name,
            Err(msg) => return BlobFetchResult::FetchError(msg),
        };
        let path = Self::blob_path(track_id, &filename);
        let text = match self.fetch_string::<(TypeCatalogueDocument, String)>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::tddd::catalogue_codec::decode(&text) {
            Ok(doc) => BlobFetchResult::Found((doc, filename)),
            Err(e) => BlobFetchResult::FetchError(format!("{path}: {filename} decode error: {e}")),
        }
    }

    fn read_enabled_layers(&self, branch: &str) -> BlobFetchResult<Vec<String>> {
        // T007: read `architecture-rules.json` from the PR branch blob so
        // that tracks which modify the rules file itself are evaluated
        // against their own layer definitions (not the local workspace).
        // An empty binding list (legacy rules file, or a PR that disables
        // every layer) is returned verbatim — the usecase gate is the
        // fail-closed authority and will reject an empty set explicitly.
        let text = match self.fetch_string::<Vec<String>>(branch, "architecture-rules.json") {
            Ok(s) => s,
            Err(result) => return result,
        };
        let bindings = match super::tddd_layers::parse_tddd_layers(&text) {
            Ok(b) => b,
            Err(e) => {
                return BlobFetchResult::FetchError(format!(
                    "architecture-rules.json parse error: {e}"
                ));
            }
        };
        BlobFetchResult::Found(bindings.iter().map(|b| b.layer_id().to_owned()).collect())
    }

    fn read_track_metadata(&self, branch: &str, track_id: &str) -> BlobFetchResult<TrackMetadata> {
        let path = Self::blob_path(track_id, "metadata.json");
        let text = match self.fetch_string::<TrackMetadata>(branch, &path) {
            Ok(s) => s,
            Err(result) => return result,
        };
        match crate::track::codec::decode(&text) {
            Ok((track, _meta)) => BlobFetchResult::Found(track),
            Err(e) => {
                BlobFetchResult::FetchError(format!("{path}: metadata.json decode error: {e}"))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    // --- Fixture helpers ---

    fn git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("LANGUAGE", "C")
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git command failed to spawn");
        if !output.status.success() {
            panic!(
                "git {:?} failed: stdout={} stderr={}",
                args,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    /// Creates a temp git repo with a track directory containing the
    /// supplied blobs, then sets up a local `origin` remote pointing to
    /// itself so `origin/main:track/items/<id>/<file>` resolves.
    ///
    /// When `files` is empty, a placeholder `.gitkeep` is committed so the
    /// initial commit can succeed (git rejects empty commits by default).
    /// The placeholder lives at the repo root, not in the track dir, so
    /// `track/items/<id>/...` resolves to NotFound as expected by the test.
    fn setup_repo_with_track(track_id: &str, files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);

        if files.is_empty() {
            // Commit a placeholder so the initial commit is non-empty.
            std::fs::write(repo.join(".gitkeep"), b"").unwrap();
            git(repo, &["add", ".gitkeep"]);
        } else {
            let track_dir = repo.join("track/items").join(track_id);
            std::fs::create_dir_all(&track_dir).unwrap();
            for (name, contents) in files {
                std::fs::write(track_dir.join(name), contents).unwrap();
            }
            git(repo, &["add", "track"]);
        }

        git(repo, &["commit", "--quiet", "-m", "initial"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);
        dir
    }

    // --- Spec document fixtures ---

    const SPEC_JSON_MINIMAL: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

    const DOMAIN_TYPES_MINIMAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "test", "approved": true }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true }
  ]
}"#;

    fn metadata_json_minimal(track_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": 3,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test",
  "status": "planned",
  "created_at": "2026-04-12T00:00:00Z",
  "updated_at": "2026-04-12T00:00:00Z",
  "tasks": [
    {{"id": "T001", "description": "Test task", "status": "todo", "commit_hash": null}}
  ],
  "plan": {{
    "summary": ["Test plan"],
    "sections": [
      {{"id": "S1", "title": "Section", "description": ["D"], "task_ids": ["T001"]}}
    ]
  }}
}}"#
        )
    }

    // --- read_spec_document ---

    #[test]
    fn test_read_spec_document_found() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::Found(doc) => {
                assert_eq!(doc.title(), "Feature");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_document_not_found() {
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(reader.read_spec_document("main", "foo"), BlobFetchResult::NotFound));
    }

    #[test]
    fn test_read_spec_document_decode_error() {
        let dir = setup_repo_with_track("foo", &[("spec.json", b"not valid json")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_document_invalid_utf8_fetch_error() {
        // Invalid UTF-8 byte sequence (lone 0xFF)
        let dir = setup_repo_with_track("foo", &[("spec.json", &[0xFF, 0xFE, 0xFD])]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("non-UTF-8"), "{msg}");
            }
            other => panic!("expected FetchError for non-UTF-8, got {other:?}"),
        }
    }

    #[test]
    fn test_read_spec_document_bad_branch_fetch_error() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_spec_document("does-not-exist", "foo") {
            BlobFetchResult::FetchError(_) => {}
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- read_type_catalogue ---

    #[test]
    fn test_read_type_catalogue_found() {
        let dir =
            setup_repo_with_track("foo", &[("domain-types.json", DOMAIN_TYPES_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::Found((doc, filename)) => {
                assert_eq!(doc.entries().len(), 1);
                assert_eq!(filename, "domain-types.json");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_type_catalogue_found_with_custom_catalogue_file_override() {
        // Verify that `architecture-rules.json` at the repo root on the branch
        // with an explicit `tddd.catalogue_file` override is honoured: the
        // adapter must return `Found((doc, "custom-domain-types.json"))`, not
        // the default `domain-types.json`.
        //
        // `setup_repo_with_track` only places files in track/items/<id>/, so
        // we build this fixture manually: `architecture-rules.json` lives at
        // the root and the catalogue lives in track/items/foo/.
        const ARCH_RULES_CUSTOM: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "custom-domain-types.json"
      }
    }
  ]
}"#;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        // Write architecture-rules.json at repo root.
        std::fs::write(repo.join("architecture-rules.json"), ARCH_RULES_CUSTOM).unwrap();
        // Write the custom-named catalogue in the track directory.
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("custom-domain-types.json"), DOMAIN_TYPES_MINIMAL).unwrap();
        git(repo, &["add", "."]);
        git(repo, &["commit", "--quiet", "-m", "initial"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::Found((doc, filename)) => {
                assert_eq!(doc.entries().len(), 1);
                assert_eq!(
                    filename, "custom-domain-types.json",
                    "adapter must return the override filename, not the layer-id default"
                );
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_type_catalogue_not_found() {
        let dir = setup_repo_with_track("foo", &[("spec.json", SPEC_JSON_MINIMAL.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(
            reader.read_type_catalogue("main", "foo", "domain"),
            BlobFetchResult::NotFound
        ));
    }

    #[test]
    fn test_read_type_catalogue_decode_error() {
        let dir = setup_repo_with_track("foo", &[("domain-types.json", b"{}")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- read_track_metadata ---

    #[test]
    fn test_read_track_metadata_found() {
        let metadata = metadata_json_minimal("foo");
        let dir = setup_repo_with_track("foo", &[("metadata.json", metadata.as_bytes())]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_track_metadata("main", "foo") {
            BlobFetchResult::Found(track) => {
                assert_eq!(track.tasks().len(), 1);
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_read_track_metadata_not_found() {
        let dir = setup_repo_with_track("foo", &[]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        assert!(matches!(reader.read_track_metadata("main", "foo"), BlobFetchResult::NotFound));
    }

    #[test]
    fn test_read_track_metadata_decode_error() {
        let dir = setup_repo_with_track("foo", &[("metadata.json", b"not json")]);
        let reader = GitShowTrackBlobReader::new(dir.path().to_path_buf());
        match reader.read_track_metadata("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("decode error"), "{msg}");
            }
            other => panic!("expected FetchError, got {other:?}"),
        }
    }

    // --- Symlink / submodule rejection ---

    #[cfg(unix)]
    #[test]
    fn test_read_spec_document_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        // Create spec.json as a symlink to another file
        std::fs::write(track_dir.join("target.json"), SPEC_JSON_MINIMAL).unwrap();
        std::os::unix::fs::symlink("target.json", track_dir.join("spec.json")).unwrap();
        git(repo, &["add", "track"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_spec_document("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("symlink"), "{msg}");
            }
            other => panic!("expected FetchError(symlink), got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_read_type_catalogue_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("target.json"), DOMAIN_TYPES_MINIMAL).unwrap();
        std::os::unix::fs::symlink("target.json", track_dir.join("domain-types.json")).unwrap();
        git(repo, &["add", "track"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_type_catalogue("main", "foo", "domain") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("symlink"), "{msg}");
            }
            other => panic!("expected FetchError(symlink), got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_read_track_metadata_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        let track_dir = repo.join("track/items/foo");
        std::fs::create_dir_all(&track_dir).unwrap();
        let metadata = metadata_json_minimal("foo");
        std::fs::write(track_dir.join("target.json"), metadata).unwrap();
        std::os::unix::fs::symlink("target.json", track_dir.join("metadata.json")).unwrap();
        git(repo, &["add", "track"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let reader = GitShowTrackBlobReader::new(repo.to_path_buf());
        match reader.read_track_metadata("main", "foo") {
            BlobFetchResult::FetchError(msg) => {
                assert!(msg.contains("symlink"), "{msg}");
            }
            other => panic!("expected FetchError(symlink), got {other:?}"),
        }
    }
}
