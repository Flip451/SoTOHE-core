//! Shared fail-closed response for command families backed by semantic-dup.

use std::process::ExitCode;

/// Feature-gated commands whose implementation is compiled behind `semantic-dup`.
pub enum SemanticDupCommandFamily {
    /// The `sotp dry write` command.
    DryWrite,
    /// The `sotp dry results` command.
    DryResults,
    /// The `sotp dry fix-local` command.
    DryFixLocal,
    /// The semantic duplicate inspection command family.
    SemanticDuplicate,
}

impl SemanticDupCommandFamily {
    fn name(&self) -> &'static str {
        match self {
            Self::DryWrite => "dry write",
            Self::DryResults => "dry results",
            Self::DryFixLocal => "dry fix-local",
            Self::SemanticDuplicate => "semantic duplicate",
        }
    }
}

/// Print a clear feature-gate error and return a non-success exit code.
pub fn semantic_dup_feature_disabled_exit(command_family: SemanticDupCommandFamily) -> ExitCode {
    eprintln!(
        "{} commands require the disabled `semantic-dup` feature; rebuild sotp with `--features semantic-dup`",
        command_family.name()
    );
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use std::process::ExitCode;

    use super::{SemanticDupCommandFamily, semantic_dup_feature_disabled_exit};

    #[test]
    fn test_feature_disabled_command_selectors_have_presentation_labels() {
        assert_eq!(SemanticDupCommandFamily::DryWrite.name(), "dry write");
        assert_eq!(SemanticDupCommandFamily::DryResults.name(), "dry results");
        assert_eq!(SemanticDupCommandFamily::DryFixLocal.name(), "dry fix-local");
        assert_eq!(SemanticDupCommandFamily::SemanticDuplicate.name(), "semantic duplicate");
    }

    #[test]
    fn test_feature_disabled_command_selectors_return_failure() {
        for selector in [
            SemanticDupCommandFamily::DryWrite,
            SemanticDupCommandFamily::DryResults,
            SemanticDupCommandFamily::DryFixLocal,
            SemanticDupCommandFamily::SemanticDuplicate,
        ] {
            assert_eq!(semantic_dup_feature_disabled_exit(selector), ExitCode::FAILURE);
        }
    }
}
