//! Typed wrapper around rendered Contract Map markdown.
//!
//! `ContractMapContent` is the value object that `render_contract_map`
//! produces and that `ContractMapWriter` (added in T004 at
//! `crate::tddd::catalogue_ports::ContractMapWriter`) persists. Keeping it as a distinct newtype prevents
//! arbitrary strings from being written to `contract-map.md` and documents
//! the expected provenance at every port / adapter boundary (ADR
//! 2026-04-17-1528 §D1).
//!
//! The wrapper is intentionally validation-free: the renderer is the sole
//! producer, it is infallible, and its output always contains at least the
//! fenced mermaid scaffold (`flowchart LR` + `end`). Adding a non-empty
//! validation would force every call site to handle an `Err` that cannot
//! occur, and would reintroduce `expect_used` / `unwrap_used` lint
//! violations in library code.

/// Rendered Contract Map markdown (mermaid flowchart embedded in a fenced
/// code block). Produced by `render_contract_map`, consumed by
/// `ContractMapWriter` adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractMapContent(String);

impl ContractMapContent {
    /// Creates a new `ContractMapContent` from any string-like value.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self(content.into())
    }

    /// Consumes the wrapper and returns the owned markdown string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for ContractMapContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_map_content_roundtrips_input_string() {
        let content = ContractMapContent::new("```mermaid\nflowchart LR\nend\n```\n");
        assert!(content.as_ref().contains("flowchart LR"));
    }

    #[test]
    fn test_contract_map_content_into_string_returns_owned_value() {
        let content = ContractMapContent::new("payload");
        assert_eq!(content.into_string(), "payload");
    }

    #[test]
    fn test_contract_map_content_equality_compares_inner_string() {
        assert_eq!(ContractMapContent::new("a"), ContractMapContent::new("a"));
        assert_ne!(ContractMapContent::new("a"), ContractMapContent::new("b"));
    }
}
