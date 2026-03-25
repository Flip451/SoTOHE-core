# Harness Design Analysis

> **Source**: [Harness Design for Long-Running Application Development](https://www.anthropic.com/engineering/harness-design-long-running-apps) (Anthropic Engineering Blog, Prithvi Rajasekaran)
> **External guide**: `docs/external-guides.json` id=`harness-design-long-running-apps`
> **Analysis date**: 2026-03-25
> **TODO-PLAN version**: v3 (2026-03-22, updated 2026-03-25)

---

## 1. Article Summary

The article documents techniques for improving Claude's performance on complex, extended tasks through multi-agent architectural patterns.

### Core Problems

| Problem | Description |
|---|---|
| **Context degradation** | Models lose coherence as context fills. "Context anxiety" causes premature task completion. |
| **Self-evaluation blindness** | Agents praise their own mediocre work. Tuning an external evaluator to be skeptical is far more tractable than making a generator self-critical. |

### Key Patterns

1. **Generator-Evaluator separation** (GAN-inspired): Separate execution from evaluation. External evaluator tuned for skepticism.
2. **Three-agent architecture**: Planner (high-level spec) -> Generator (implementation) -> Evaluator (Playwright-based testing against hard thresholds).
3. **Sprint contracts**: Generator and Evaluator negotiate explicit "done" criteria before implementation, bridging user stories and testable specs.
4. **File-based artifact handoff**: State carried via files, not context compaction, preserving clarity across sessions.
5. **Assumption stress-testing**: "Every component in a harness encodes an assumption about what the model can't do independently." These assumptions require continuous re-evaluation as models improve.

### Measured Results

- Solo run: 20 min / $9 -- broken gameplay despite polished UI.
- Full harness: 6 hours / $200 -- functional game with working mechanics.
- As models improved (Opus 4.5 -> 4.6), some harness components became unnecessary, while the evaluator remained valuable for tasks exceeding generator capability.

---

## 2. SoTOHE-core Alignment Map

### Already Realized

| Article Pattern | SoTOHE-core Implementation | Status |
|---|---|---|
| Generator-Evaluator separation | implementer + reviewer capability split; inline review prohibited (`feedback_no_inline_review`) | Active |
| Three-agent architecture | planner(Codex) -> implementer(Claude) -> reviewer(Codex) via `/track:plan` -> `/track:implement` -> `/track:review` | Active |
| Sprint contracts | `spec.md` + `verification.md` define acceptance criteria before implementation | Active |
| File-based artifact handoff | `metadata.json` as SSoT, `plan.md`/`registry.md` as rendered views | Active |
| Self-evaluation blindness avoidance | External reviewer mandatory; self-review blocked; escalation threshold (3 consecutive same-concern cycles) | Active |
| Evaluator tuning | Review sequential escalation (fast model -> full model); findings-to-concerns extraction | Active |
| Context degradation mitigation | All state persisted to files; subagent context isolation | Active |

### Partially Realized

| Article Pattern | SoTOHE-core Gap | Roadmap Item |
|---|---|---|
| Sprint contract negotiation | `verification.md` is planner-authored, not negotiated between implementer and reviewer | See Action 1 below |
| Evaluator accuracy metrics | No verdict precision tracking (false positive rate, escalation frequency) | Phase 3: WF-47/50/53 |
| Assumption documentation | `agent-profiles.json` allows provider switching, but no ADR records *why* each capability is separated | See Action 2 below |
| Cost/time tracking | No per-track API cost or wall-time recording | Phase 5: GAP-11 (tracing) |

### Not Yet Applicable (Future Phases)

| Article Pattern | SoTOHE-core Future | Phase |
|---|---|---|
| Per-project harness customization | `sotp init` with capability selection | v4 Phase 6 (draft) |
| Harness component stripping on model improvement | Periodic capability consolidation review | No roadmap item yet |

---

## 3. Actionable Recommendations

### Action 1: Reviewer-testable acceptance criteria (Phase 2 follow-up)

**Context**: CC-SDD-01 (traceability) and CC-SDD-02 (approval gate) are both done. The article's sprint contracts pattern suggests the next step: make `verification.md` criteria reviewer-parseable.

**Proposal**: In `/track:plan` skill, generate `verification.md` items in a format the reviewer can systematically check (e.g., file existence, function signature, test name). This reduces out-of-scope findings during review cycles.

**Effort**: S (prompt change in `/track:plan` skill)
**Dependency**: None (CC-SDD-01/02 already provide the structural foundation)

### Action 2: Capability separation ADRs (Phase 1.5-18 prerequisite)

**Context**: The article's most important insight -- "every component encodes an assumption about model limitations" -- applies directly to `agent-profiles.json` capability definitions.

**Proposal**: When adding `domain_modeler`, `spec_reviewer`, `acceptance_reviewer` capabilities (Phase 1.5-18), record an ADR for each answering: "What model limitation does this separation address?" This enables future consolidation decisions when models improve.

**Effort**: S (documentation alongside 1.5-18 implementation)
**Dependency**: Phase 1.5-18

### Action 3: Verdict accuracy metrics (Phase 3)

**Context**: The article reports that evaluator tuning required multiple iterations, and initial versions had poor judgment. SoTOHE-core's review escalation threshold is a reactive mechanism; proactive accuracy tracking would enable systematic tuning.

**Proposal**: Track per-track metrics in `metadata.json`:
- `review_rounds_total`: total fast+final rounds
- `escalation_triggered`: boolean
- `false_positive_estimate`: out-of-scope findings / total findings (available via RVW-11 scope filtering)

**Effort**: M (domain type extension + CLI reporting)
**Dependency**: RVW-10/11 (done), RVW-13/15/17 (in progress)

### Action 4: Periodic assumption review (operational)

**Context**: The article warns that harness assumptions become load-bearing and require stress-testing when models improve.

**Proposal**: Add a lightweight review trigger: when `agent-profiles.json` provider models are updated (e.g., gpt-5.4 -> next generation), run a "capability necessity audit" -- temporarily disable each non-orchestrator capability and measure impact on a reference track.

**Effort**: L (requires reference track + measurement framework)
**Dependency**: Phase 3 completion (need metrics infrastructure from Action 3)

---

## 4. Summary

SoTOHE-core's multi-agent architecture is well-aligned with the article's recommendations. The project independently arrived at Generator-Evaluator separation, file-based handoff, and evaluator skepticism tuning before this article was published.

The primary gaps are:
1. **Sprint contract negotiation** -- verification criteria are planner-authored, not reviewer-validated
2. **Assumption documentation** -- capability separations lack explicit rationale ADRs
3. **Evaluator accuracy tracking** -- no quantitative feedback loop for reviewer tuning

With Phase 2 nearly complete (5/6 items done, TSUMIKI-03 remaining), the project is approaching Phase 3 (Moat: test generation pipeline). Actions 1 and 2 are low-effort improvements that can be folded into ongoing work. Action 3 aligns with existing Phase 3 roadmap items. Action 4 is a longer-term operational practice.
