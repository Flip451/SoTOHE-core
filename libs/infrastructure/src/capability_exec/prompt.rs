//! Prompt construction shared by provider-native adapters.

use usecase::capability_exec::{CapabilityDispatchRequest, CapabilityResumeRequest};

pub(crate) fn capability_prompt(request: &CapabilityDispatchRequest) -> String {
    let upstream_reread = matches!(
        &request.request.resume,
        CapabilityResumeRequest::Resume(_) | CapabilityResumeRequest::ResumeWithoutTarget
    )
    .then_some(
        "For resumed work, check whether upstream artifacts changed; if they did, reread them before working.",
    )
    .unwrap_or_default();
    format!(
        "${} Briefing: Read {} and perform the task.\n\n{}{}",
        request.request.capability.as_str(),
        request.request.briefing_file.as_path().display(),
        request.discipline.as_str(),
        if upstream_reread.is_empty() { String::new() } else { format!("\n\n{upstream_reread}") },
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::PathBuf;

    use super::capability_prompt;
    use usecase::capability_exec::{
        BriefingText, CapabilityDispatchRequest, CapabilityExecRequest, CapabilityFilePath,
        CapabilityProfile, CapabilityResumeRequest, DisciplineText, ExecutionMode, ModelName,
        ProviderName, ReasoningEffort, TargetArtifactPath, TargetArtifactSet,
    };
    use usecase::dry_write_driver::CapabilityName;

    fn request(resume: CapabilityResumeRequest) -> CapabilityDispatchRequest {
        CapabilityDispatchRequest {
            request: CapabilityExecRequest {
                capability: CapabilityName::try_new("implementer").expect("valid capability"),
                host: Some(ProviderName::try_new("codex").expect("valid host")),
                briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))
                    .expect("valid briefing"),
                timeout: None,
                resume,
            },
            profile: CapabilityProfile {
                provider: usecase::capability_exec::CapabilityProviderBinding::Standard(
                    ProviderName::try_new("codex").expect("valid provider"),
                ),
                model: ModelName::try_new("gpt-5").expect("valid model"),
                effort: ReasoningEffort::High,
                execution_mode: ExecutionMode::OrchestratorOutput,
            },
            briefing: BriefingText::try_new("briefing".to_owned()).expect("valid briefing text"),
            discipline: DisciplineText::try_new("shared discipline".to_owned())
                .expect("valid discipline"),
        }
    }

    #[test]
    fn test_capability_prompt_resume_includes_upstream_reread_instruction() {
        let target = TargetArtifactPath::try_new(PathBuf::from("track/items/a/spec.json"))
            .expect("valid target");
        let prompt = capability_prompt(&request(CapabilityResumeRequest::Resume(
            TargetArtifactSet::try_new(vec![target]).expect("non-empty target set"),
        )));

        assert!(prompt.contains("shared discipline"));
        assert!(prompt.contains(
            "check whether upstream artifacts changed; if they did, reread them before working"
        ));
    }

    #[test]
    fn test_capability_prompt_fresh_omits_resume_only_instruction() {
        let prompt = capability_prompt(&request(CapabilityResumeRequest::Fresh));

        assert!(!prompt.contains("check whether upstream artifacts changed"));
    }
}
