//! CLI driver layer (primary adapter).
//!
//! Holds injected use cases, invokes them, and renders their result into a
//! transport-neutral [`CommandOutcome`]. It depends only on [`usecase`]; domain
//! types it needs are reached through `usecase`'s re-exports, so it never takes
//! a direct dependency on the domain crate.

use usecase::{GreetUser, SalutationProvider, Username};

/// Rendered result of a CLI command, ready for a presenter to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    /// Human-readable message to display.
    pub message: String,
    /// Whether the command completed successfully.
    pub success: bool,
}

/// Driver that renders the greeting use case into a [`CommandOutcome`].
pub struct GreetDriver<P: SalutationProvider> {
    interactor: GreetUser<P>,
}

impl<P: SalutationProvider> GreetDriver<P> {
    /// Builds the driver from the injected interactor.
    pub fn new(interactor: GreetUser<P>) -> Self {
        Self { interactor }
    }

    /// Runs the greeting use case for `user` and renders the outcome.
    ///
    /// Errors from the use case are captured into the returned
    /// [`CommandOutcome`] rather than propagated, so the presenter has a single
    /// value to render.
    pub fn run(&self, user: &Username) -> CommandOutcome {
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
    fn test_run_successful_usecase_returns_success_outcome() {
        let driver = GreetDriver::new(GreetUser::new(FixedSalutation("Hello")));
        let user = Username::new("ada").unwrap();
        let outcome = driver.run(&user);
        assert!(outcome.success);
        assert_eq!(outcome.message, "Hello, ada!");
    }

    #[test]
    fn test_run_usecase_error_returns_failure_outcome() {
        let driver = GreetDriver::new(GreetUser::new(MissingSalutation));
        let user = Username::new("ada").unwrap();
        let outcome = driver.run(&user);
        assert!(!outcome.success);
        assert_eq!(outcome.message, "salutation is unavailable");
    }
}
