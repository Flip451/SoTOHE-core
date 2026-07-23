//! CLI driver layer (primary adapter).
//!
//! Holds injected use cases, invokes them, and renders their result into a
//! transport-neutral [`CommandOutcome`]. It depends only on [`usecase`]; domain
//! types it needs are reached through `usecase`'s re-exports, so it never takes
//! a direct dependency on the domain crate.

use usecase::{GreetError, GreetUser, SalutationProvider, Username};

/// Rendered result of a CLI command, ready for a presenter to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    /// Human-readable message to display.
    pub message: String,
    /// Whether the command completed successfully.
    pub success: bool,
}

/// Driver that renders the greeting use case into a [`CommandOutcome`].
pub struct GreetDriver {
    interactor: Box<dyn GreetingService>,
}

trait GreetingService {
    fn execute(&self, user: &Username) -> Result<String, GreetError>;
}

impl<P: SalutationProvider> GreetingService for GreetUser<P> {
    fn execute(&self, user: &Username) -> Result<String, GreetError> {
        GreetUser::execute(self, user)
    }
}

impl GreetDriver {
    /// Builds the driver from the injected interactor.
    #[must_use]
    pub fn new<P: SalutationProvider + 'static>(interactor: GreetUser<P>) -> Self {
        Self { interactor: Box::new(interactor) }
    }

    /// Validates `raw_name`, runs the greeting use case, and renders the outcome.
    ///
    /// Validation and use-case errors are captured into the returned
    /// [`CommandOutcome`] rather than propagated, so the presenter has a single
    /// value to render.
    #[must_use]
    pub fn handle(&self, raw_name: &str) -> CommandOutcome {
        match Username::new(raw_name) {
            Ok(user) => self.render_greeting(&user),
            Err(error) => CommandOutcome { message: error.to_string(), success: false },
        }
    }

    fn render_greeting(&self, user: &Username) -> CommandOutcome {
        match self.interactor.execute(user) {
            Ok(message) => CommandOutcome { message, success: true },
            Err(error) => CommandOutcome { message: error.to_string(), success: false },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use usecase::GreetError;

    struct FixedSalutation(&'static str);

    impl SalutationProvider for FixedSalutation {
        fn salutation(&self) -> Result<String, GreetError> {
            Ok(self.0.to_owned())
        }
    }

    struct MissingSalutation;

    impl SalutationProvider for MissingSalutation {
        fn salutation(&self) -> Result<String, GreetError> {
            Err(GreetError::Unavailable)
        }
    }

    #[test]
    fn test_handle_valid_raw_name_returns_success_outcome() {
        let driver = GreetDriver::new(GreetUser::new(FixedSalutation("Hello")));
        let outcome = driver.handle("ada");
        assert!(outcome.success);
        assert_eq!(outcome.message, "Hello, ada!");
    }

    #[test]
    fn test_handle_empty_raw_name_returns_validation_failure() {
        let driver = GreetDriver::new(GreetUser::new(FixedSalutation("Hello")));
        let outcome = driver.handle("   ");

        assert!(!outcome.success);
        assert_eq!(outcome.message, "username must not be empty");
    }

    #[test]
    fn test_handle_unavailable_salutation_returns_failure_outcome() {
        let driver = GreetDriver::new(GreetUser::new(MissingSalutation));
        let outcome = driver.handle("ada");
        assert!(!outcome.success);
        assert_eq!(outcome.message, "salutation is unavailable");
    }
}
