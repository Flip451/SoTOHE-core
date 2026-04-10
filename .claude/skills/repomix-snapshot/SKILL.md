---
name: repomix-snapshot
description: |
  Pack the entire repository into a single file with repomix and feed it to
  the researcher capability (Gemini) for whole-codebase analysis.
  Use when full project understanding is needed in one pass.
metadata:
  short-description: Codebase snapshot for Gemini researcher analysis
---

# /repomix-snapshot — Codebase Snapshot for Researcher

**Pack the repository with repomix and pipe it to the researcher capability (Gemini) for full-codebase analysis.**

Check `.harness/config/agent-profiles.json` first. Use this skill only when `researcher` capability is assigned to Gemini.

## When to Use

| Situation | Reason |
|-----------|--------|
| Full project structure understanding required | One pass covers everything vs. multiple Read calls |
| `track-plan` Phase 1 codebase analysis | Understand the whole before writing spec.md |
| Pre-refactoring survey | Dependency and naming patterns in one shot |
| Domain model discovery before new feature design | Grasp all existing types and traits at once |

Use regular `Read` / `Grep` when only specific modules need inspection.

## Usage

### Step 1: Generate snapshot

```bash
repomix --output tmp/repomix-snapshot.md --style markdown 2>/dev/null
```

Options:
- `--output tmp/repomix-snapshot.md` — `tmp/` is gitignored; no commit pollution
- `--style markdown` — readable format for Gemini
- `--compress` — add when token count is too large (large repos)

Filtering when needed:

```bash
# Target specific directories only
repomix --include "src/**,libs/**,apps/**" \
        --output tmp/repomix-snapshot.md --style markdown 2>/dev/null

# Exclude noisy files (.gitignore is respected by default)
repomix --ignore "*.lock,*.toml" \
        --output tmp/repomix-snapshot.md --style markdown 2>/dev/null
```

### Step 2: Pipe to Gemini via stdin

```bash
gemini -p "Analyze this Rust codebase comprehensively:
- Cargo workspace structure and crate boundaries
- Domain model: key types, value objects, aggregates
- Port definitions (traits in domain layer)
- Adapter implementations (infra layer)
- Async patterns and Tokio usage
- Error handling strategy
- Test organization: unit, integration, mocks
- Key dependencies and their roles

Return 7-10 key findings as bullet points." \
  < tmp/repomix-snapshot.md 2>/dev/null
```

### Step 3: Save results

Save analysis output before deleting the snapshot:

```bash
# Claude Code saves via Write tool to:
# knowledge/research/{feature}-codebase.md
```

### Step 4: Delete snapshot

```bash
rm tmp/repomix-snapshot.md
```

## Full Example (track-plan Phase 1)

```bash
# 1. Generate snapshot
repomix --output tmp/repomix-snapshot.md --style markdown 2>/dev/null

# 2. Analyze with Gemini
gemini -p "Analyze this Rust codebase for feature planning:
- Cargo workspace: member crates and their roles
- Existing domain types, value objects, aggregates
- Traits (ports) defined in domain layer
- Infrastructure adapters and their implementations
- Error type hierarchy
- Test patterns (unit, integration, mock strategy)
- Naming conventions and module layout patterns
- Crates already in use (from Cargo.toml entries)

Summarize in 10 concise bullet points to inform implementation planning." \
  < tmp/repomix-snapshot.md 2>/dev/null

# 3. Delete snapshot
rm tmp/repomix-snapshot.md
```

## Notes

- `tmp/` is gitignored — snapshot never pollutes commits
- repomix respects `.gitignore` by default (`target/`, `.cache/` are excluded)
- Add `--compress` or narrow with `--include` if output is too large
- Always save analysis results to `knowledge/research/{topic}.md` before deleting the snapshot
