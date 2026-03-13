---
name: codex-system
description: |
  Use this skill whenever the user needs deep Rust reasoning — whether asked casually or
  formally, in Japanese or English. Trigger for: Rust compiler errors (E0382, E0505, E0507
  etc. or "コンパイル通らない"), ownership/borrowing/lifetime questions ("moved value", "所有権",
  clone vs borrow), trait design or review ("トレイト設計", Repository pattern, method signatures,
  return types), Rust architecture planning (domain layer, usecase layer, hexagonal/DDD,
  Command/Query patterns), and implementation planning for Rust features ("実装計画", "設計したい").
  Also trigger when the active profile assigns planner/reviewer/debugger/implementer roles
  to Codex. Do NOT trigger for simple Cargo.toml edits, cargo fmt/clippy fixes, test assertion
  updates, dependency version lookups, or non-Rust tasks.
metadata:
  short-description: Consult Codex for Rust design & complex tasks
---

# /codex-system — Codex CLI Consultation Skill

Skill for using Codex CLI as a specialist provider.

Check `.claude/agent-profiles.json` first. Use this skill only when the target capability is assigned to Codex.

## Configuration

Before invoking Codex, read `.claude/agent-profiles.json` to resolve the model:

```
profiles.<active_profile>.provider_model_overrides.codex  →  {model}
fallback: providers.codex.default_model  →  {model}
```

All templates below use `{model}` as a placeholder. Replace it with the actual value from `agent-profiles.json`.

### Reasoning Effort

Append `--config model_reasoning_effort="{effort}"` to control reasoning depth:

```bash
timeout 180 codex exec --model {model} --config model_reasoning_effort="high" \
  --sandbox read-only --full-auto "{task}" 2>&1
```

Values: `low`, `medium`, `high`. Default varies by model. Use `high` for complex design/review tasks.

## When to Use

- Rust trait/architecture design
- Ownership and lifetime planning
- Compiler error diagnosis (E-codes)
- Implementation planning (TDD-friendly)
- Complex Rust code review
- When `planner` / `reviewer` / `debugger` / `implementer` capability is assigned to Codex

## Usage Patterns

### Architecture Design

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Design Rust architecture for: {feature description}

Current context:
- Architecture pattern: Hexagonal (Ports & Adapters)
- Async runtime: Tokio
- Error handling: thiserror

Provide:
1. Trait definitions (ports) with method signatures
2. Adapter structure (infra layer)
3. Ownership model (Arc<dyn Trait>, owned types)
4. Error type hierarchy
5. Module layout
" 2>&1
```

### Ownership/Lifetime Design

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Design the ownership model for:

{struct or function description}

Context: {how it's used, what it holds}

Provide:
1. Recommended ownership approach (owned vs borrowed vs Arc/Rc)
2. Lifetime annotations if needed
3. Alternative designs with trade-offs
4. Potential pitfalls
" 2>&1
```

### Compiler Error Diagnosis

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Diagnose this Rust compiler error:

Error code: {E0XXX}
Full error:
{paste full error message}

Relevant code:
{paste code snippet}

Analyze root cause (ownership/borrow/lifetime/trait bounds)
and suggest a fix that preserves the intended semantics.
" 2>&1
```

### Implementation Planning

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Create a TDD implementation plan for: {feature}

Requirements: {list of requirements}
Constraints: {crates to use, architecture patterns}

Output:
1. Step-by-step plan (domain types → traits → tests → impl → integration)
2. Key design decisions with rationale
3. Potential challenges and mitigations
" 2>&1
```

### Code Review (Hybrid Prompt Pattern)

Embed design intent and change summary directly in the prompt; let Codex read files for details.
This avoids Codex spending time searching the codebase while keeping the prompt concise.
Use `timeout 180` to prevent indefinite hangs (Codex file exploration can exceed 120s).

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Review {feature name}. Report ONLY bugs or logic errors. Be concise.

## Design
{2-5 bullet points explaining the design intent and invariants}

## Changed files
{list of changed file paths}

## Key code (optional — include only if ~1KB or less)
{short code snippet of the core logic change}

Check for: logic errors, doc-code inconsistencies, edge cases, security issues.
" 2>&1
```

For larger diffs, use the file-based briefing pattern (see below) instead of inlining.

## File-Based Briefing Pattern

Prefer writing content to a file over inline embedding when:

| Situation | Reason |
|-----------|--------|
| Diff or error log is long | Shell escaping (quotes, special chars) can break the command |
| Passing full `spec.md` / `plan.md` | Inline embedding risks truncation errors |
| Combining content from multiple files | Heredocs become unwieldy |

`read-only` sandbox can still read existing files — the file-based approach works without relaxing sandbox constraints.

### Steps

1. **Write the briefing file** (Claude Code uses the Write tool)

   ```
   tmp/codex-briefing.md
   ```

2. **Run Codex with a file reference**

   ```bash
   timeout 180 codex exec --model {model} --sandbox read-only --full-auto \
     "Read tmp/codex-briefing.md and perform the task described there." 2>&1
   ```

3. **Delete the briefing file** (Claude Code uses Bash tool)

   ```bash
   rm tmp/codex-briefing.md
   ```

### Briefing File Template

```markdown
# Task

{task type: Architecture Design / Code Review / Compiler Error Diagnosis / etc.}

## Context

{feature name, relevant spec excerpt, architecture constraints}

## Input

{diff, error message, code snippet — verbatim}

## Output Required

{numbered list of what Codex should return}
```

Short queries (1–2 paragraphs) are fine as inline prompts.
Use this file-based pattern when content is long or contains special characters.

## Output Format

Codex will return structured output. Extract and relay to user in Japanese:

```markdown
## Analysis
{Deep analysis of the Rust problem}

## Recommendation
{Clear, actionable recommendation with code if needed}

## Implementation Plan (if applicable)
1. {Step 1}
2. {Step 2}

## Rust Code Example
\`\`\`rust
{concrete Rust code}
\`\`\`

## Risks
{Potential issues to watch}
```

## Iterative Review Loop

For review-fix-review cycles ("修正→レビュー→修正→…→指摘なし"):

1. **First round**: Use the hybrid prompt pattern — embed design intent and change summary, list changed files, let Codex read them
2. **Fix findings**: Add tests for each bug, implement fixes
3. **Subsequent rounds**: State what was fixed, ask Codex to verify the fix and check for remaining issues
4. Stop when Codex reports no bugs

### Round 1 template (hybrid prompt)

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Review {feature}. Report ONLY bugs or logic errors.

## Design
{design invariants}

## Changed files
{file list}

Check for: {checklist}
" 2>&1
```

### Round N template (fix verification)

```bash
timeout 180 codex exec --model {model} --sandbox read-only --full-auto "
Previous review found: {finding summary}. Fixed by {fix description}. Test added.
Verify the fix in {file}:{line range}. Any remaining bugs?
" 2>&1
```

### Tips

- Always use `timeout 180` — Codex file exploration can exceed 120s
- Prefer `2>&1` over `2>/dev/null` to capture diagnostics
- Instruct Codex "do not run Python code" if you want text-only analysis (faster)
- Codex may run code in `read-only` sandbox to verify edge cases — this is fine and often produces better findings
- Each round's prompt should reference only the delta, not the full history
- Keep inline prompts under ~1KB; for larger context, use the file-based briefing pattern

## Execution Tips

- **Model flag**: resolve `profiles.<active_profile>.provider_model_overrides.codex` first, then fall back to `providers.codex.default_model`, and pass the result as `--model {model}`
- **Foreground preferred**: `2>&1` captures both stdout and stderr reliably; `2>/dev/null` may hide useful diagnostics
- **Long prompts**: always use the file-based briefing pattern — inline prompts over ~1KB risk shell escaping issues
- **Short prompts**: inline is fine for 1–2 paragraph queries

## Notes

- Always ask in English for best results
- Pass full error messages including error codes
- Include relevant code context (not just the error line)
- Save important design decisions to `.claude/docs/DESIGN.md`
