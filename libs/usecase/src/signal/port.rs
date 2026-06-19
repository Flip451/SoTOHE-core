//! Secondary port for the argless signal orchestrator functions.
//!
//! Declared here so that `libs/usecase` defines the contract; implemented by a
//! local filesystem adapter in `libs/infrastructure`.

use std::fmt;

use domain::TrackId;
use domain::tddd::LayerId;

// ── Error type ────────────────────────────────────────────────────────────────

/// Error returned by [`SignalLayerReader`] port methods.
///
/// Intentionally payload-free: the usecase layer must not expose filesystem
/// paths, hostnames, or OS-level details through its port contract.
///
/// - `Io` — a sanitized filesystem / OS failure from the local adapter.
/// - `TrackIdUnresolved` — the active-track ID could not be determined (e.g.
///   no active `track/…` branch found in the current repository).
#[derive(Debug)]
pub enum SignalLayerReaderError {
    /// A filesystem or OS-level failure occurred in the adapter.
    Io,
    /// The active-track ID could not be resolved from the current branch.
    TrackIdUnresolved,
}

impl fmt::Display for SignalLayerReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io => write!(f, "I/O error while reading signal layer data"),
            Self::TrackIdUnresolved => {
                write!(f, "could not resolve active track ID from the current git branch")
            }
        }
    }
}

impl std::error::Error for SignalLayerReaderError {}

// ── Secondary port trait ──────────────────────────────────────────────────────

/// Secondary port for the argless signal orchestrator functions.
///
/// Provides:
/// - `active_track_id()` — resolves the active-track ID from the current branch.
/// - `enabled_layers(track_id)` — enumerates TDDD-enabled layer IDs.
/// - `catalogue_bytes(track_id, layer)` — reads raw catalogue file bytes for a
///   layer (`None` when the file is absent; the orchestrator skips it).
///
/// The usecase orchestrator calls `active_track_id()` first, then passes the
/// resolved `TrackId` explicitly to `enabled_layers` and `catalogue_bytes`.
/// No filesystem path is exposed through this port — path construction is an
/// infrastructure responsibility (D8-4).
pub trait SignalLayerReader: Send + Sync {
    /// Resolve the active-track `TrackId` from the current git branch.
    ///
    /// # Errors
    ///
    /// Returns [`SignalLayerReaderError::TrackIdUnresolved`] when no active
    /// `track/…` branch can be found, or [`SignalLayerReaderError::Io`] on a
    /// lower-level failure.
    fn active_track_id(&self) -> Result<TrackId, SignalLayerReaderError>;

    /// Return the list of TDDD-enabled layer IDs for the given track.
    ///
    /// The list preserves the declaration order from `architecture-rules.json`.
    ///
    /// # Errors
    ///
    /// Returns [`SignalLayerReaderError::Io`] when `architecture-rules.json`
    /// cannot be read or parsed.
    fn enabled_layers(&self, track_id: TrackId) -> Result<Vec<LayerId>, SignalLayerReaderError>;

    /// Return the raw bytes of `<layer>-types.json` for the given track.
    ///
    /// Returns `Ok(None)` when the catalogue file does not exist (the
    /// orchestrator skips that layer without error).
    ///
    /// # Errors
    ///
    /// Returns [`SignalLayerReaderError::Io`] on any I/O failure other than
    /// `NotFound`.
    fn catalogue_bytes(
        &self,
        track_id: TrackId,
        layer: LayerId,
    ) -> Result<Option<Vec<u8>>, SignalLayerReaderError>;
}
