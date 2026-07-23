//! CLI composition root.
//!
//! The only place allowed to know every layer at once. It constructs the
//! concrete infrastructure adapter, injects it into the usecase interactor,
//! and wires that into the CLI driver.

use cli_driver::GreetDriver;
use infrastructure::StaticSalutation;
use usecase::GreetUser;

/// Composition root for the greeting command.
pub struct GreetingCompositionRoot;

impl GreetingCompositionRoot {
    /// Creates a greeting composition root.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Builds the fully wired greeting primary adapter.
    #[must_use]
    pub fn greeting_driver(&self) -> GreetDriver {
        let adapter = StaticSalutation::new("Hello");
        let interactor = GreetUser::new(adapter);

        GreetDriver::new(interactor)
    }
}

impl Default for GreetingCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_greeting_driver_wires_all_layers() {
        let root = GreetingCompositionRoot::new();
        let outcome = root.greeting_driver().handle("ada");
        assert!(outcome.success);
        assert_eq!(outcome.message, "Hello, ada!");
    }

    #[test]
    fn test_greeting_driver_rejects_empty_name() {
        let root = GreetingCompositionRoot::new();
        let outcome = root.greeting_driver().handle("   ");

        assert!(!outcome.success);
        assert_eq!(outcome.message, "username must not be empty");
    }
}
