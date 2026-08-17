//! Secondary adapters for track-lifecycle compatibility boundaries.

pub mod resolution_compat;

/// TDDD-specific secondary adapters for track lifecycle use cases.
pub mod tddd {
    pub mod baseline_capture;
    pub mod baseline_graph;
    pub mod catalogue_impl_signals;
}
