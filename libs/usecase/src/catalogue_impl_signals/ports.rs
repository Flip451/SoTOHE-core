//! Secondary ports for run-wide catalogue implementation-signals inputs.
//!
//! The evaluation-start capture is a usecase concern because it authorizes one
//! complete evaluation run rather than a particular rustdoc crate. Concrete
//! filesystem and toolchain access remains in `libs/infrastructure`.

use std::fmt;

use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::type_signals_doc::ImplementationFingerprint;

// ---------------------------------------------------------------------------
// EvaluationStartCaptureError
// ---------------------------------------------------------------------------

/// Error returned when the complete run-wide evaluation-start fingerprint
/// cannot be acquired authoritatively.
#[derive(Debug)]
pub enum EvaluationStartCaptureError {
    /// The evaluation inputs were incomplete, unavailable, or otherwise
    /// unverifiable.
    AuthoritativeInput {
        /// Human-readable reason the fingerprint cannot be trusted.
        reason: FreeText,
    },
}

impl fmt::Display for EvaluationStartCaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthoritativeInput { reason } => {
                write!(formatter, "evaluation-start fingerprint unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for EvaluationStartCaptureError {}

// ---------------------------------------------------------------------------
// EvaluationStartCapturePort
// ---------------------------------------------------------------------------

/// Secondary port for one synchronous, run-wide evaluation-start capture.
pub trait EvaluationStartCapturePort: Send + Sync {
    /// Captures the complete implementation fingerprint before any layer
    /// rustdoc export begins.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationStartCaptureError::AuthoritativeInput`] when the
    /// fingerprint cannot be acquired as one complete authoritative value.
    fn capture_evaluation_start(
        &self,
    ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError>;
}
