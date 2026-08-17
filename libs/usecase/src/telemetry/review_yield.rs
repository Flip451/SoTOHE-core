//! Typed values for structured-review yield reporting.

use std::fmt;
use std::num::NonZeroU64;

use domain::review_v2::{RoundType, ScopeName};

use crate::capability_exec::{ModelName, ProviderName, ReasoningEffort};

/// A non-negative finding count from one completed structured review round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewFindingCount {
    value: u32,
}

impl ReviewFindingCount {
    /// Wraps a finding count.
    #[must_use]
    pub fn new(value: u32) -> Self {
        Self { value }
    }

    /// Returns the finding count.
    #[must_use]
    pub fn value(&self) -> u32 {
        self.value
    }
}

/// A positive number of completed structured review rounds in an aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewExecutionCount {
    value: NonZeroU64,
}

impl ReviewExecutionCount {
    /// Wraps a positive execution count.
    #[must_use]
    pub fn new(value: NonZeroU64) -> Self {
        Self { value }
    }
}

impl fmt::Display for ReviewExecutionCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

/// A structured-review detection rate represented in basis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewDetectionRateBasisPoints {
    value: u16,
}

impl ReviewDetectionRateBasisPoints {
    /// Creates a rate in the inclusive `0..=10_000` basis-point range.
    ///
    /// # Errors
    ///
    /// Returns [`ReviewYieldValueError::DetectionRateOutOfRange`] when the
    /// supplied rate exceeds 10,000 basis points.
    pub fn try_new(value: u16) -> Result<Self, ReviewYieldValueError> {
        if value > 10_000 {
            return Err(ReviewYieldValueError::DetectionRateOutOfRange);
        }
        Ok(Self { value })
    }
}

impl fmt::Display for ReviewDetectionRateBasisPoints {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

/// A typed dimension by which structured-review yield can be grouped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewYieldValue {
    /// Review scope dimension.
    Scope(ScopeName),
    /// Review round-type dimension.
    RoundType(RoundType),
    /// Reviewer provider dimension.
    Provider(ProviderName),
    /// Reviewer model dimension.
    Model(ModelName),
    /// Reviewer reasoning-effort dimension.
    ReasoningEffort(ReasoningEffort),
}

/// Read-model metric for one structured-review yield dimension value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewYieldMetric {
    /// Grouping dimension value.
    pub value: ReviewYieldValue,
    /// Number of matching completed review rounds.
    pub execution_count: ReviewExecutionCount,
    /// Proportion of matching rounds that produced one or more findings.
    pub detection_rate: ReviewDetectionRateBasisPoints,
}

/// Validation failures for structured-review yield values.
#[derive(Debug, thiserror::Error)]
pub enum ReviewYieldValueError {
    /// A detection rate cannot be represented in the basis-point range.
    #[error("review detection rate must be between 0 and 10_000 basis points")]
    DetectionRateOutOfRange,
}
