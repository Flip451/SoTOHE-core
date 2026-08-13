<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Multi-provider capability routing: extend capability profiles with Codex custom model providers

## Summary

GO-01 -> T001, T002, T003.
GO-02 -> T004, T005, T006.

## Tasks (4/6 resolved)

### typed-routing-binding — Typed routing binding

> Modify `usecase::capability_exec` and `infrastructure::capability_exec`. [IN-01; IN-02; CN-01; AC-01; AC-02]

- [x] **T001**: Modify `usecase::capability_exec::{ModelProviderName, CapabilityProviderBinding, CapabilityProfile, CapabilityInputValidationError}` and unit coverage. [IN-01; IN-02; OUT-01; CN-01; AC-01; AC-02]
- [x] **T002**: Modify `infrastructure::agent_profiles::{ModelProviderNameDto, CapabilityProviderBindingDto, CapabilityConfigDto}` and `infrastructure::capability_exec::agent_profiles::AgentProfilesCapabilityAdapter`; add coverage. [IN-01; IN-02; OUT-01; CN-01; AC-01; AC-02]
- [x] **T003**: Modify `infrastructure::capability_exec::codex::CodexCapabilityAdapter` and dispatch coverage. [IN-01; IN-02; OUT-02; CN-01; AC-01; AC-02]

### consumer-provider-guidance — Consumer provider guidance

> Modify `README.md` documentation. [IN-03; IN-04; CN-02; CN-03; AC-03; AC-04; AC-05]

- [x] **T004**: Modify `README.md` documentation. [IN-03; OUT-03; CN-02; AC-03]
- [ ] **T005**: Modify `README.md` documentation. [CN-02; AC-04]
- [ ] **T006**: Modify `README.md` documentation. [IN-04; OUT-04; CN-03; AC-05]
