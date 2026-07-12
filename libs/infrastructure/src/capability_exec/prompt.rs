//! Prompt construction shared by provider-native adapters.

use usecase::capability_exec::CapabilityDispatchRequest;

pub(crate) fn capability_prompt(request: &CapabilityDispatchRequest) -> String {
    format!(
        "${} Briefing: Read {} and perform the task.\n\n{}",
        request.request.capability.as_str(),
        request.request.briefing_file.as_path().display(),
        request.discipline.as_str(),
    )
}
