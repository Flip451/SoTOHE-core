# Harness Engineering Landscape — April 2026

Date: 2026-04-07
Scope: Industry-wide survey of agent harness engineering concepts, frameworks, and practices.
Prior art: `harness-engineering-best-practices-2026-03-09.md` (single-source gap analysis)

## 1. Industry Context

2026 marks the shift from "AI agents" to "agent harnesses" as the primary unit of value.
The consensus equation: **Agent = Model + Harness**.

Key signals:
- Martin Fowler / ThoughtWorks published a systematic taxonomy of harness engineering
- Anthropic shipped a three-agent harness (Planning / Generation / Evaluation)
- OpenAI published "Harness Engineering: leveraging Codex in an agent-first world"
- Deloitte projects agent orchestration as a $8.5B market by 2026, $35B by 2030
- Gartner predicts 33% of enterprise software will include agentic AI by 2028

The competitive moat is no longer model capability — it is harness quality.

## 2. Fowler Taxonomy (ThoughtWorks)

Source: https://martinfowler.com/articles/exploring-gen-ai/harness-engineering.html

### 2.1 Two Control Mechanisms

| Mechanism | Timing | Purpose | Examples |
|---|---|---|---|
| **Guides (Feedforward)** | Before action | Steer behavior proactively | CLAUDE.md, architecture rules, conventions |
| **Sensors (Feedback)** | After action | Validate and self-correct | Tests, linters, review agents |

Critical insight: "Feedback-only approaches create agents that repeat mistakes.
Feedforward-only approaches never validate whether rules worked."
Both are required for reliable harnesses.

### 2.2 Two Execution Types

| Type | Characteristics | Examples |
|---|---|---|
| **Computational** | Deterministic, fast (ms-sec) | Linters, type checkers, tests, structural analysis |
| **Inferential** | Semantic, probabilistic, slower | Code review agents, complex judgment |

Computational controls provide reliable guardrails; inferential controls offer richer
guidance but with probabilistic outcomes.

### 2.3 Three Regulation Dimensions

**Maintainability Harness** (most mature):
- Computational sensors catch structural issues: duplication, complexity, coverage, style
- Inferential sensors partially address semantic issues like redundant logic
- Neither reliably catches misdiagnosis or instruction misunderstanding

**Architecture Fitness Harness** (fitness functions):
- Performance requirement guides + performance test sensors
- Observability conventions + debugging validation
- Structural dependency rules + layer enforcement

**Behaviour Harness** (least mature, biggest gap):
- Functional specifications (feedforward) + AI-generated test suites (feedback)
- Fowler warns: "Puts a lot of faith into AI-generated tests — that's not good enough yet"
- Most teams supplement with manual testing

### 2.4 Lifecycle Integration

Quality controls distribute across three stages:

| Stage | Speed | Examples |
|---|---|---|
| **Pre-integration** | Fast | Language servers, linters, fast test suites, basic review agents |
| **Post-integration** | Expensive | Mutation testing, architectural reviews, semantic analysis |
| **Continuous monitoring** | Drift detection | Dead code detection, dependency scanning, SLO analysis, anomaly detection |

### 2.5 "Harnessability" (Ambient Affordances)

Environmental properties that make systems inherently agent-governable:
- Strong type systems
- Clear module boundaries
- Framework conventions
- Explicit error handling patterns

Greenfield projects can embed harnessability from inception.
Legacy systems face steeper challenges — ironically where harnesses are most needed.

### 2.6 Harness Template Pattern

Organizations typically have 3-5 service topologies covering 80% of needs.
Pre-built harness templates bundling guides, sensors, conventions, and structural
patterns reduce configuration burden and ensure consistency.

## 3. Anthropic Three-Agent Harness

Source: https://www.infoq.com/news/2026/04/anthropic-three-agent-harness-ai/

### Architecture

Three specialized agents with structured handoffs:

| Agent | Role | Characteristics |
|---|---|---|
| **Planning** | Requirements decomposition, task planning | Establishes scope and acceptance criteria |
| **Generation** | Code creation and implementation | Executes based on plan artifacts |
| **Evaluation** | Independent quality assessment | Uses few-shot examples + explicit scoring criteria |

Key design insight: **"Separating the agent doing the work from the agent judging it
proves to be a strong lever"** for quality assessment.

### Context Management

- Context resets with defined state artifacts (not context compaction)
- Structured JSON specs for continuity across resets
- Enforcement testing and initialization scripts
- 5-15 iterations per session, up to 4 hours autonomous execution

### Evaluation Calibration (Frontend-specific)

Four assessment metrics: design quality, originality, craft, functionality.
Few-shot examples anchor the evaluator's judgment.

## 4. Practitioner Patterns

Source: https://dev.to/tacoda/the-agent-harness-turning-ai-slop-into-shipping-software-589i

### Four-Stage Adoption Framework (Ian Johnson)

**Stage 1: Foundation**
- Characterization tests first ("reins, not the saddle")
- Linting + pre-commit hooks as machine-checkable standards
- Tests as "communication protocol with agents"

**Stage 2: Refactoring for Safety**
- Extract traits into service classes with clear contracts
- Explicit, "boring" architecture that agents can navigate
- Authorization via explicit policies

**Stage 3: Migration Strategy**
- Dual frontends with environment gating
- Feature delivery throughout refactoring

**Stage 4: The Harness System**
- Transition from "in-the-loop" to "on-the-loop"
- Scoped guidance files (subdirectory CLAUDE.md)
- Feedback loops that update harness files based on review findings

### Key Principles

- **Guardrails over instructions**: Codify judgment through boundaries, not verbose docs
- **Small batches**: Trunk-based development, short-lived branches (hours not days)
- **Curator role**: Engineers shift from writing code to curating repository design
- **Each stage narrows failure space**: Tests → Linting → Architecture → Harness

## 5. OpenAI Harness Engineering

Source: https://openai.com/index/harness-engineering/ (403 — summary from search results)

Key practices referenced:
- Layered architecture enforced through custom linters and structural tests
- Regular "garbage collection" scanning for drift
- Steering loop: observe failure patterns → strengthen controls iteratively

## 6. Industry Real-World Patterns

### Stripe
- Pre-push hooks with heuristic-based linting
- Blueprints integrating feedback sensors into workflows
- Emphasis on shifting feedback left

### ThoughtWorks
- Mixed computational/inferential sensors for drift detection
- "Janitor armies" for continuous code quality improvement

### Common Anti-Patterns
- Relying solely on feedback (test-after) without feedforward (guides)
- Over-investing in model capability while under-investing in harness
- Treating harness as one-time setup rather than continuous evolution

## 7. Gap Analysis: SoTOHE-core vs Industry

### Covered (Strong)

| Industry Concept | SoTOHE-core Implementation |
|---|---|
| Guides (Feedforward) | `.claude/rules/`, `knowledge/conventions/`, `architecture-rules.json`, `CLAUDE.md` |
| Computational Sensors | `cargo make ci` (clippy, fmt-check, deny, check-layers, test) |
| Inferential Sensors | Codex reviewer cycle (fail-closed, sequential escalation) |
| Planning/Generation/Evaluation separation | planner / implementer / reviewer capabilities |
| Context reset + state artifacts | track metadata.json + spec/plan/verification + state machine |
| On-the-loop operation | hooks + guardrails enabling autonomous execution |
| Escalation mechanism | `sotp review record-round` 3-strike threshold with evidence-based resolution |
| Harnessability | `04-coding-principles.md` (enum-first, typestate, newtype) |
| Agent interchangeability | `agent-profiles.json` capability-to-provider mapping |
| Architecture Fitness | `check-layers`, `deny.toml`, `architecture-rules.json` |
| Small batches | Guardrail: <500 line commits, per-task review |

### Partially Covered

| Industry Concept | Status | Notes |
|---|---|---|
| Harness Template pattern | Partial | Repo is in `templates/` but not yet parameterized for multiple topologies |
| High-speed feedback | Partial | clippy >1s; no PostToolUse fast-path beyond daemon |
| Continuous drift detection | Partial | `cargo make machete` exists but not scheduled/continuous |
| Harness evolution feedback loop | Partial | Review findings update code but don't systematically update harness rules |

### Gaps (Investment Opportunities)

| Gap | Industry Recommendation | Priority | Rationale |
|---|---|---|---|
| **Behaviour Harness** | spec → acceptance test auto-generation pipeline | High | Fowler identifies this as weakest link; SoTOHE relies on human-written tests |
| **Drift Detection** | Post-merge continuous scanning (dead code, arch drift, dependency staleness) | Medium | Pre-integration is strong but post-integration monitoring is minimal |
| **Harness Template Parameterization** | 3-5 topology templates with bundled guides/sensors | Medium | Natural evolution of `templates/` repo; multiplies harness value |
| **Conflicting Signal Resolution** | Priority framework when multiple sensors disagree | Low | Current escalation handles repetition but not contradiction between reviewers |
| **Harnessability as Explicit Goal** | Document "agent success probability" as a design principle motivation | Low | Principles exist but aren't framed as harness optimization |

## 8. Terminology Map

| Industry Term | SoTOHE-core Equivalent |
|---|---|
| Agent Harness | SoTOHE-core (the entire system) |
| Guides / Feedforward | `.claude/rules/`, conventions, `CLAUDE.md` |
| Sensors / Feedback | CI gates, reviewer cycle, hooks |
| Computational Sensor | clippy, fmt-check, deny, check-layers, tests |
| Inferential Sensor | Codex reviewer (external LLM judgment) |
| Fitness Function | `architecture-rules.json` + `check-layers` |
| Harnessability | `04-coding-principles.md` design patterns |
| Harness Template | `templates/SoTOHE-core` (single topology today) |
| Context Reset | track state machine transitions |
| State Artifact | metadata.json, spec.md, plan.md, verification.md |
| Steering Loop | review → fix → review cycle with escalation |
| In-the-loop → On-the-loop | progressive autonomy via hooks/guardrails |
| AI Managed Service (fukkyy) | Harness + Agent as packaged delivery |

## 9. Strategic Implications

1. **SoTOHE-core is ahead of most practitioners** — the Fowler taxonomy validates
   what was built organically. The system has both feedforward and feedback,
   computational and inferential, across maintainability and architecture dimensions.

2. **Behaviour Harness is the next frontier** — the weakest dimension industry-wide.
   Investing here (spec-to-test pipeline, property-based testing, mutation testing)
   would extend the moat beyond what most harnesses achieve.

3. **Template parameterization is the scaling play** — the harness is currently
   coupled to one project topology. Extracting topology-agnostic patterns into
   reusable templates (as Fowler recommends) multiplies the value.

4. **The "curator" role shift is real** — harness engineering redefines the engineer's
   job from "write code" to "design the system that makes agents write good code."
   SoTOHE-core's investment in rules, conventions, and architecture enforcement
   aligns with this shift.

## Sources

- [Harness engineering for coding agent users — Martin Fowler](https://martinfowler.com/articles/exploring-gen-ai/harness-engineering.html)
- [Anthropic Three-Agent Harness — InfoQ](https://www.infoq.com/news/2026/04/anthropic-three-agent-harness-ai/)
- [The Agent Harness: Turning AI Slop Into Shipping Software — DEV Community](https://dev.to/tacoda/the-agent-harness-turning-ai-slop-into-shipping-software-589i)
- [Harness Engineering: leveraging Codex — OpenAI](https://openai.com/index/harness-engineering/)
- [Harness Engineering: The Missing Layer — Louis Bouchard](https://www.louisbouchard.ai/harness-engineering/)
- [The Agent Harness: Infrastructure Not Intelligence — Hugo Nogueira](https://www.hugo.im/posts/agent-harness-infrastructure)
- [2025 Was Agents, 2026 Is Agent Harnesses — Aakash Gupta](https://aakashgupta.medium.com/2025-was-agents-2026-is-agent-harnesses-heres-why-that-changes-everything-073e9877655e)
- [AI Agent Orchestration — Deloitte](https://www.deloitte.com/us/en/insights/industry/technology/technology-media-and-telecom-predictions/2026/ai-agent-orchestration.html)
- [What is an Agent Harness — Parallel AI](https://parallel.ai/articles/what-is-an-agent-harness)
- [What Is Harness Engineering? Complete Guide — NxCode](https://www.nxcode.io/resources/news/what-is-harness-engineering-complete-guide-2026)
- [エージェントハーネスとAIマネージドサービス — 福島良典 (LayerX)](https://note.com/fukkyy/n/n1d8fce44e67a)
