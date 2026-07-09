//! Infrastructure codecs and adapters for the test-obligation gate.
//!
//! Holds the JSON codecs that load and validate the decision-table config
//! (`.harness/config/test-obligation-rules.json`) into the domain rules model
//! (IN-02 / IN-04 / AC-02) and that persist the track-scoped artifacts:
//!
//! * [`obligations_codec`] — derived obligations artifact (IN-05 / AC-03).
//! * [`bindings_codec`] — implementer-authored test-bindings artifact (IN-06 /
//!   AC-04).
//! * [`fulfillment_cache_codec`] / [`waiver_cache_codec`] — hash-frozen verdict
//!   caches (IN-09 / CN-04 / AC-06).
//!
//! It also holds the worktree / LLM secondary adapters the gate drives:
//!
//! * [`source_scanner`] — `syn`-based bound-test body scanner (IN-06 / IN-09).
//! * [`fulfillment_verifier`] / [`waiver_verifier`] — capability adapters that
//!   delegate the semantic judgement to the configured provider (IN-09 / IN-11 /
//!   IN-12 / IN-15 / CN-08). Their shared runtime lives in `semantic_verifier`.

pub mod bindings_codec;
pub mod fulfillment_cache_codec;
pub mod fulfillment_escalation_driver;
pub mod fulfillment_verifier;
pub mod obligations_codec;
pub mod rules_codec;
mod semantic_verifier;
pub mod sha256_content_hasher;
pub mod source_scanner;
pub mod waiver_cache_codec;
pub mod waiver_escalation_driver;
pub mod waiver_verifier;

use std::path::Path;

use domain::tddd::test_obligation::ids::DiagnosticMessage;

/// Builds a [`DiagnosticMessage`] for a codec failure, substituting a constant
/// for empty input so a diagnostic can never be silent.
pub(crate) fn diagnostic(message: &str) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "unspecified test-obligation codec error".to_owned()
    } else {
        message.to_owned()
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            // Unreachable: `text` is non-empty on entry; reset defensively.
            Err(_) => text = "unspecified test-obligation codec error".to_owned(),
        }
    }
}

/// Rejects a symlinked track-items root before it is used as a trusted guard
/// anchor. Missing roots are allowed so missing artifacts can still load as
/// `None` and saves can create the directory.
pub(crate) fn reject_symlinked_items_root(items_dir: &Path) -> Result<(), std::io::Error> {
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to use symlinked track items root: {}", items_dir.display()),
        )),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(std::io::Error::new(
            source.kind(),
            format!("failed to stat track items root {}: {source}", items_dir.display()),
        )),
    }
}
