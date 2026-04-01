# Tsumiki Requirements Hearing — Implementation-Level Deep Dive

Date: 2026-04-01
Sources:
- https://github.com/classmethod/tsumiki (v1.3.0, MIT License)
- https://zenn.dev/hidechannu/articles/20260314-spec-driven-development-tsumiki
- SoTOHE-core SKILL.md (diff-hearing-2026-03-27 track, TSUMIKI-03)

Purpose: Compare tsumiki's `kairo-requirements` hearing implementation with
SoTOHE-core's differential hearing at the implementation level. Identify
concrete adoption opportunities.

---

## 1. Tsumiki Repository Structure

```
classmethod/tsumiki/
  commands/               # 31 slash commands (.md prompt specs)
    kairo-requirements.md # Phase 1: requirements hearing
    kairo-design.md       # Phase 2: architecture design
    kairo-tasks.md        # Phase 3: task decomposition
    kairo-loop.md         # Phase 4: TDD implementation loop
    tdd-red.md
    tdd-green.md
    tdd-refactor.md
    tdd-testcases.md
    tdd-verify-complete.md
    ...
  skills/                 # 12 reusable skill modules
    kairo-implement/
      SKILL.md
      kairo-tdd-process.md
      kairo-direct-process.md
    dev-context/
    dev-debug/
    dev-impl/
    ...
  .claude-plugin/plugin.json
  MANUAL.md
```

Key observation: **Tsumiki has no runtime code**. Everything is prompt
specification files (`.md`). There is no library, no JSON schema validation,
no AST scanning, no domain types. The entire framework operates through
Claude Code's prompt interpretation.

---

## 2. kairo-requirements: Hearing Flow (5 stages)

### Stage 1: Argument Validation

```
Input:  requirement_name (required), prd_file_path (optional)
Output: validated context for subsequent stages
```

The command accepts a requirement name and an optional PRD file. If the PRD
file is provided, it is read and used as the primary context source.

### Stage 2: Scope Selection (Workload Mode)

Tsumiki asks the user to choose one of three modes via `AskUserQuestion`:

| Mode | Artifacts Produced | Use Case |
|------|-------------------|----------|
| Full | requirements.md, user-stories.md, acceptance-criteria.md, interview-record.md, note.md | New feature, complex requirement |
| Lightweight | requirements.md, interview-record.md, note.md | Small change, bug fix |
| Custom | User-selected subset | Partial re-hearing |

This is a **significant UX pattern** that SoTOHE-core lacks entirely.
The `/track:plan` skill always runs the full flow regardless of feature size.

### Stage 3: Context Construction

Reads existing project context in this order:
1. `docs/rule/` — project rules and conventions
2. `docs/design/` — existing design documents
3. Existing notes from prior runs
4. PRD file (if provided)

This is analogous to SoTOHE-core's Phase 1 Step 2-3 (researcher + spec.json
reading), but tsumiki does it inline within the same command rather than as
a separate agent team phase.

### Stage 4: Differential Interviewing

This is the core hearing mechanism. Implementation is **purely prompt-driven**:

The prompt instructs Claude to classify questions into 5 categories:

```
1. Existing Design Validity     — Review current constraints and tech choices
2. Undefined Detail Elicitation — Clarify behavior, data formats, UI expectations
3. Additional/Changed Needs     — New features, integrations, reporting
4. Existing Feature Impact      — Performance, security implications
5. Priority Adjustment          — Must Have / Should Have / Could Have
```

Question format specification:
- Each question uses `AskUserQuestion` tool
- `multiSelect: true` for multi-choice responses
- Structure: `header` (<=12 chars) + `question` + `options` (2-4 items)
- Always include "Other (free text)" option
- Questions are designed as yes/no or selection-based (minimize typing)

**Critical insight**: The "differential" aspect means "read existing docs first,
then ask only about gaps." There is **no structural diff algorithm**, no signal
evaluation, no formal gap analysis. The LLM reads context and generates
questions based on what it judges to be missing.

### Stage 5: Artifact Generation

Produces structured Markdown files with traffic light signals:

```markdown
## Functional Requirements

### FR-001: User Login
🔵 Users can log in with email and password [source: PRD §3.2]

### FR-002: Password Reset Expiry
🟡 Password reset links expire after 24 hours [source: inference — security best practice]

### FR-003: Social Login Provider
🔴 Only Google OAuth is supported [source: assumption — not confirmed]
```

Each requirement item gets:
- Traffic light signal (🔵/🟡/🔴)
- Source attribution tag
- EARS notation (When/If/While/Shall structure)

### Stage 6: Quality Assessment

Reports signal distribution:
```
✅ High Quality: predominantly 🔵, few 🟡, no 🔴
⚠️ Needs Improvement: many 🟡 or any 🔴 remaining
```

---

## 3. interview-record.md Format

A unique tsumiki artifact with no SoTOHE-core equivalent:

```markdown
# Interview Record: {requirement_name}

## Hearing Context
- Date: {timestamp}
- Mode: Full / Lightweight / Custom
- Input Documents: {list of PRDs/docs read}

## Question-by-Question Record

### Q1: {question header}
- **Background**: Why this question was asked
- **User Response**: {selected options + free text}
- **Impact on Confidence**: FR-003 🔴→🟡 (user provided partial guidance)

## Signal Distribution Change
| Signal | Before Hearing | After Hearing | Delta |
|--------|---------------|---------------|-------|
| 🔵     | 5             | 8             | +3    |
| 🟡     | 3             | 4             | +1    |
| 🔴     | 4             | 0             | -4    |

## Remaining Gaps
- {items that still need clarification}
```

This provides **hearing process traceability** — not just the result (spec)
but the reasoning process that led to it.

---

## 4. SoTOHE-core Differential Hearing (TSUMIKI-03)

### Implementation Location

`.claude/skills/track-plan/SKILL.md`:
- Phase 1 Step 3 (line ~95): Signal classification when existing spec.json found
- Phase 1 Step 4 (line ~138): Differential vs. full hearing branching
- Phase 3 Step 3 (line ~299): Presentation format for hearing results

### Mechanism

```
1. Read spec.json (structured JSON with sources array)
2. Run `bin/sotp track signals <track-id>` (Rust CLI, deterministic evaluation)
3. Classify each requirement into 4 categories:
   🔵 Confirmed  — highest source is document/feedback/convention → skip
   🟡 Needs Check — highest source is inference/discussion → ask confirmation
   🔴 Needs Discussion — no source or empty sources → mandatory question
   ❌ Missing — detected by heuristics but absent from spec.json
4. Present only 🟡/🔴/❌ items to user
5. Update spec.json with user responses
6. Re-evaluate signals via sotp CLI
7. Regenerate views (spec.md, plan.md, registry.md)
```

### Fallback Design

- If `sotp track signals` fails: read `sources` array directly from spec.json
  and classify using the same rules (avoids false-all-Red on tool failure)
- If no spec.json exists: fall back to full hearing (5 fixed questions)

---

## 5. Comparative Analysis

### 5.1 Signal System

| Aspect | Tsumiki | SoTOHE-core |
|--------|---------|-------------|
| Signal definition | Prompt instruction ("tag each item with 🔵🟡🔴") | Rust domain types: `ConfidenceSignal`, `SignalBasis` |
| Signal evaluation | LLM judgment at generation time | Deterministic: `evaluate_requirement_signal()` based on `sources` array |
| Signal storage | Inline in Markdown (🔵 prefix) | `spec.json` → `signals` field (computed, not stored per-item) |
| Signal per item | Yes (each requirement line) | Yes (each scope/constraint/criterion item has `sources`) |
| Signal validation | None (prompt honor system) | `sotp track signals` CLI + Stage 2 AST scan for domain_states |
| Signal propagation | All phases (requirements → design → tasks → TDD) | Spec-level only (does NOT propagate to plan.md tasks) |

**Verdict**: SoTOHE-core's signal backend is categorically stronger (typed,
deterministic, validated). Tsumiki's signal propagation across phases is a
gap in SoTOHE-core.

### 5.2 Differential Hearing

| Aspect | Tsumiki | SoTOHE-core |
|--------|---------|-------------|
| Diff basis | "Read docs, ask about gaps" (implicit LLM judgment) | `sources` array → signal level → 4-category classification (explicit) |
| Gap detection | 5-category question template | 3 heuristics (domain_states, tech-stack, conventions) |
| Question format | `AskUserQuestion` + multiSelect (structured) | Markdown block (free text) |
| Mode selection | Full / Lightweight / Custom | None (always full flow) |
| Process record | `interview-record.md` (signal delta tracking) | None (only spec.json diff in git) |
| Fallback | N/A (always reads existing docs) | Full hearing when no spec.json exists |

**Verdict**: SoTOHE-core's diff basis is more rigorous (source-based
classification vs. LLM intuition). Tsumiki's UX (structured questions,
mode selection, process recording) is more refined.

### 5.3 Artifact Quality

| Aspect | Tsumiki | SoTOHE-core |
|--------|---------|-------------|
| SSoT format | Markdown files (parse-fragile) | JSON (spec.json, metadata.json) |
| Schema validation | None | JSON schema + Rust type deserialization |
| View generation | Direct write (Markdown IS the artifact) | `spec.json` → `spec.md` (read-only view via `render_spec()`) |
| EARS notation | Yes (When/If/While/Shall) | No (natural language) |
| Acceptance criteria | Given-When-Then (separate file) | Natural language in `acceptance_criteria` array |
| User stories | Separate file with epic→story hierarchy | Not produced |

**Verdict**: SoTOHE-core's data integrity is stronger. Tsumiki's requirement
expressiveness (EARS + Given-When-Then) is stronger.

### 5.4 Review and Validation

| Aspect | Tsumiki | SoTOHE-core |
|--------|---------|-------------|
| External review | None (self-contained) | Mandatory external reviewer (Codex CLI) |
| Implementation coverage | `tdd-verify-complete` (80%+ requirement coverage) | None (TSUMIKI-04 pending) |
| Approval gate | None (auto-proceed) | Plan approval (explicit user confirmation) |
| Spec approval | None | `cargo make spec-approve` (content_hash + approved_at) |

**Verdict**: SoTOHE-core's review rigor is categorically stronger.

---

## 6. What SoTOHE-core Should Adopt

### Priority 1: Structured Question UX (S difficulty, high value)

Tsumiki's `AskUserQuestion` + `multiSelect` pattern significantly reduces
user cognitive load. Current SoTOHE-core presents a Markdown wall that
requires users to read everything and respond in free text.

Adoption approach:
- Modify SKILL.md Step 4 to use `AskUserQuestion` tool calls
- Each 🟡/🔴/❌ item becomes a structured question with options
- Options: "Confirm as-is", "Modify (explain)", "Remove", "Defer"
- For ❌ (missing) items: "Add to spec", "Not needed", "Need more info"

### Priority 2: Workload Mode Selection (S difficulty, medium value)

Add scope selection at the start of `/track:plan`:

```
AskUserQuestion:
  header: "Hearing Mode"
  question: "How comprehensive should the requirements hearing be?"
  options:
    - "Full: Complete hearing with all phases"
    - "Focused: Only 🟡/🔴/❌ items (differential)"
    - "Quick: Confirm existing spec, ask about new items only"
```

This avoids running the full Phase 1-3 pipeline for minor updates.

### Priority 3: Interview Record (S difficulty, medium value)

After hearing completion, generate a summary section in `verification.md`
or a new `hearing-record` section in spec.json:

```json
{
  "hearing_history": [
    {
      "date": "2026-04-01T15:17:00Z",
      "mode": "differential",
      "signal_delta": { "blue": [8, 12], "yellow": [6, 2], "red": [0, 0] },
      "questions_asked": 4,
      "items_added": 1,
      "items_modified": 3
    }
  ]
}
```

This provides process traceability without a separate Markdown file.

### Priority 4: Signal Propagation to Tasks (M difficulty, high value)

Currently `spec.json` signals do not flow to `metadata.json` tasks.
A task that depends on a 🟡 spec item should inherit a "needs verification"
flag. This requires:
- `task_refs` in spec.json already link criteria → tasks
- Reverse lookup: for each task, collect signal levels of linked criteria
- Propagate worst-case signal to task metadata
- Block `implementing` transition if any task depends on 🔴 criteria

This aligns with both TSUMIKI signal propagation and CC-SDD-01 traceability.

### Priority 5: EARS Notation (M difficulty, medium value)

Phase 3 candidate. Would require:
- spec.json schema extension (requirement `format: "ears"` field)
- SKILL.md template update for EARS syntax
- Reviewer checklist addition

### Not Recommended for Adoption

| Tsumiki Feature | Reason to Skip |
|----------------|----------------|
| TDD rollback mechanism | Review cycle serves the same purpose |
| Reverse engineering commands | New-project template, not needed |
| rulesync (multi-tool support) | Claude Code-only is sufficient |
| Task notes (tasknote) | metadata.json + Git Notes cover this |
| Model hierarchy (opus/sonnet/haiku) | agent-profiles.json already handles this |

---

## 7. Honest Assessment: How Much Does TSUMIKI-03 Reproduce?

### What TSUMIKI-03 Successfully Reproduces

1. **Core concept**: "Don't re-ask confirmed items" — faithfully implemented
2. **Signal-based classification**: Exceeded tsumiki (deterministic vs. LLM judgment)
3. **Fallback behavior**: Graceful degradation when no spec.json exists
4. **Source-based diff**: More rigorous than tsumiki's implicit "read and judge"
5. **spec.json update flow**: Structured update with signal re-evaluation

### What TSUMIKI-03 Does NOT Reproduce

1. **Structured question format**: Tsumiki uses tool-based structured questions;
   SoTOHE-core uses prose Markdown blocks
2. **Workload mode selection**: No equivalent (always full flow)
3. **Interview record**: No process traceability artifact
4. **5-category question classification**: SoTOHE-core has 4 signal categories
   but not tsumiki's 5 question-type categories (design validity, undefined
   details, new needs, impact scope, priority)
5. **EARS notation**: Not adopted
6. **Signal propagation beyond spec**: Signals stop at spec.json level

### Reproduction Rate Estimate

| Dimension | Coverage | Notes |
|-----------|----------|-------|
| Core hearing concept | 95% | Faithfully reproduced, arguably improved |
| Signal system | 120% | Exceeds tsumiki (typed, validated, deterministic) |
| Hearing UX | 30% | Major gap — free text vs. structured questions |
| Process traceability | 10% | Only git diff, no interview record |
| Requirement expressiveness | 20% | No EARS, no Given-When-Then, no user stories |
| Cross-phase propagation | 0% | Signals do not propagate to tasks |
| Workload adaptation | 0% | No mode selection |

**Overall**: The backend (signal evaluation, SSoT, diff classification) is
**stronger than tsumiki**. The frontend (UX, process recording, expressiveness)
is **significantly weaker**. The reproduction targets the right concept but
invests in the wrong layer — infrastructure over user experience.

---

## 8. Recommended Next Steps

1. **Immediate (S difficulty)**: Add `AskUserQuestion` structured questions
   and workload mode selection to SKILL.md. This is the highest-ROI change.

2. **Short-term (S difficulty)**: Add `hearing_history` to spec.json for
   process traceability. Minimal schema change.

3. **Medium-term (M difficulty)**: Implement signal propagation from
   spec.json criteria to metadata.json tasks. Combine with CC-SDD-01
   bidirectional traceability.

4. **Long-term (M difficulty)**: EARS notation adoption for spec.json
   requirements. Requires schema extension and template updates.
