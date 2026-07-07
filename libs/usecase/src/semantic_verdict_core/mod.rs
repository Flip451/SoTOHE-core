//! Responsibility-neutral semantic-verdict core (IN-01 / AC-15 / OS-02).
//!
//! Extracts the verifier-agnostic parts of semantic verification so ref-verify
//! (chain 1 / chain 2) and the obligation-fulfillment gate share one core
//! instead of a third copy-pasted verifier: the escalation driver
//! ([`driver::SemanticEscalationDriverPort`]), the verdict projection
//! ([`verdict::SemanticEscalationVerdictBridge`]), and the calibration-probe
//! configuration ([`probe::SemanticCalibrationProbeConfig`]).
//!
//! Verifier-specific concerns — pair / obligation types, target-set generation,
//! scope resolution, and fail routing — deliberately stay in each verifier.

pub mod driver;
pub mod probe;
pub mod verdict;
