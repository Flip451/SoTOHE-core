//! Use-case layer for semantic duplicate detection.
//!
//! Ports, error types, command/output types, application-service traits, and
//! interactors for the discoverability soft-gate feature
//! (ADR 2026-05-29-1118-semantic-dup-detection-discoverability-gate).
//!
//! Ports are placed here (not in domain) because embedding and vector-index
//! capabilities are infrastructure concerns — the domain carries no concept of
//! ML inference or ANN search. Analogous to `ReviewHasher`.

mod command;
mod errors;
mod interactor;
mod ports;

// ── Public re-exports — preserve the `usecase::semantic_dup::X` paths ─────────

pub use command::{
    BuildIndexCommand, BuildIndexOutput, DupCheckCommand, DupCheckOutput, DupCheckWarning,
    FindSimilarCommand, FindSimilarOutput, MeasureQualityCommand, QualityMetrics,
};

pub use errors::{
    BuildIndexError, DupCheckError, EmbeddingError, FindSimilarError, MeasureQualityError,
    SemanticIndexError,
};

pub use interactor::{
    BuildIndexInteractor, BuildIndexService, DupCheckInteractor, DupCheckService,
    FindSimilarInteractor, FindSimilarService, MeasureQualityInteractor, MeasureQualityService,
};

pub use ports::{EmbeddingPort, SemanticIndexPort};
