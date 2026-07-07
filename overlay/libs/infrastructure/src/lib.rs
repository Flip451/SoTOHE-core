//! Infrastructure layer.
//!
//! Outward adapters that implement the ports declared by the [`usecase`] layer.
//! This is where I/O, persistence, and external-service concerns live. It may
//! depend on [`domain`] and [`usecase`] but nothing above it — adapters are
//! wired into the application only at the composition root.
//!
//! The worked example, [`StaticSalutation`], is a trivial adapter implementing
//! [`usecase::SalutationProvider`] with a fixed word.

use usecase::{GreetError, SalutationProvider};

/// Adapter supplying a fixed salutation word.
///
/// A real adapter would read from configuration, a database, or a remote
/// service; this placeholder keeps the wiring demonstrable without any I/O.
pub struct StaticSalutation {
    word: String,
}

impl StaticSalutation {
    /// Builds the adapter from the salutation word to serve.
    pub fn new(word: impl Into<String>) -> Self {
        Self { word: word.into() }
    }
}

impl SalutationProvider for StaticSalutation {
    fn salutation(&self) -> Result<String, GreetError> {
        Ok(self.word.clone())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn static_salutation_returns_configured_word() {
        let adapter = StaticSalutation::new("Hi");
        assert_eq!(adapter.salutation().unwrap(), "Hi");
    }
}
