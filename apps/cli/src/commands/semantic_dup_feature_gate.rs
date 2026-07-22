//! Shared fail-closed response for command families backed by semantic-dup.

use std::process::ExitCode;

/// Command families whose implementation is compiled behind `semantic-dup`.
pub enum SemanticDupCommandFamily {
    /// The DRY command family.
    Dry,
    /// The semantic duplicate inspection command family.
    SemanticDuplicate,
}

impl SemanticDupCommandFamily {
    fn name(&self) -> &'static str {
        match self {
            Self::Dry => "dry",
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
    fn test_feature_disabled_command_families_return_failure() {
        assert_eq!(
            semantic_dup_feature_disabled_exit(SemanticDupCommandFamily::Dry),
            ExitCode::FAILURE
        );
        assert_eq!(
            semantic_dup_feature_disabled_exit(SemanticDupCommandFamily::SemanticDuplicate),
            ExitCode::FAILURE
        );
    }
}
