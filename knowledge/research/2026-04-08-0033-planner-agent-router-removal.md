# Planner Design Review: agent-router.py Removal

> Date: 2026-04-08
> Capability: planner (Claude Opus)
> Feature: agent-router hook removal + skill compliance hook

## Key Findings

### 1. External Guide Injection — Functional Gap

agent-router.py is NOT purely advisory. It also calls `external_guides.find_relevant_guides_for_track_workflow(prompt)` when `/track:plan`, `/track:implement`, etc. are detected, injecting `[External Guide Context]` summaries into the prompt. This is a real functional behavior that would be lost.

**Mitigation**: The replacement skill-compliance hook should incorporate external guide injection for `/track:*` commands.

### 2. Complete Edit Scope

| Action | File | Detail |
|--------|------|--------|
| DELETE | `.claude/hooks/agent-router.py` | Hook body |
| DELETE | `.claude/hooks/test_agent_router.py` | Must be deleted atomically with hook — pytest collection fails otherwise |
| EDIT | `.claude/settings.json` | Remove entire `UserPromptSubmit` key (not just empty the array) |
| EDIT | `libs/infrastructure/src/verify/orchestra.rs` | Remove from `EXPECTED_HOOK_PATHS` line 46 |
| EDIT | `knowledge/DESIGN.md` | Remove from Python advisory hooks table line 440 |

### 3. No Hidden Dependencies

- No other hook imports from `agent-router.py`
- `EXPECTED_HOOK_PATHS` tests are loop-based (no hardcoded count)
- No references in `.claude/rules/`, `Makefile.toml`, `knowledge/conventions/`, `DEVELOPER_AI_WORKFLOW.md`, `knowledge/WORKFLOW.md`
- `_agent_profiles.py` is independent and used by other hooks — keep it

### 4. settings.json Schema

`UserPromptSubmit` key can be absent entirely. The `hook_commands()` parser in `orchestra.rs` iterates over whatever keys exist — no minimum set required.

### 5. Replacement Hook Concept (WF-67)

Instead of intent detection + provider suggestion (what agent-router does), the replacement should:
- Detect `/track:*` slash command invocations
- Inject reminders about SKILL.md phase requirements (e.g., "Phase 1.5 planner review is mandatory for Full mode")
- Inject external guide summaries (preserving the current functional behavior)
- NOT duplicate routing logic (leave that to rules + agent-profiles.json)

### 6. Risk Assessment

- Routing accuracy: Low risk. `08-orchestration.md` + `agent-profiles.json` are sufficient
- External guides: Medium risk if guides are registered. Mitigated by replacement hook
- Skill compliance: The current agent-router does NOT enforce skill compliance. The proposed replacement would add new enforcement that doesn't exist today
