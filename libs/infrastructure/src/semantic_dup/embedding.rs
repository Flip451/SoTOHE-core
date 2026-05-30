//! Infrastructure adapter implementing [`usecase::semantic_dup::EmbeddingPort`]
//! via fastembed-rs (ONNX Runtime, synchronous API, Tokio-independent).

use std::path::PathBuf;
use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use usecase::semantic_dup::{EmbeddingError, EmbeddingPort};

// ── FastEmbedAdapter ──────────────────────────────────────────────────────────

/// The fastembed default cache directory name (relative to CWD when
/// `FASTEMBED_CACHE_DIR` is not set).
const FASTEMBED_DEFAULT_CACHE_DIR: &str = ".fastembed_cache";

/// HuggingFace model identifier for the Jina v2 base code model.
///
/// fastembed-rs downloads model weights from HuggingFace Hub and stores them
/// using the hf-hub cache layout:
/// `{cache_dir}/models--{org}--{model}/snapshots/{commit-hash}/`.
/// For `jinaai/jina-embeddings-v2-base-code` the model directory is
/// `models--jinaai--jina-embeddings-v2-base-code`.
const JINA_V2_CODE_MODEL_CODE: &str = "jinaai/jina-embeddings-v2-base-code";

/// The ONNX model file that fastembed fetches into the snapshot subdirectory.
///
/// Under the hf-hub cache, model files reside at
/// `models--{org}--{model}/snapshots/{commit-hash}/onnx/model.onnx`.
const JINA_V2_CODE_MODEL_FILE: &str = "onnx/model.onnx";

/// Tokenizer JSON file required by fastembed's `load_tokenizer_hf_hub`.
///
/// `TextEmbedding::try_new` calls `load_tokenizer_hf_hub`, which fetches
/// `tokenizer.json`, `config.json`, `special_tokens_map.json`, and
/// `tokenizer_config.json` from the hf-hub cache (same snapshot subdirectory
/// as the ONNX file).  All these files must be present for the constructor to
/// stay offline.
const JINA_V2_CODE_TOKENIZER_FILE: &str = "tokenizer.json";

/// Model config JSON file required by fastembed's `load_tokenizer_hf_hub`.
const JINA_V2_CODE_CONFIG_FILE: &str = "config.json";

/// Special tokens map file required by fastembed's `load_tokenizer_hf_hub`.
const JINA_V2_CODE_SPECIAL_TOKENS_FILE: &str = "special_tokens_map.json";

/// Tokenizer config JSON file required by fastembed's `load_tokenizer_hf_hub`.
const JINA_V2_CODE_TOKENIZER_CONFIG_FILE: &str = "tokenizer_config.json";

/// The revision (branch/tag) fastembed uses when pinning the Jina v2 model.
///
/// hf-hub resolves the active commit via `{model_dir}/refs/{revision}`.
/// The file contains the commit hash that the `snapshots/` directory was
/// populated with.  Using the wrong revision causes the preflight to check the
/// wrong snapshot and `TextEmbedding::try_new` to fall through to the network.
const JINA_V2_CODE_REVISION: &str = "main";

/// Infrastructure adapter implementing [`EmbeddingPort`] using fastembed-rs
/// (ONNX Runtime, Jina v2 base code model, synchronous API, Tokio-independent).
/// CN-04: fastembed-rs / ort dependencies are confined to infrastructure.
///
/// # Model Cache
///
/// **CN-01 / offline-only**: this adapter never downloads model weights at
/// runtime.  Before calling `TextEmbedding::try_new`, it verifies that the
/// model ONNX file is already present in the local fastembed cache.  If the
/// file is absent the constructor returns an [`EmbeddingError::ModelLoadFailed`]
/// with a clear "pre-populate the cache" message instead of triggering a
/// network download.
///
/// The cache location is resolved in the same priority order that fastembed's
/// internal `pull_from_hf` uses:
///
/// 1. `HF_HOME` environment variable: if set (even to an empty string), uses
///    its value directly as the cache root (fastembed's `pull_from_hf` treats
///    any set `HF_HOME` as authoritative and overrides `cache_dir`), or
/// 2. `FASTEMBED_CACHE_DIR` environment variable: explicit fastembed override,
///    or
/// 3. the default fastembed cache: `.fastembed_cache` in the current working
///    directory.
///
/// The cache is considered ready when all of the following exist under the
/// cache root (revision pinned via `refs/main`):
/// - `models--jinaai--jina-embeddings-v2-base-code/snapshots/{commit}/onnx/model.onnx`
/// - `models--jinaai--jina-embeddings-v2-base-code/snapshots/{commit}/tokenizer.json`
/// - `models--jinaai--jina-embeddings-v2-base-code/snapshots/{commit}/config.json`
/// - `models--jinaai--jina-embeddings-v2-base-code/snapshots/{commit}/special_tokens_map.json`
/// - `models--jinaai--jina-embeddings-v2-base-code/snapshots/{commit}/tokenizer_config.json`
///
/// To pre-populate the cache, run (once, with network access):
/// ```text
/// FASTEMBED_CACHE_DIR=/path/to/cache sotp dup-index build --db-path /tmp/seed ...
/// ```
/// or use the fastembed Python library / any other tool that downloads from
/// `jinaai/jina-embeddings-v2-base-code` on HuggingFace Hub.
///
/// In CI, set `FASTEMBED_CACHE_DIR` to a pre-populated volume mount.
pub struct FastEmbedAdapter {
    /// Wrapped in `Mutex` because `TextEmbedding::embed` takes `&mut self`.
    model: Mutex<TextEmbedding>,
}

impl std::fmt::Debug for FastEmbedAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedAdapter").finish_non_exhaustive()
    }
}

/// Resolve the fastembed cache directory, matching the priority order used by
/// fastembed's `pull_from_hf` internally:
///
/// 1. `HF_HOME` (if the variable is set — even to an empty string — fastembed's
///    `pull_from_hf` uses its value directly as the cache root, overriding the
///    `cache_dir` value passed to it),
/// 2. `FASTEMBED_CACHE_DIR` (if set — any value, including empty — treated as
///    authoritative, matching fastembed's own behaviour), or
/// 3. `.fastembed_cache` in the current working directory (fastembed default).
///
/// Both `FASTEMBED_CACHE_DIR` and `HF_HOME` are read here so that the offline
/// preflight checks the same directory that `TextEmbedding::try_new` will load
/// from, preventing a mismatch where the preflight passes but the loader then
/// reaches out to a different (empty) path.
fn resolve_cache_dir() -> PathBuf {
    // Priority 1: HF_HOME — fastembed's pull_from_hf treats any *set* HF_HOME
    // as authoritative, even if it is an empty string.  Mirror that exactly:
    // use the env-var value as-is without an is_empty() guard.
    if let Ok(hf_home) = std::env::var("HF_HOME") {
        return PathBuf::from(hf_home);
    }
    // Priority 2: FASTEMBED_CACHE_DIR — explicit fastembed override.
    // Like HF_HOME, treat any set value (even empty string) as authoritative.
    if let Ok(v) = std::env::var("FASTEMBED_CACHE_DIR") {
        return PathBuf::from(v);
    }
    // Priority 3: fastembed default — ".fastembed_cache" in CWD.
    PathBuf::from(FASTEMBED_DEFAULT_CACHE_DIR)
}

/// Return the model root directory for `model_code` under `cache_dir`.
///
/// The hf-hub cache layout is:
/// `{cache_dir}/models--{org}--{model}/`
fn model_root_dir(cache_dir: &std::path::Path, model_code: &str) -> PathBuf {
    // "jinaai/jina-embeddings-v2-base-code"
    //   → "models--jinaai--jina-embeddings-v2-base-code"
    let dir_name = format!("models--{}", model_code.replace('/', "--"));
    cache_dir.join(dir_name)
}

/// Construct the `snapshots/` directory path for `model_code` under `cache_dir`.
///
/// The hf-hub cache layout stores model files at:
/// `{cache_dir}/models--{org}--{model}/snapshots/{commit-hash}/{files}`
///
/// Because the commit hash is not known at preflight time, this function
/// returns the `snapshots/` parent directory.  Use
/// [`model_onnx_exists_in_cache`] to check whether the revision-selected
/// snapshot contains the required model files.
fn model_snapshots_dir(cache_dir: &std::path::Path, model_code: &str) -> PathBuf {
    model_root_dir(cache_dir, model_code).join("snapshots")
}

/// Return `true` when `model_code`'s required model files are present in the
/// revision-selected snapshot under `cache_dir`.
///
/// The hf-hub cache stores the active commit hash in:
/// `{cache_dir}/models--{org}--{model}/refs/{revision}`
///
/// This function reads that file to find the pinned commit, then checks that:
/// - `snapshots/{commit}/onnx/model.onnx` exists (ONNX weights), and
/// - `snapshots/{commit}/tokenizer.json` exists, and
/// - `snapshots/{commit}/config.json` exists, and
/// - `snapshots/{commit}/special_tokens_map.json` exists, and
/// - `snapshots/{commit}/tokenizer_config.json` exists
///
/// (the last four files are required by fastembed's `load_tokenizer_hf_hub`
/// call inside `TextEmbedding::try_new`).
///
/// Using the revision-selected snapshot (rather than "any snapshot") ensures
/// the preflight matches the exact directory that `TextEmbedding::try_new`
/// will load from, preventing a false-positive pass on a stale cached snapshot.
///
/// Returns `false` if `refs/{revision}` is absent, unreadable, or if any
/// required file is missing in the pinned snapshot.
fn model_onnx_exists_in_cache(cache_dir: &std::path::Path, model_code: &str) -> bool {
    let model_root = model_root_dir(cache_dir, model_code);
    // Read the pinned commit hash from refs/{revision}.
    let refs_file = model_root.join("refs").join(JINA_V2_CODE_REVISION);
    let commit_hash = match std::fs::read_to_string(&refs_file) {
        Ok(s) => s.trim().to_owned(),
        Err(_) => return false,
    };
    if commit_hash.is_empty() {
        return false;
    }
    // Verify the revision-selected snapshot has the ONNX weights and all
    // tokenizer-related files that fastembed's load_tokenizer_hf_hub fetches.
    let snapshot_dir = model_root.join("snapshots").join(&commit_hash);
    snapshot_dir.join(JINA_V2_CODE_MODEL_FILE).exists()
        && snapshot_dir.join(JINA_V2_CODE_TOKENIZER_FILE).exists()
        && snapshot_dir.join(JINA_V2_CODE_CONFIG_FILE).exists()
        && snapshot_dir.join(JINA_V2_CODE_SPECIAL_TOKENS_FILE).exists()
        && snapshot_dir.join(JINA_V2_CODE_TOKENIZER_CONFIG_FILE).exists()
}

impl FastEmbedAdapter {
    /// Load the Jina v2 base code model and create a new [`FastEmbedAdapter`].
    ///
    /// # Offline preflight (CN-01)
    ///
    /// Before initialising the model, this constructor checks that the ONNX
    /// weights are already present in the local fastembed cache.  If they are
    /// not found it returns an error with a message explaining how to
    /// pre-populate the cache — it does NOT initiate a network download.
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError::ModelLoadFailed`] if:
    /// - the model ONNX file is not found in the local cache (offline
    ///   preflight failure — no network download is attempted), or
    /// - the model fails to load or initialise (e.g. ONNX Runtime error or
    ///   invalid model weights).
    pub fn new() -> Result<Self, EmbeddingError> {
        let cache_dir = resolve_cache_dir();
        Self::new_with_cache_dir(cache_dir)
    }

    /// Internal constructor that accepts an explicit `cache_dir`.
    ///
    /// Separated from [`FastEmbedAdapter::new`] so that unit tests can supply a
    /// temporary directory without setting process-global environment variables
    /// (which require `unsafe` in Rust 2024 edition).
    fn new_with_cache_dir(cache_dir: PathBuf) -> Result<Self, EmbeddingError> {
        // CN-01 offline preflight: fail fast if the model is not fully cached.
        // The hf-hub layout requires:
        //   {cache_dir}/models--{org}--{model}/refs/main            (commit hash)
        //   {cache_dir}/models--{org}--{model}/snapshots/{commit}/onnx/model.onnx
        //   {cache_dir}/models--{org}--{model}/snapshots/{commit}/tokenizer.json
        //   {cache_dir}/models--{org}--{model}/snapshots/{commit}/config.json
        //   {cache_dir}/models--{org}--{model}/snapshots/{commit}/special_tokens_map.json
        //   {cache_dir}/models--{org}--{model}/snapshots/{commit}/tokenizer_config.json
        // We resolve the active snapshot via refs/main so we check the exact
        // directory that TextEmbedding::try_new will load from.
        if !model_onnx_exists_in_cache(&cache_dir, JINA_V2_CODE_MODEL_CODE) {
            let snapshots = model_snapshots_dir(&cache_dir, JINA_V2_CODE_MODEL_CODE);
            return Err(EmbeddingError::ModelLoadFailed {
                source: format!(
                    "model not found in local cache \
                     (expected onnx/model.onnx, tokenizer.json, config.json, \
                     special_tokens_map.json, and tokenizer_config.json in the refs/main \
                     snapshot under {}); pre-populate it — this tool does not download models \
                     at runtime. Set FASTEMBED_CACHE_DIR to point at a pre-populated cache \
                     directory, or use the fastembed Python library / HuggingFace Hub to \
                     download '{}' into '{}'.",
                    snapshots.display(),
                    JINA_V2_CODE_MODEL_CODE,
                    cache_dir.display(),
                ),
            });
        }

        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::JinaEmbeddingsV2BaseCode)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(false),
        )
        .map_err(|e| EmbeddingError::ModelLoadFailed { source: e.to_string() })?;

        Ok(Self { model: Mutex::new(model) })
    }
}

impl EmbeddingPort for FastEmbedAdapter {
    /// Compute an embedding vector for the given code fragment using the Jina
    /// v2 base code ONNX model (synchronous, Tokio-independent).
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError::InferenceFailed`] if ONNX inference fails or
    /// if the internal mutex is poisoned.
    fn embed(
        &self,
        fragment: &domain::semantic_dup::CodeFragment,
    ) -> Result<Vec<f32>, EmbeddingError> {
        let mut model = self.model.lock().map_err(|e| EmbeddingError::InferenceFailed {
            source: format!("model mutex poisoned: {e}"),
        })?;

        let texts = vec![fragment.content()];
        let mut results = model
            .embed(texts, None)
            .map_err(|e| EmbeddingError::InferenceFailed { source: e.to_string() })?;

        results.pop().ok_or_else(|| EmbeddingError::InferenceFailed {
            source: "fastembed returned an empty embedding batch".to_owned(),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    // ── model_snapshots_dir ───────────────────────────────────────────────────

    #[test]
    fn test_model_snapshots_dir_constructs_correct_hf_hub_path() {
        let cache_dir = std::path::Path::new("/some/cache");
        let path = model_snapshots_dir(cache_dir, JINA_V2_CODE_MODEL_CODE);
        // HuggingFace hub replaces '/' with '--' and prepends 'models--',
        // then places model files under a 'snapshots/' subdirectory.
        assert_eq!(
            path,
            std::path::PathBuf::from(
                "/some/cache/models--jinaai--jina-embeddings-v2-base-code/snapshots"
            ),
            "model_snapshots_dir must construct the correct hf-hub snapshots path"
        );
    }

    #[test]
    fn test_model_snapshots_dir_ends_with_snapshots_segment() {
        let cache_dir = std::path::Path::new("/cache");
        let path = model_snapshots_dir(cache_dir, JINA_V2_CODE_MODEL_CODE);
        assert!(
            path.ends_with("snapshots"),
            "path must end with 'snapshots', got: {}",
            path.display()
        );
    }

    // ── model_onnx_exists_in_cache ────────────────────────────────────────────
    //
    // The `#![forbid(unsafe_code)]` crate attribute prevents us from calling
    // `std::env::set_var` / `remove_var` (which are `unsafe` in Rust 2024)
    // inside tests, so we test the preflight logic via the helper functions
    // that underpin it, rather than via `FastEmbedAdapter::new()` directly.
    //
    // Key behaviours verified:
    //   (a) `model_snapshots_dir` produces the correct base path (above).
    //   (b) `model_onnx_exists_in_cache` returns false when refs/main is absent.
    //   (c) `model_onnx_exists_in_cache` returns true when refs/main points to a
    //       snapshot that has all five required files.
    //   (d) `model_onnx_exists_in_cache` returns false when onnx file is missing.
    //   (e) `model_onnx_exists_in_cache` returns false when tokenizer.json is missing.
    //   (f) `model_onnx_exists_in_cache` returns false when config.json is missing.
    //   (g) `model_onnx_exists_in_cache` returns false when special_tokens_map.json missing.
    //   (h) `model_onnx_exists_in_cache` returns false when tokenizer_config.json missing.
    //
    // Full end-to-end verification (that `FastEmbedAdapter::new()` returns
    // `Err` when the env var points at an empty dir) is covered by the
    // `new_with_cache_dir` tests below, which supply the cache dir directly.

    /// Helper: create the hf-hub cache layout for the Jina model.
    ///
    /// Creates `{cache}/models--jinaai--jina-embeddings-v2-base-code/refs/main`
    /// (containing `commit`) and the files listed in `snapshot_files` under
    /// `snapshots/{commit}/`.
    fn create_test_cache(cache: &std::path::Path, commit: &str, snapshot_files: &[&str]) {
        let model_root = model_root_dir(cache, JINA_V2_CODE_MODEL_CODE);
        // Write refs/main with the commit hash.
        let refs_dir = model_root.join("refs");
        std::fs::create_dir_all(&refs_dir).unwrap();
        std::fs::write(refs_dir.join(JINA_V2_CODE_REVISION), commit).unwrap();
        // Create each requested file in the snapshot directory.
        let snapshot_dir = model_root.join("snapshots").join(commit);
        for file in snapshot_files {
            let file_path = snapshot_dir.join(file);
            std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
            std::fs::write(&file_path, b"").unwrap();
        }
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_for_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        // Fresh temp dir has no model files.
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when the cache has no model snapshot"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_when_refs_main_is_missing() {
        // Snapshot exists but refs/main is absent.
        let dir = tempfile::tempdir().unwrap();
        let snapshots = model_snapshots_dir(dir.path(), JINA_V2_CODE_MODEL_CODE);
        let commit_dir = snapshots.join("abc123deadbeef");
        let onnx_path = commit_dir.join(JINA_V2_CODE_MODEL_FILE);
        std::fs::create_dir_all(onnx_path.parent().unwrap()).unwrap();
        std::fs::write(&onnx_path, b"").unwrap();
        // Without refs/main the preflight cannot identify the active snapshot.
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when refs/main is absent (cannot resolve active snapshot)"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_true_when_all_required_files_present() {
        // Full hf-hub layout with refs/main → snapshot with all required files.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(
            dir.path(),
            "abc123deadbeef",
            &[
                JINA_V2_CODE_MODEL_FILE,
                JINA_V2_CODE_TOKENIZER_FILE,
                JINA_V2_CODE_CONFIG_FILE,
                JINA_V2_CODE_SPECIAL_TOKENS_FILE,
                JINA_V2_CODE_TOKENIZER_CONFIG_FILE,
            ],
        );
        assert!(
            model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return true when refs/main points to a snapshot with all required files"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_when_onnx_file_missing() {
        // refs/main is present but the ONNX file is absent.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(dir.path(), "abc123deadbeef", &[JINA_V2_CODE_TOKENIZER_FILE]);
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when onnx/model.onnx is missing from the pinned snapshot"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_when_tokenizer_file_missing() {
        // refs/main is present but tokenizer.json is absent; all other files present.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(
            dir.path(),
            "abc123deadbeef",
            &[
                JINA_V2_CODE_MODEL_FILE,
                JINA_V2_CODE_CONFIG_FILE,
                JINA_V2_CODE_SPECIAL_TOKENS_FILE,
                JINA_V2_CODE_TOKENIZER_CONFIG_FILE,
            ],
        );
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when tokenizer.json is missing from the pinned snapshot"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_when_config_file_missing() {
        // refs/main is present but config.json is absent; all other files present.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(
            dir.path(),
            "abc123deadbeef",
            &[
                JINA_V2_CODE_MODEL_FILE,
                JINA_V2_CODE_TOKENIZER_FILE,
                JINA_V2_CODE_SPECIAL_TOKENS_FILE,
                JINA_V2_CODE_TOKENIZER_CONFIG_FILE,
            ],
        );
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when config.json is missing from the pinned snapshot"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_when_special_tokens_map_missing() {
        // refs/main is present but special_tokens_map.json is absent.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(
            dir.path(),
            "abc123deadbeef",
            &[
                JINA_V2_CODE_MODEL_FILE,
                JINA_V2_CODE_TOKENIZER_FILE,
                JINA_V2_CODE_CONFIG_FILE,
                JINA_V2_CODE_TOKENIZER_CONFIG_FILE,
            ],
        );
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when special_tokens_map.json is missing from the pinned snapshot"
        );
    }

    #[test]
    fn test_model_onnx_exists_in_cache_returns_false_when_tokenizer_config_missing() {
        // refs/main is present but tokenizer_config.json is absent.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(
            dir.path(),
            "abc123deadbeef",
            &[
                JINA_V2_CODE_MODEL_FILE,
                JINA_V2_CODE_TOKENIZER_FILE,
                JINA_V2_CODE_CONFIG_FILE,
                JINA_V2_CODE_SPECIAL_TOKENS_FILE,
            ],
        );
        assert!(
            !model_onnx_exists_in_cache(dir.path(), JINA_V2_CODE_MODEL_CODE),
            "must return false when tokenizer_config.json is missing from the pinned snapshot"
        );
    }

    // ── FastEmbedAdapter::new_with_cache_dir offline preflight ────────────────
    //
    // `FastEmbedAdapter::new()` is a two-step wrapper:
    //   1. `resolve_cache_dir()` — reads env vars (`HF_HOME`, `FASTEMBED_CACHE_DIR`)
    //      in priority order, falling back to `.fastembed_cache` in CWD.
    //   2. `new_with_cache_dir(path)` — runs the offline preflight and loads.
    //
    // A hermetic unit test of `new()` via env vars requires `std::env::set_var` /
    // `remove_var`, which are `unsafe` in Rust 2024 edition.  Because this crate
    // carries `#![forbid(unsafe_code)]`, the env-var branches of step 1 cannot be
    // injected in tests.  The tests below call `new_with_cache_dir` directly with
    // an explicit `PathBuf`, exercising step 2 (the offline preflight)
    // deterministically.  Step 1 is a simple env-var read with no preflight logic.
    //
    // Note: `FastEmbedAdapter::new()` itself is NOT directly tested here because
    // the `#![forbid(unsafe_code)]` attribute prevents setting env vars to provide
    // hermetic test conditions.  The offline-preflight correctness is fully covered
    // by the `new_with_cache_dir` tests below.

    #[test]
    fn test_new_with_cache_dir_returns_error_when_cache_is_empty() {
        // An empty temp dir has no model weights — `new_with_cache_dir` must
        // return `EmbeddingError::ModelLoadFailed` without accessing the network.
        let dir = tempfile::tempdir().unwrap();
        let result = FastEmbedAdapter::new_with_cache_dir(dir.path().to_path_buf());
        assert!(
            matches!(result, Err(EmbeddingError::ModelLoadFailed { .. })),
            "new_with_cache_dir must return ModelLoadFailed when cache is empty"
        );
    }

    #[test]
    fn test_new_with_cache_dir_returns_error_when_refs_main_missing() {
        // Snapshot exists but refs/main is absent — preflight must fail.
        let dir = tempfile::tempdir().unwrap();
        let snapshots = model_snapshots_dir(dir.path(), JINA_V2_CODE_MODEL_CODE);
        let commit_dir = snapshots.join("abc123deadbeef");
        let onnx_path = commit_dir.join(JINA_V2_CODE_MODEL_FILE);
        std::fs::create_dir_all(onnx_path.parent().unwrap()).unwrap();
        std::fs::write(&onnx_path, b"").unwrap();
        let result = FastEmbedAdapter::new_with_cache_dir(dir.path().to_path_buf());
        assert!(
            matches!(result, Err(EmbeddingError::ModelLoadFailed { .. })),
            "new_with_cache_dir must return ModelLoadFailed when refs/main is absent"
        );
    }

    #[test]
    fn test_new_with_cache_dir_returns_error_when_only_onnx_present() {
        // refs/main exists and points to a snapshot with only the ONNX file;
        // all tokenizer-related files are absent — preflight must fail.
        let dir = tempfile::tempdir().unwrap();
        create_test_cache(dir.path(), "abc123deadbeef", &[JINA_V2_CODE_MODEL_FILE]);
        let result = FastEmbedAdapter::new_with_cache_dir(dir.path().to_path_buf());
        assert!(
            matches!(result, Err(EmbeddingError::ModelLoadFailed { .. })),
            "new_with_cache_dir must return ModelLoadFailed when tokenizer files are absent"
        );
    }
}
