//! Infrastructure adapter implementing [`usecase::semantic_dup::EmbeddingPort`]
//! via fastembed-rs (ONNX Runtime, synchronous API, Tokio-independent).

use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use usecase::semantic_dup::{EmbeddingError, EmbeddingPort};

// ── FastEmbedAdapter ──────────────────────────────────────────────────────────

/// Infrastructure adapter implementing [`EmbeddingPort`] using fastembed-rs
/// (ONNX Runtime, Jina v2 base code model, synchronous API, Tokio-independent).
/// CN-04: fastembed-rs / ort dependencies are confined to infrastructure.
///
/// # Model Cache
///
/// At first construction, fastembed-rs downloads the Jina v2 base code model
/// weights (~550 MB) from Hugging Face Hub and stores them in the local model
/// cache. The cache location is controlled by:
///
/// - `FASTEMBED_CACHE_DIR` environment variable (highest priority), or
/// - the default fastembed cache: `.fastembed_cache` in the current working
///   directory.
///
/// In CI, set `FASTEMBED_CACHE_DIR` to a pre-populated volume mount so that
/// no network access is required at runtime. No network call occurs at compile
/// time; only the first runtime construction of this adapter triggers a
/// download (if the cache is empty).
pub struct FastEmbedAdapter {
    /// Wrapped in `Mutex` because `TextEmbedding::embed` takes `&mut self`.
    model: Mutex<TextEmbedding>,
}

impl std::fmt::Debug for FastEmbedAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedAdapter").finish_non_exhaustive()
    }
}

impl FastEmbedAdapter {
    /// Load the Jina v2 base code model and create a new [`FastEmbedAdapter`].
    ///
    /// # Errors
    ///
    /// Returns [`EmbeddingError::ModelLoadFailed`] if the model fails to load
    /// or initialise (e.g. ONNX Runtime error, invalid model weights, or a
    /// network error when the model cache is empty and the download fails).
    pub fn new() -> Result<Self, EmbeddingError> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::JinaEmbeddingsV2BaseCode)
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
