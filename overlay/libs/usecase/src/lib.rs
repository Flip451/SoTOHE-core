//! Usecase layer.
//!
//! Application-specific business rules. This layer orchestrates domain types
//! and depends **only** on the [`domain`] crate. Outward dependencies (I/O,
//! persistence, external services) are expressed as *ports* — traits defined
//! here and implemented by adapters in the infrastructure layer.
//!
//! The worked example pairs one port ([`SalutationProvider`]) with one
//! interactor ([`GreetUser`]) that composes the port's output with a
//! [`domain::Username`].

use thiserror::Error;

// Re-export the domain types that appear in this layer's public API so that
// downstream layers depending only on `usecase` (e.g. the CLI driver) can name
// them without taking a direct dependency on `domain`.
pub use domain::{Username, UsernameError};

/// Error returned when a greeting cannot be produced.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GreetError {
    /// The salutation source was unavailable.
    #[error("salutation is unavailable")]
    Unavailable,
}

/// Port supplying the salutation word used to greet a user (e.g. `"Hello"`).
///
/// The usecase layer owns this contract; an infrastructure adapter provides the
/// concrete implementation, keeping I/O concerns out of the business rules.
pub trait SalutationProvider {
    /// Returns the salutation word to prefix a greeting with.
    ///
    /// # Errors
    ///
    /// Returns [`GreetError::Unavailable`] when no salutation can be supplied.
    fn salutation(&self) -> Result<String, GreetError>;
}

/// Interactor that greets a [`Username`] using an injected [`SalutationProvider`].
pub struct GreetUser<P: SalutationProvider> {
    provider: P,
}

impl<P: SalutationProvider> GreetUser<P> {
    /// Builds the interactor from its injected port dependency.
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Produces a greeting for `user`.
    ///
    /// # Errors
    ///
    /// Propagates [`GreetError`] when the [`SalutationProvider`] cannot supply a
    /// salutation.
    pub fn execute(&self, user: &Username) -> Result<String, GreetError> {
        let word = self.provider.salutation()?;
        Ok(format!("{word}, {}!", user.as_str()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn test_execute_successful_salutation_returns_greeting() {
        let interactor = GreetUser::new(FixedSalutation("Hello"));
        let user = Username::new("ada").unwrap();
        assert_eq!(interactor.execute(&user).unwrap(), "Hello, ada!");
    }

    #[test]
    fn test_execute_missing_salutation_returns_unavailable_error() {
        let interactor = GreetUser::new(MissingSalutation);
        let user = Username::new("ada").unwrap();
        assert_eq!(interactor.execute(&user), Err(GreetError::Unavailable));
    }
}
