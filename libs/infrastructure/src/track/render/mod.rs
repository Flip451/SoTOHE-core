//! Rendering and sync of track read-only views (`plan.md`, `registry.md`, `spec.md`, `domain-types.md`) from metadata.json / spec.json / domain-types.json.

mod contract_map;
mod plan;
mod registry;
mod snapshot;
mod sync;
mod validate;

use std::path::PathBuf;

// Re-export codec so tests can use `codec::decode` via `use super::*`.
pub(crate) use super::codec;

// Re-export public items for use by the rest of `infrastructure`.
pub use plan::render_plan;
pub use registry::render_registry;
pub use snapshot::{TrackSnapshot, collect_track_snapshots};
pub use sync::sync_rendered_views;
pub use validate::validate_track_snapshots;

// Re-export for the adapter_render_rejects_symlinked_style_config test which
// references `super::ContractMapRendererAdapter`.
pub use crate::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;

// Re-export internal helpers used in tests (via `use super::*`).
// These are `pub(crate)` so they don't form part of the public API surface
// but are visible to the test module included via `#[path]`.
#[cfg(test)]
pub(crate) use snapshot::{decode_legacy_metadata, validate_track_document};

// Constants shared across submodules.
pub(super) const TRACK_ITEMS_DIR: &str = "track/items";
pub(super) const TRACK_ARCHIVE_DIR: &str = "track/archive";

/// Returns `true` when `actual` and `expected` have identical content, allowing
/// for a trailing newline difference.
///
/// This normalisation prevents spurious re-renders when a file was written with
/// a trailing `\n` that the renderer omits (or vice-versa).
pub(super) fn rendered_matches(actual: &str, expected: &str) -> bool {
    actual == expected || actual.trim_end_matches('\n') == expected.trim_end_matches('\n')
}
pub(super) const VALID_TRACK_STATUSES: &[&str] =
    &["planned", "in_progress", "done", "blocked", "cancelled", "archived"];

/// Error while collecting or syncing rendered views.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid metadata at {path}: {source}")]
    InvalidMetadata {
        path: PathBuf,
        #[source]
        source: codec::CodecError,
    },

    #[error("rendered view out of sync at {path}: {reason}")]
    OutOfSync { path: PathBuf, reason: String },

    #[error("unsupported schema_version {schema_version} at {path}")]
    UnsupportedSchemaVersion { path: PathBuf, schema_version: u32 },

    #[error("invalid track metadata at {path}: {reason}")]
    InvalidTrackMetadata { path: PathBuf, reason: String },
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "../render_tests.rs"]
mod tests;
