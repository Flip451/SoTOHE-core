//! Infrastructure codecs and adapters for the test-obligation gate.
//!
//! Holds the JSON codec that loads and validates the decision-table config
//! (`.harness/config/test-obligation-rules.json`) into the domain rules model
//! (IN-02 / IN-04 / AC-02).

pub mod rules_codec;
