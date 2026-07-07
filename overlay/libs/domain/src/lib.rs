//! Domain layer.
//!
//! The innermost layer of the hexagonal architecture. It holds pure business
//! types and rules and has **no** dependency on any other workspace crate.
//!
//! This template ships a single worked example — the [`Username`] newtype — to
//! demonstrate the "parse, don't validate" pattern: an invariant (non-empty)
//! is enforced once, in a constructor, and encoded in the type thereafter.

use thiserror::Error;

/// Error returned when a [`Username`] fails validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UsernameError {
    /// The provided value was empty or contained only whitespace.
    #[error("username must not be empty")]
    Empty,
}

/// A validated, non-empty username.
///
/// Construct one through [`Username::new`]; the wrapped string is guaranteed to
/// be non-empty and free of surrounding whitespace for the value's lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username(String);

impl Username {
    /// Creates a `Username`, trimming surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns [`UsernameError::Empty`] when the trimmed input is empty.
    pub fn new(raw: impl Into<String>) -> Result<Self, UsernameError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(UsernameError::Empty);
        }
        Ok(Self(trimmed))
    }

    /// Returns the username as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn new_trims_and_accepts_non_empty() {
        let name = Username::new("  ada  ").unwrap();
        assert_eq!(name.as_str(), "ada");
    }

    #[test]
    fn new_rejects_empty_input() {
        assert_eq!(Username::new("   "), Err(UsernameError::Empty));
    }
}
