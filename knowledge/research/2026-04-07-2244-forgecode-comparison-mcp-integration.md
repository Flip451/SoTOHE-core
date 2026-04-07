# ForgeCode vs SoTOHE Comparison & MCP Integration Strategy

Date: 2026-04-07

## 1. Project Positioning

| Aspect | ForgeCode | SoTOHE-core |
|--------|-----------|-------------|
| **Category** | AI coding agent (product) | AI-collaborative development workflow framework (template) |
| **Purpose** | Provide AI coding assistance from the terminal | Structurize Rust development process with AI multi-agent collaboration |
| **Target** | End-user developers | Rust project teams (with AI workflow) |
| **Maturity** | Released product | In-development template/framework |

## 2. Architecture Comparison

| Aspect | ForgeCode | SoTOHE-core |
|--------|-----------|-------------|
| **Language** | Rust (Edition 2024) | Rust (Edition 2024) |
| **Structure** | `crates/*` 22+ crates monorepo | Hexagonal architecture (domain/usecase/infra/cli 4 layers) |
| **Design patterns** | Service layer based | Hexagonal + Typestate + Enum-first |
| **Type safety** | Standard Rust type safety | Make Illegal States Unrepresentable (types eliminate invalid states) |
| **Runtime** | `tokio 1.50` (full async) | Fully synchronous (no async) |
| **HTTP** | `reqwest` + rustls + HTTP/2 | `reqwest` blocking |
| **CI/Quality gates** | Standard CI | `sotp verify` + layer dep check + review cycle enforcement |

## 3. Agent Model Comparison

### ForgeCode: 3 fixed specialized agents

| Agent | Role | File modification |
|-------|------|-------------------|
| `forge` | Implementation (feature dev, bug fixes) | Yes |
| `sage` | Research (read-only code comprehension) | No |
| `muse` | Planning (design, strategy) | Yes |

### SoTOHE: 6 capabilities × provider resolution

| Capability | Default provider | Role |
|------------|-----------------|------|
| `planner` | Claude Code | Design, trade-off evaluation |
| `implementer` | Claude Code | Implementation |
| `reviewer` | Codex CLI | Code review |
| `debugger` | Codex CLI | Compile error diagnosis |
| `researcher` | Gemini CLI | Crate research, codebase analysis |
| `multimodal_reader` | Gemini CLI | PDF/image reading |

### Key Difference: Provider Lock vs Provider Abstraction

- **ForgeCode**: Supports 300+ LLMs but agent structure is fixed (forge/sage/muse)
- **SoTOHE**: Capability → provider indirection layer (`agent-profiles.json`), same capability can be routed to different providers. Profile switching (`claude-heavy`/`codex-heavy`)

## 4. Workflow Management Comparison

- **ForgeCode**: Conversation-based. Session persistence, conversation branching (`clone`)
- **SoTOHE**: Track-based. `spec.md` → `plan.md` → implementation → `verification.md` state machine. `metadata.json` is SSoT. Commit blocked until reviewer reports `zero_findings`

## 5. Quality Assurance Depth

### ForgeCode
- Restricted shell mode (file access limitation) — 1 layer

### SoTOHE — 3 layers deep
1. **Compile-time**: clippy `deny` (`unwrap_used`, `indexing_slicing`, `panic` etc. all forbidden)
2. **CI-time**: `syn` AST scan detects I/O leaks in usecase/domain layers, layer dependency check
3. **Workflow-time**: hooks block direct git ops, reviewer `zero_findings` required, escalation threshold (3 same concerns → block)

## 6. Customization Mechanisms

| Mechanism | ForgeCode | SoTOHE |
|-----------|-----------|--------|
| Persistent instructions | `AGENTS.md` | `CLAUDE.md` + `.claude/rules/` |
| Skills | `.forge/skills/<skill>/SKILL.md` (plain Markdown) | `.claude/skills/` |
| Agent definitions | `.forge/agents/*.md` (Markdown + YAML frontmatter) | `.claude/agent-profiles.json` |
| Configuration | `forge.yaml` | `.claude/settings.json` + `architecture-rules.json` |

## 7. ForgeCode's Unique Strengths

1. **ZSH plugin**: `:` prefix for instant shell access
2. **300+ LLM support**: OpenRouter/Bedrock/Groq provider flexibility
3. **Native Git integration**: AI commit message generation, merge conflict resolution
4. **Conversation branching**: `clone` for experimental development paths
5. **Zero-config startup**: API key only, immediate use
6. **Semantic search**: Codebase indexing for meaning-based file discovery

## 8. SoTOHE's Unique Strengths

1. **Type-driven design enforcement**: Typestate, Enum-first as mandatory coding conventions
2. **Formalized review cycle**: `record-round` → `check-approved` → commit state machine. Verdict tamper-proof (SHA-256)
3. **Escalation mechanism**: 3 same concerns auto-block → researcher investigation → `resolve-escalation` with evidence
4. **Full traceability**: spec → plan → implementation → verification all tracked
5. **Architecture enforcement**: `architecture-rules.json` + `deny.toml` + `check_layers.py` detect layer violations in CI
6. **Test generation pipeline** (Phase 3 planned): Types shrink state space → spec generates massive tests → implementation just passes tests

## 9. MCP Integration Strategy

### Core Insight

SoTOHE's real value is the `sotp` domain layer — type-safe state management that makes invalid states unrepresentable. This is currently tightly coupled to Claude Code. Exposing it as an MCP server makes it host-independent.

### Architecture

```
ForgeCode (forge/sage/muse)
    ↓ MCP protocol
sotp-mcp-server
    ↓
sotp domain + usecase (type-safe state machine)
    ↓
sotp infrastructure (file persistence)
```

### Why MCP

| Approach | Pros | Cons |
|----------|------|------|
| Publish domain as crate | Direct type access | Invades ForgeCode build, strong version coupling |
| CLI invocation | Loose coupling | String parsing, weak error handling |
| **MCP server** | **ForgeCode already supports MCP, JSON Schema preserves type info, loose coupling** | MCP spec compliance cost |

### MCP Tools to Expose

#### Track state machine
- `track_create`: Create track with spec confidence signals
- `track_transition`: State transition (planned→in_progress→done). Blocked if red signals exist.
- `track_status`: Query derived status (computed from tasks, never stored)

#### Task state machine
- `task_transition`: Task state transition (Todo→InProgress→Done{commit_hash}|Skipped). Domain rejects invalid transitions.

#### Review cycle
- `review_record_round`: Record review round. Enforces escalation threshold (3 same concerns → EscalationActive error)
- `review_check_approved`: Check if review cycle allows commit. Returns bool, never falsifiable.
- `review_resolve_escalation`: Resolve escalation with evidence artifacts

#### Guard
- `guard_evaluate`: Evaluate shell command safety. Fail-closed (parse error → Block)

#### Verification
- `verify_spec_signals`: Verify no red signals remain in spec
- `verify_layer_purity`: AST scan for I/O leaks in domain/usecase layers

### The Key Value: Type Enforcement, Not Prompt Enforcement

Prompt-based rules ("don't commit before review") are ignorable by LLMs. `sotp` MCP tools enforce rules structurally:

- `review_check_approved` returns `false` → agent cannot commit (tool refuses)
- No "skip review" tool exists → impossible at the type level
- `track_transition` rejects invalid state transitions → domain error, not prompt violation

**"Make Illegal States Unrepresentable" applied to AI agent control.**

### ForgeCode-side Integration

```jsonc
// .mcp.json (ForgeCode's MCP config format)
{
  "mcpServers": {
    "sotp": {
      "command": "sotp-mcp-server",
      "args": ["--project-dir", "."]
    }
  }
}
// Registration: edit .mcp.json directly, then run `forge mcp reload`
```

```md
<!-- .forge/agents/forge-guarded.md -->
---
id: forge-guarded
title: SoTOHE-Guarded Implementation Agent
description: Implementation agent with SoTOHE state management
---

You have access to sotp MCP tools for state management.

RULES:
1. Before implementing, call verify_spec_signals. Stop if red signals exist.
2. Before each commit, call review_check_approved. Never commit without approval.
3. Use task_transition to track progress. The tool rejects invalid transitions.
4. If review_record_round returns EscalationActive, stop and report to user.

You cannot bypass these checks — they are enforced by Rust's type system
in the sotp domain layer, not by this prompt.
```

## 10. Integration Approach: CLI + Skill (Recommended)

### MCP vs CLI + Skill Evaluation

| Approach | Argument safety | Hallucination risk | Implementation cost | Multi-host reuse |
|----------|----------------|-------------------|-------------------|-----------------|
| **MCP** | Schema-validated | Low (enum choices) | New crate + MCP transport | One schema for all hosts |
| **CLI + Skill** | Skill instructions guide LLM | Low (skill constrains usage) | **Zero (CLI already exists)** | Skill per host |
| CLI only (no skill) | Free-form string | High | Zero | Manual |

### Decision: CLI + Skill first, MCP only if needed later

The existing `sotp` CLI already provides all the state management functions. ForgeCode's
`.forge/skills/` can wrap CLI commands with structured instructions, achieving the same
practical outcome as MCP without any implementation cost.

**What Skills solve:**
- Correct CLI usage (argument names, order, flags)
- Workflow sequencing (check review → transition → commit)
- Error handling instructions (what to do on failure)

**What Skills don't solve (but MCP would):**
- Schema-level argument validation (typos still possible, caught by CLI at runtime)
- Multi-host deployment with a single definition (each host needs its own skill format)

**When to upgrade to MCP:**
- When sotp needs to serve 3+ different AI hosts simultaneously
- When CLI argument hallucination becomes a recurring problem in practice

### Skill Examples

```md
<!-- .forge/skills/sotp-commit/SKILL.md -->
# sotp-commit: Review-gated commit via sotp CLI

1. Run: sotp review check-approved --track-id {track_id}
2. If approved=false → STOP, tell user review not complete
3. If approved=true → proceed to commit
Never bypass this sequence. Never call git commit without step 1 passing.
```

```md
<!-- .forge/skills/sotp-implement/SKILL.md -->
# sotp-implement: Implementation with spec signal gate

1. Run: sotp track signals --items-dir track/items {track_id}
2. If red signals exist → STOP, report to user
3. Run: sotp track transition --items-dir track/items {track_id} {task} in_progress
4. Implement the task
5. On completion: sotp track transition --items-dir track/items {track_id} {task} done
```

### Remaining Risk: Skill Bypass

Both CLI+Skill and MCP share the same residual risk: the LLM can ignore
skill instructions and run `git commit` directly via Bash. MCP does not
eliminate this because the Bash tool remains available alongside MCP tools.

Mitigation is the same in both cases: host-level hooks (ForgeCode's restricted
shell mode, SoTOHE's `block-direct-git-ops` hook).

### Implementation Roadmap (Revised)

```
Step 1: Write ForgeCode skills wrapping sotp CLI (cost: ~zero)
        - sotp-commit, sotp-implement, sotp-review skills
        - forge-guarded agent definition referencing these skills
            ↓
Step 2: Test in practice, measure hallucination/error rate
            ↓
Step 3: If CLI errors are frequent → upgrade to MCP server
        If not → stay on CLI + Skill (YAGNI)
```

### Decoupling Diagram

```
Current:   sotp ←tight coupling→ Claude Code (.claude/skills/)
Step 1:    sotp CLI ←Skill→ ForgeCode (.forge/skills/)
                   ←Skill→ Claude Code (.claude/skills/)
Step 3:    sotp-mcp ←MCP→ Any MCP-compatible host (only if needed)
```

## 11. MCP Token Cost Considerations

### The Real Cost: Tool Schema Injection, Not MCP Itself

MCP is not inherently expensive. The token cost comes from **tool definitions being injected into the context on every turn** — the same mechanism used by built-in tools (Read, Edit, Bash, etc.).

```
Per-turn context:
  system prompt              ~fixed
  + tool definitions × N     ← this grows with tool count
  + conversation history
  + user message
```

### Approximate Token Overhead

| Factor | Token cost | Frequency |
|--------|-----------|-----------|
| Tool definition (schema) | ~100-300 tokens/tool | **Every turn** |
| Tool call (arguments) | Variable | On use only |
| Tool result (response) | Variable | On use only |

### Design Principle: Minimize Tool Count

Tool definitions are sent every turn regardless of use. More tools = more wasted tokens.

```
Bad design: 30 fine-grained tools
  → ~9,000 tokens/turn schema overhead
  → Unused tools still cost tokens

Good design: 5-7 coarse tools
  → ~1,500 tokens/turn
  → Acceptable range
```

### sotp-mcp Tool Consolidation Strategy

Consolidate from 12 fine-grained tools to 4 coarse tools using action parameters:

```
Fine-grained (NG — 12 tools):
  track_create, track_transition, track_status,
  task_create, task_transition, task_status,
  review_record_round, review_check_approved, review_resolve_escalation,
  guard_evaluate, verify_spec_signals, verify_layer_purity

Consolidated (OK — 4 tools):
  track_manage    (action: create|transition|status)
  task_manage     (action: transition|status)
  review_manage   (action: record|check|resolve)
  verify          (target: spec_signals|layer_purity|guard)
```

Trade-off: fewer tools shifts decision-making to the model, but constraining choices via `enum` in the JSON Schema keeps control sufficient.

### Conclusion on Token Cost

- "MCP is expensive" → **misconception** (same mechanism as built-in tools)
- "Publishing many tools is expensive" → **true** (definitions injected every turn)
- sotp-mcp with 4-7 tools → **practically no problem** (~1,500 tokens/turn overhead)

## 12. Cross-Learning Opportunities

### What ForgeCode can learn from SoTOHE
1. Spec Confidence Signals — requirement confidence tracking has universal value
2. Formalized review cycle — verdict tamper-proof (SHA-256), escalation threshold
3. Layer dependency CI enforcement — `architecture-rules.json` → `deny.toml` → CI chain
4. Type-driven design philosophy — reduce test count via types

### What SoTOHE can learn from ForgeCode
1. ZSH plugin UX — `:` prefix immediacy vs `/track:*` verbosity
2. Conversation branching (`clone`) — lighter than `plan-only` branches
3. Declarative agent YAML definitions — more extensible than `agent-profiles.json`
4. Semantic search — complement `researcher` capability with indexed codebase understanding

## 13. Review Architecture: Codex as External Reviewer + sotp Enforcement

### Why ForgeCode's Missing Reviewer Is Not a Problem

ForgeCode's 3-agent model (forge/sage/muse) has no dedicated reviewer. This is correct:
**an implementer reviewing its own output is structurally unsound.** The reviewer must be
a separate system with independent judgment.

### Architecture: forge + Codex + sotp

```
forge (implementation)
  ↓ completes task
sotp review cycle (enforced by CLI/Skill)
  ↓ delegates to
Codex CLI (external reviewer, independent model)
  ↓ returns verdict
sotp record-round (records verdict, checks escalation)
  ↓
zero_findings? → sotp check-approved → commit allowed
findings?      → forge fixes → re-review (loop)
3× same concern → sotp escalation block → user decision required
```

### Key Design Decisions

1. **Reviewer is always external**: Codex CLI, not forge/sage/muse. This ensures
   independence — the reviewer has no shared context or incentive alignment with
   the implementer.

2. **sotp enforces the cycle**: The review loop is not optional. `sotp review check`
   must return `approved=true` before commit is possible. This is enforced via
   git pre-commit hook (host-independent) + ForgeCode skill instructions.

3. **Escalation is automatic**: 3 consecutive rounds with the same concern category
   triggers `EscalationActive` in sotp's domain layer. Neither forge nor Codex can
   override this — only a human with evidence artifacts can resolve it.

### ForgeCode Skill for Review Cycle

```md
<!-- .forge/skills/sotp-review/SKILL.md -->
# sotp-review: Run Codex review cycle enforced by sotp

1. Stage changes: cargo make add-all
2. Run Codex reviewer:
   cargo make track-local-review -- --prompt "Review staged changes"
3. Parse verdict from reviewer output
4. Record round: sotp review record-round --track-id {id} --verdict {verdict} --concerns {concerns}
5. If verdict=findings_remain:
   - Fix findings
   - Go to step 1 (max 3 rounds per concern before escalation)
6. If verdict=zero_findings:
   - Verify: sotp review check --track-id {id}
   - Proceed to commit
```

### Why This Is Better Than Claude Code's Current Approach

| Aspect | Claude Code (current) | ForgeCode + Codex + sotp |
|--------|----------------------|--------------------------|
| Review enforcement | .claude/hooks (host-specific) | git pre-commit hook (host-independent) |
| Reviewer independence | Codex CLI (same) | Codex CLI (same) |
| Cycle state machine | sotp domain layer (same) | sotp domain layer (same) |
| Host portability | Claude Code only | Any host with Bash access |

The reviewer capability and enforcement logic are already host-independent (Codex CLI + sotp).
The only Claude Code-specific part is the hook that blocks direct git commits, which should
be migrated to a git pre-commit hook regardless of ForgeCode adoption.

## 14. Final Verdict: MCP Investment Not Justified Now

**Do not invest in MCP at this stage.**

MCP's value proposition is "write one tool schema, serve all hosts." This only pays off
when sotp serves 3+ different AI hosts simultaneously and maintaining per-host Skills
costs more than building an MCP server.

Current reality:
- sotp serves 1 host (Claude Code)
- ForgeCode integration (if pursued) is adequately served by CLI + Skill
- CLI already exists — zero implementation cost
- Skill format differences between hosts are trivial (~30 min to port)

**When to revisit:**
- sotp is used by 3+ hosts AND skill maintenance becomes a burden
- A host drops Bash/CLI access and only supports MCP (unlikely near-term)
- MCP ecosystem matures with standardized testing/debugging tools

**YAGNI applies.** Build the MCP server when there is concrete demand, not in anticipation of it.

## 15. ForgeCode Performance Research (2026-04-07)

### Benchmark: TermBench 2.0

- **ForgeCode: #1 at 81.8%** (GPT 5.4 and Opus 4.6, submitted 2026-03-12)
- **Claude Code: #39 at 58.0%** (Opus 4.6)
- Leaderboard: tbench.ai (Stanford x Laude Institute, arxiv 2601.11868)

**Caveats:**
- All results are self-reported (0 verified entries on the leaderboard)
- Top score requires **ForgeCode Services** (proprietary runtime), not reproducible with OSS alone
- TermBench rewards terminal-harness engineering specifically — not general SWE ability
- No SWE-bench, HumanEval, or Aider Leaderboard numbers exist for ForgeCode

### Harness Engineering: 4 Concrete Improvements

1. **Schema field ordering**: `required` before `properties` → fewer malformed tool calls (GPT-5.4)
2. **Schema flattening**: Single-level structures → reduced invocation errors
3. **Explicit truncation markers**: Plain-text "truncated N more lines" → fewer hallucinations
4. **Enforced verification mode**: Agent must review task completion before terminating

These are substantive, technically credible improvements — not marketing. Source: ForgeCode blog
"Benchmarks Don't Matter — Until They Do" and "GPT 5.4 Agent Improvements".

### User Community

- GitHub: 6,100 stars, 1,300 forks (small vs Aider ~20k+)
- Independent review: 78/100 (Speed 92, Privacy 90, Dev Experience 77)
- **Large codebase inconsistency** noted by independent reviewers
- No Claude Code → ForgeCode migration stories found
- HN discussion: minimal engagement
- Most visible "comparison" articles are vendor-written (conflict of interest)

### Cost Structure

ForgeCode has its own subscription tiers (Free/Pro/Max):

```
Claude Code:
  Claude Pro    $20/month  → flat rate, near-unlimited
  Claude Max    $100/month → flat rate, high throughput

ForgeCode (source: forgecode.dev/pricing, 2026-04-07):
  Free          $0/month   (limited prompts per day)
  Pro           $20/month  (500 prompts/month, premium AI models included)
  Max           $200/month (unlimited prompts, currently free while in beta; premium AI models included)
```

**Key difference for Pro/Max:** ForgeCode Pro and Max include premium AI models (no separate
API key required for those tiers). The Free tier requires users to bring their own API keys
for advanced models.

**Cost analysis:**
- ForgeCode Pro ($20/mo) is comparable in price to Claude Code Pro ($20/mo), but Claude Code
  includes unlimited usage while ForgeCode Pro caps at 500 prompts/month
- ForgeCode Max ($200/mo, currently free) vs Claude Code Max ($100/mo) — double the price when
  not in the free beta period
- The included premium model access in Pro/Max simplifies ForgeCode's cost model vs the
  older API-key-only model
- Claude Code's flat-rate model is simpler and more predictable for heavy usage

### Limitations

- Top benchmark score locked behind proprietary ForgeCode Services
- Shell compatibility issues (zsh-vi-mode, fish shell)
- VS Code extension is basic and buggy
- Small team — sustainability risk if company pivots
- No independent head-to-head comparison with rigorous methodology

## 16. Final Assessment and Decision (2026-04-07)

### Verdict: Stay on Claude Code, Monitor ForgeCode

**ForgeCode's harness engineering is technically impressive** — the schema
optimization and verification enforcement represent real improvements that
SoTOHE could learn from. However, adoption is blocked by:

1. **Cost**: Subscription plans exist (Free/Pro/Max) but Pro caps at 500 prompts/month
   and Max costs $200/month — double Claude Code Max ($100/month) once the free beta ends.
   Heavy Opus usage makes ForgeCode more expensive than Claude Code Max.
2. **Maturity**: Small community, inconsistent large-codebase handling,
   limited independent validation.
3. **Hook migration gap**: SoTOHE's Claude Code hooks cover Bash/Write tool
   calls broadly, not just git operations. `.git/hooks/` can only replace
   the git-operation subset.
4. **Benchmark narrowness**: TermBench 2.0 leadership reflects terminal-harness
   optimization, not general superiority for SoTOHE's workflow.

### What to Adopt from ForgeCode (Without Switching)

The 4 harness engineering techniques can be applied within SoTOHE's existing
Claude Code setup:

1. Schema field ordering → review sotp MCP tool schemas (if ever built)
2. Explicit truncation markers → improve `sotp` CLI output formatting
3. Enforced verification → already implemented (`review_check_approved`)
4. Schema flattening → applicable to `spec.json` / `metadata.json` schemas

### Revisit Conditions

- ForgeCode adds Claude subscription support (eliminates cost blocker)
- ForgeCode Services becomes open-source (eliminates benchmark reproducibility gap)
- Independent SWE-bench results published showing significant advantage
- SoTOHE needs multi-provider flexibility that Claude Code cannot provide

ForgeCode = "A tool to make AI easy to use"
SoTOHE = "A process to make AI output trustworthy"

These are complementary layers. Current decision: stay on Claude Code (cost + maturity),
adopt ForgeCode's harness techniques selectively, revisit when subscription support arrives.
