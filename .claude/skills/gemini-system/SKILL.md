---
name: gemini-system
description: |
  Use Gemini CLI when the active profile assigns researcher/multimodal_reader
  capabilities to Gemini. Invoke this skill whenever the task involves crate research,
  codebase-wide analysis, dependency auditing, Rust ecosystem surveys, or reading
  multimodal files (PDF, image, video, audio) — even if the user doesn't explicitly
  mention Gemini. Gemini's 1M context window and Google Search grounding make it
  the right choice for tasks that require broad context or up-to-date external information.
metadata:
  short-description: Use Gemini for Rust research & codebase analysis
---

# /gemini-system — Gemini CLI Research Skill

Skill for using Gemini CLI as a specialist provider for research and multimodal tasks.

Check `.claude/agent-profiles.json` first. Use this skill only when the target capability is assigned to Gemini.

## When to Use

- Rust codebase analysis (1M context advantage)
- Crate research (latest version, features, best practices)
- Multimodal file reading (PDF, video, audio, image)
- Large-scale dependency analysis
- Rust ecosystem surveys
- When `researcher` / `multimodal_reader` capability is assigned to Gemini

## Usage Patterns

### Codebase Analysis

```bash
gemini -p "Analyze this Rust codebase comprehensively:
- Cargo workspace structure and crate boundaries
- Domain model: key types, value objects, aggregates
- Port definitions (traits in domain layer)
- Adapter implementations (infra layer)
- Async patterns and Tokio usage
- Error handling strategy
- Test organization: unit, integration, mocks
- Key dependencies and their roles" 2>/dev/null
```

### Crate Research

```bash
# Single crate deep dive
gemini -p "Research Rust crate: {name}.
- Current stable version and recent changelog
- Core API: key traits, types, functions
- Async support: tokio compatibility, Send+Sync bounds
- Feature flags and their impact
- Known limitations or footguns
- Migration from older versions
- Comparison with alternatives: {alt1}, {alt2}
- Real-world usage patterns from popular OSS projects
Include docs.rs and crates.io links." 2>/dev/null

# Multiple crates comparison
gemini -p "Compare Rust solutions for {problem}: {A} vs {B} vs {C}.
API ergonomics, performance, async support, community support,
maintenance status, license." 2>/dev/null
```

### Rust Pattern Research

```bash
gemini -p "Research Rust design patterns for: {pattern name}.
- Canonical implementation
- When to use vs alternatives
- Async considerations
- Examples from well-known Rust crates (tokio, axum, etc.)
- Performance implications" 2>/dev/null
```

### Dependency Audit Research

```bash
gemini -p "Research security and compatibility of Rust crates:
{crate list with versions}
- Known CVEs or security advisories
- License compatibility (MIT/Apache-2.0 preferred)
- Maintenance status (active/abandoned)
- Alternative recommendations if problematic" 2>/dev/null
```

### Multimodal File Reading

```bash
# PDF — use path-in-prompt format
gemini -p "Extract the key technical specifications from /path/to/spec.pdf" 2>/dev/null

# Image (architecture diagram)
gemini -p "Describe the architecture shown in /path/to/diagram.png" 2>/dev/null
```

## File-Based Briefing Pattern

Prefer writing content to a file over inline embedding when:

| Situation | Reason |
|-----------|--------|
| Passing full `spec.md` / `plan.md` | Inline embedding risks truncation errors |
| Combining content from multiple files | Prompt string becomes unwieldy |
| Content contains shell special chars (`$`, backticks, etc.) | Escaping can break the command |

Gemini accepts text content via stdin for briefing files.
For multimodal files (PDF, image, etc.), use path-in-prompt instead of stdin redirect.

### Steps

1. **Write the briefing file** (Claude Code uses the Write tool)

   ```
   tmp/gemini-briefing.md
   ```

2. **Pipe to Gemini via stdin** — keep the prompt as instructions only, pass content via stdin

   ```bash
   gemini -p "Read the task description below and perform it." \
     < tmp/gemini-briefing.md 2>/dev/null
   ```

3. **Save results** before deleting the briefing file

   ```bash
   # Save to .claude/docs/research/{topic}.md via Write tool
   ```

4. **Delete the briefing file** (Claude Code uses Bash tool)

   ```bash
   rm tmp/gemini-briefing.md
   ```

### Briefing File Template

```markdown
# Task

{task type: Codebase Analysis / Crate Research / Dependency Audit / etc.}

## Context

{feature name, relevant background, what decision this research informs}

## Input

{crate names, file paths, version constraints, or other relevant data}

## Output Required

{numbered list of what Gemini should return}
```

Short queries (1–2 paragraphs) are fine as inline `-p` prompts.
Use this file-based pattern when content is long or contains special characters.

## Output Management

Always save research results before discarding the briefing file:

```
.claude/docs/research/{topic}.md       # research results
.claude/docs/libraries/{crate-name}.md # crate-specific notes
```

## Execution Tips

- **Foreground preferred**: `2>&1` captures both stdout and stderr reliably; `2>/dev/null` may hide useful diagnostics
- **Long prompts**: always use the file-based briefing pattern via stdin — inline `-p` prompts over ~1KB risk shell escaping issues
- **Short prompts**: inline `-p` is fine for 1–2 paragraph queries

## Notes

- Ask in English for best results
- Use `-p` flag for prompts (not interactive mode)
- `2>&1` is safer than `2>/dev/null` for debugging; use the latter only in production hooks
- For very large codebases, specify which directories to focus on
- Summarize results to 5–10 bullet points before reporting to user
