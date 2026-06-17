<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# knowledge/strategy ディレクトリの整理方針

## Summary

Delete knowledge/strategy/ (15 files), knowledge/designs/ (3 files), knowledge/schemas/ (2 files), and knowledge/DESIGN.md after salvaging any unique information to knowledge/research/. Update all active doc references to the deleted paths.

## Tasks (0/6 resolved)

### S1 — Salvage and delete target directories

> Read each target file, judge salvage-worthiness, encode novel information to knowledge/research/, then delete the directory or file.

- [~] **T001**: knowledge/strategy/ — salvage then delete. Read all 15 files in knowledge/strategy/. For each file, judge whether it contains information not yet encoded elsewhere in ADR / convention / track artifacts. Encode salvage-worthy information to knowledge/research/YYYY-MM-DD-HHMM-<topic>.md independent files (no inline SoT edits). Then delete the entire knowledge/strategy/ directory. (IN-01 / IN-05 / AC-01 / AC-05 / AC-06)
- [ ] **T002**: knowledge/designs/ and knowledge/schemas/ — salvage then delete. Read all 3 files in knowledge/designs/ (auto-mode-*) and all 2 files in knowledge/schemas/ (auto-mode-config-schema.md, auto-state-schema.md). Apply the same salvage judgment: encode novel information to knowledge/research/ independent files, then delete both directories. (IN-02 / IN-03 / IN-05 / AC-02 / AC-03 / AC-05 / AC-06)
- [ ] **T003**: knowledge/DESIGN.md — salvage then delete. Read knowledge/DESIGN.md (50 lines). The content is mostly pointers to other SSoTs (architecture-rules.json, agent-profiles.json, adr/README.md). Apply salvage judgment; if any novel information exists, encode to knowledge/research/ independent file. Then delete knowledge/DESIGN.md. (IN-04 / IN-05 / AC-04 / AC-05 / AC-06)

### S2 — Update active doc references

> Remove or redirect references to the deleted paths in all surviving active docs.

- [ ] **T004**: Update active doc references to deleted paths. Remove or replace references to knowledge/DESIGN.md and to files under deleted directories (knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/) in active docs not otherwise handled by T005/T006: CLAUDE.md, README.md, .claude/rules/08-orchestration.md, .claude/rules/01-language.md, .claude/commands/track/implement.md, .claude/commands/track/review.md, .claude/commands/track/setup.md, .claude/settings.json (context-compaction echo command), .codex/instructions.md, .claude/skills/codex-system/SKILL.md, knowledge/conventions/task-completion-flow.md, and any other surviving active doc found by rg. For each reference, either delete the line or redirect to the appropriate replacement SSoT / current track artifact (architecture-rules.json / agent-profiles.json / knowledge/adr/README.md / track/items/<id>/plan.md). (IN-06 / AC-07 / AC-08 / AC-09)
- [ ] **T005**: Update knowledge/README.md Directory Structure. Remove the strategy/, designs/, schemas/ rows from the Directory Structure table in knowledge/README.md and remove the knowledge/DESIGN.md entry from the Related Top-Level Files section. Also remove the 'strategy/TODO-PLAN.md — Strategic roadmap' reference from the numbered list if present. (IN-07 / AC-10)
- [ ] **T006**: Remove dead reference from knowledge/conventions/adr.md Decision Reference. Delete the line referencing knowledge-restructure-design-2026-03-20.md (../strategy/knowledge-restructure-design-2026-03-20.md) from the Decision Reference section of knowledge/conventions/adr.md. (IN-08 / AC-11)
