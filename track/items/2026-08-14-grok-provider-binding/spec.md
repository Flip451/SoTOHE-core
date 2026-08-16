<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 28, yellow: 0, red: 0 }
---

# Grok provider binding

## Goal

- [GO-01] Enable Grok CLI as a subscription-authenticated provider for the repository's capability and typed-pipeline execution contracts, while preserving deterministic, isolated, fail-closed dispatch behavior and leaving the shipped default provider selection unchanged. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D1, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D2, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D3, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D5, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D7, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8]

## Scope

### In Scope
- [IN-01] Grok provider executions accept a briefing file and a structured-output schema, and derive the returned value exclusively from the envelope's structured-output field. A missing value fails closed and reports the envelope's failure reason; the envelope text field is not a return channel. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D1] [tasks: T001, T003, T004, T005, T010]
- [IN-02] Every Grok capability execution runs as an independent subprocess with shared-process connection disabled, an explicit model resolved from the capability profile, and reasoning effort resolved from that profile. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D2, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [IN-03] Grok is eligible as both an orchestrator host and a provider. When the host and provider are both Grok, dispatch still starts an independent Grok subprocess rather than delegating in-host. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D4] [tasks: T003, T006]
- [IN-04] The profile-defined capability universe may route Grok through both execution modes: orchestrator-output uses the capability-exec Grok path, while typed-pipeline retains its dedicated execution paths with the same Grok launch contract. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D5, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [IN-05] Compatible capability definitions are discovered from the shared .agents/ surface. Grok-specific sandbox permission is declared as grok-sandbox on that adapter definition, using Grok's accepted sandbox vocabulary; Grok-only project files are limited to surfaces the shared definition cannot provide. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D3, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D6] [tasks: T009, T008, T002, T003]
- [IN-06] The repository ships a Grok sample profile and adapter permission declaration examples without changing the shipped default profile to select Grok. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D7] [tasks: T008]
- [IN-07] When Grok is the host, its project hook surface is .grok/hooks/. Grok envelope and tool names are translated into the existing hook-handler contract, and untranslatable hook input fails closed. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D9] [tasks: T007, T013]

### Out of Scope
- [OS-01] Routing Grok through the Codex custom-provider path or requiring an API key is out of scope; Grok is added as its own provider adapter for its CLI sign-in authentication path. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D1] [tasks: T003, T006, T008]
- [OS-02] Using the envelope text field as a fallback or alternative result-extraction channel is out of scope. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D1] [tasks: T001, T003]
- [OS-03] Shared-process connections and in-host Grok delegation are out of scope, including when Grok is the active orchestrator host. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D2, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D4] [tasks: T003, T006]
- [OS-04] Hard-coding or enumerating the capability universe, and merging typed-pipeline execution into capability exec, are out of scope. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D5] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [OS-05] Duplicating shared capability bodies in .grok/skills/, accepting a Grok sandbox value through the Codex sandbox key, or accepting unrestricted sandbox mode is out of scope. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D3, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D6] [tasks: T009, T008, T002, T003]
- [OS-06] Changing the shipped default profile to use Grok, or replacing the Claude host configuration surface, is out of scope. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D7, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D9] [tasks: T007, T008, T013]

## Constraints
- [CN-01] Dispatch fails closed when the profile capability is absent or its required adapter definition is absent. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D5, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D6] [tasks: T002, T003]
- [CN-04] Dispatch fails closed when grok-sandbox is undeclared for an execution request or its value is unsupported or unrestricted. An undeclared grok-sandbox resolves to read-only only for diagnosis and validation; it never authorizes dispatch. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D3] [tasks: T009, T002, T003]
- [CN-05] Dispatch fails closed when model or effort cannot be resolved, or when a declared adapter model does not exactly match the profile projection. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [CN-02] Grok follows the existing model, effort, and session-resume dispatch contract without Grok-specific capability exceptions. A resumed execution remains an independent subprocess and explicitly receives model, effort, and permission settings; failed, expired, provider-mismatched, or model-mismatched resume falls back to a new session without rejecting the dispatch for that reason alone. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [CN-03] Grok host hooks reuse the existing policy handlers; .grok/hooks/ is the canonical Grok-specific declaration surface and .claude/settings.json remains the Claude-specific surface. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D9] [tasks: T007, T013]

## Acceptance Criteria
- [ ] [AC-01] A successful Grok provider execution returns only the envelope structured-output value. If that value is absent, the execution fails closed and exposes the envelope failure reason rather than treating envelope text as a result. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D1] [tasks: T001, T003, T004, T005, T010]
- [ ] [AC-02] Each Grok capability dispatch starts an isolated subprocess with no shared-process connection and passes the profile-resolved model and reasoning effort explicitly. The same behavior occurs when Grok hosts a Grok-provided capability. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D2, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D4] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [ ] [AC-03] A profile-defined Grok capability is dispatchable only when the capability name exists in the profile, its shared adapter definition exists, and that definition declares a valid grok-sandbox. It then uses its applicable execution mode: orchestrator-output uses capability exec, and typed-pipeline uses its dedicated path. Adding a new orchestrator-output capability does not require a new per-capability Grok dispatch arm. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D5, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D3, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D6] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]
- [ ] [AC-04] Dispatch rejects a Grok capability whose shared adapter definition is missing or lacks a valid grok-sandbox declaration, and rejects an unsupported or unrestricted Grok sandbox. Diagnostic resolution of an undeclared grok-sandbox is read-only and does not make execution admissible. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D3, knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D6] [tasks: T009, T002, T003]
- [ ] [AC-05] A resumed Grok dispatch explicitly re-supplies model, effort, and permission settings, remains an independent process, and falls back to a new session when resume is unavailable, expired, or incompatible without adding a Grok-only dispatch exception. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8] [tasks: T003, T004, T005, T010, T006, T011, T012]
- [ ] [AC-06] The shipped profile continues to select no Grok provider by default, while a Grok sample profile and shared-adapter permission declaration example are present for consumers who opt in. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D7] [tasks: T008]
- [ ] [AC-07] When Grok is the host, .grok/hooks/ declares the host guards and Grok hook envelopes and tool names are mapped to the existing handler contract. An input that cannot be mapped is rejected, and the Claude configuration surface remains unchanged. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D9] [tasks: T007, T013]
- [ ] [AC-08] Grok is executed as the Grok CLI provider adapter, not as a Codex custom-provider implementation. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D1] [tasks: T003, T006]
- [ ] [AC-09] Dispatch rejects a Grok capability when its model or reasoning effort cannot be resolved, or when a declared adapter model does not exactly match the profile projection. [adr: knowledge/adr/2026-08-14-1225-grok-provider-binding.md#D8] [tasks: T002, T003, T004, T005, T010, T006, T011, T012]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 28  🟡 0  🔴 0

