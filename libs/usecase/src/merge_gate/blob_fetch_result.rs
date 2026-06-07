//! Port-level blob fetch result type for the merge gate usecase.
//!
//! `BlobFetchResult` is extracted from `merge_gate` to keep the parent module
//! under the 700-line production-code limit.

/// Result of a port-level blob fetch.
///
/// Infrastructure adapters translate their native errors (git spawn errors,
/// UTF-8 decode errors, JSON decode errors, non-path-not-found git errors)
/// into [`BlobFetchResult::FetchError`], and path-not-found cases into
/// [`BlobFetchResult::NotFound`] so the usecase can apply opt-in semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobFetchResult<T> {
    /// The blob was found and decoded into a domain document.
    Found(T),
    /// The blob does not exist at the requested path on the target ref.
    NotFound,
    /// An I/O, decode, or adapter-level error occurred. The string carries
    /// a human-readable description for the caller's error output.
    FetchError(String),
}
