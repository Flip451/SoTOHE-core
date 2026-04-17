//! Secondary ports for the Contract Map pipeline.
//!
//! `CatalogueLoader` loads per-layer type catalogues from a durable store
//! and returns them together with a topologically-sorted layer order.
//! `ContractMapWriter` persists a rendered [`ContractMapContent`] to a
//! track directory. Both ports are defined in the domain layer per the
//! hexagonal convention `knowledge/conventions/hexagonal-architecture.md`
//! (persistence-related ports live in domain) and are implemented in
//! `libs/infrastructure/src/tddd/contract_map_adapter.rs` (T004).
//!
//! Type inventory matches the TDDD declarations in
//! `track/items/tddd-contract-map-phase1-2026-04-17/domain-types.json`:
//! two secondary_port traits plus two error enums. No serde derives are
//! attached here — ADR 2026-04-14-1531 forbids serde inside
//! `libs/domain`, and the wire format lives in infrastructure codecs.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::TrackId;
use crate::tddd::LayerId;
use crate::tddd::catalogue::TypeCatalogueDocument;
use crate::tddd::contract_map_content::ContractMapContent;

/// Secondary port that loads every `tddd.enabled` layer's catalogue for a
/// given track.
///
/// Returns a tuple of `(layer_order, catalogues)` where `layer_order` is
/// the `may_depend_on`-based topological sort (dependency-less layers
/// first) and `catalogues` maps each layer to its decoded
/// [`TypeCatalogueDocument`]. Implementations must preserve the
/// `Vec<LayerId>` ordering — downstream Contract Map rendering relies on
/// it to produce stable, left-to-right subgraph layout.
pub trait CatalogueLoader: Send + Sync {
    /// Load all enabled-layer catalogues for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueLoaderError`] if the architecture rules cannot
    /// be discovered, a catalogue file is missing or reached through a
    /// rejected symlink, a catalogue fails to decode, or the
    /// dependency graph cannot be topologically sorted.
    fn load_all(
        &self,
        track_id: &TrackId,
    ) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), CatalogueLoaderError>;
}

/// Secondary port that persists a rendered Contract Map to the track
/// directory at `track/items/<track_id>/contract-map.md`.
///
/// Implementations must write atomically and refuse to follow symlinks
/// below the caller-supplied trust root (see
/// `knowledge/conventions/security.md` §Symlink Rejection).
pub trait ContractMapWriter: Send + Sync {
    /// Persist `content` for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`ContractMapWriterError`] if the track directory is
    /// missing, the target path is reached through a symlink, or the
    /// underlying I/O fails.
    fn write(
        &self,
        track_id: &TrackId,
        content: &ContractMapContent,
    ) -> Result<(), ContractMapWriterError>;
}

/// Error variants returned by `CatalogueLoader` implementations.
///
/// Variant inventory matches `domain-types.json` for the
/// `tddd-contract-map-phase1-2026-04-17` track.
#[derive(Debug, thiserror::Error)]
pub enum CatalogueLoaderError {
    /// A layer's catalogue file is absent on disk (fail-closed — no skip).
    #[error("catalogue file not found for layer '{layer_id}' at {}", .path.display())]
    CatalogueNotFound { layer_id: String, path: PathBuf },

    /// `architecture-rules.json` could not be read, parsed, or otherwise
    /// produced the layer binding list.
    #[error("layer discovery failed: {reason}")]
    LayerDiscoveryFailed { reason: String },

    /// A catalogue file failed to decode (JSON schema / L1 validation).
    #[error("failed to decode catalogue for layer '{layer_id}': {reason}")]
    DecodeFailed { layer_id: String, reason: String },

    /// A path under the trust root is (or passes through) a symlink and
    /// must be rejected.
    #[error("symlink rejected at {}", .path.display())]
    SymlinkRejected { path: PathBuf },

    /// A non-symlink I/O failure occurred while reading a catalogue
    /// artifact.
    #[error("I/O error at {}: {reason}", .path.display())]
    IoError { path: PathBuf, reason: String },

    /// `may_depend_on` forms a cycle among enabled layers.
    #[error("topological sort failed: {reason}")]
    TopologicalSortFailed { reason: String },
}

/// Error variants returned by `ContractMapWriter` implementations.
///
/// Variant inventory matches `domain-types.json` for the
/// `tddd-contract-map-phase1-2026-04-17` track.
#[derive(Debug, thiserror::Error)]
pub enum ContractMapWriterError {
    /// Non-symlink I/O failure during the atomic write.
    #[error("I/O error at {}: {reason}", .path.display())]
    IoError { path: PathBuf, reason: String },

    /// The write target or an intermediate component is a symlink; the
    /// write was refused fail-closed.
    #[error("symlink rejected at {}", .path.display())]
    SymlinkRejected { path: PathBuf },

    /// The track directory does not exist (e.g. the caller passed an
    /// unknown track id).
    #[error("track '{track_id}' not found (expected directory: {})", .expected_dir.display())]
    TrackNotFound { track_id: String, expected_dir: PathBuf },
}
