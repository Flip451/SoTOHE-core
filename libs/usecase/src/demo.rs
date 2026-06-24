//! Demo use case port.

use std::sync::Arc;

/// Error returned by [`DemoPort`] methods.
#[derive(Debug, thiserror::Error)]
pub enum DemoPortError {
    /// The infrastructure layer could not fulfill the request.
    #[error("{0}")]
    Unavailable(String),
}

/// Secondary port for the built-in demo.
pub trait DemoPort: Send + Sync {
    /// Run the demo and return a display string or error.
    fn run(&self) -> Result<String, DemoPortError>;
}

/// Application service trait for the demo use case.
pub trait DemoService: Send + Sync {
    /// Run the demo.
    fn run(&self) -> Result<String, DemoPortError>;
}

/// Interactor that delegates to the injected [`DemoPort`].
pub struct DemoInteractor {
    port: Arc<dyn DemoPort>,
}

impl DemoInteractor {
    /// Create a new interactor bound to the given port.
    #[must_use]
    pub fn new(port: Arc<dyn DemoPort>) -> Self {
        Self { port }
    }
}

impl DemoService for DemoInteractor {
    fn run(&self) -> Result<String, DemoPortError> {
        self.port.run()
    }
}
