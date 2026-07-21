//! Verdict-projection port for the semantic-verdict core.
//!
//! [`SemanticEscalationVerdictBridge`] projects a verifier-owned verdict `V` into
//! the shared [`SemanticVerdict`] vocabulary so the core can enforce the
//! citation-required discipline uniformly across verifiers (IN-01 / AC-15).

use domain::tddd::semantic_verify::SemanticVerdict;

/// Projects a verifier-owned verdict into the neutral [`SemanticVerdict`].
///
/// Generic over `V`, the verifier's own verdict type, so the core stays
/// responsibility-neutral: each verifier decides how its verdict maps onto the
/// shared pass / fail / pending vocabulary.
pub trait SemanticEscalationVerdictBridge<V> {
    /// Projects `verdict` into the neutral [`SemanticVerdict`] vocabulary.
    fn project(&self, verdict: &V) -> SemanticVerdict;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::tddd::semantic_verify::EvidenceCitation;

    use super::*;

    /// Bridge double: projects a verifier-owned `bool` verdict onto the neutral
    /// vocabulary (true → pass with a citation, false → fail).
    struct StubBridge;

    impl SemanticEscalationVerdictBridge<bool> for StubBridge {
        fn project(&self, verdict: &bool) -> SemanticVerdict {
            if *verdict {
                let citation = EvidenceCitation::try_new("supported by the anchor".to_owned())
                    .expect("non-empty citation");
                SemanticVerdict::Pass { citation }
            } else {
                SemanticVerdict::Fail { reason: "claim not backed".to_owned() }
            }
        }
    }

    #[test]
    fn test_bridge_projects_pass_with_citation() {
        let verdict = StubBridge.project(&true);
        match verdict {
            SemanticVerdict::Pass { citation } => {
                assert_eq!(citation.as_str(), "supported by the anchor");
            }
            other => panic!("expected Pass, got {other:?}"),
        }
    }

    #[test]
    fn test_bridge_projects_fail() {
        assert_eq!(
            StubBridge.project(&false),
            SemanticVerdict::Fail { reason: "claim not backed".to_owned() }
        );
    }
}
