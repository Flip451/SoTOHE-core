# Development Workflow

Detailed development workflow for this project.

- `track`: context management layer — `spec.md` / `plan.md` / `verification.md` / progress tracking
- `Claude Code + Agent Teams + Rust CLI`: execution layer for implementation, review, and guarded git/PR workflow

## Spec-Driven Development Cycle

```
1. Initialization (track workflow)
   └── /track:setup                         # One-time project initialization

2. Planning phase (Claude Code Orchestra)
   ├── /track:plan <feature>                # Standard lane: plan + branch creation
   │   ├── Phase 1: version baseline research via active `researcher` capability
   │   ├── Phase 1.5: codebase analysis via active `researcher` capability + spec.md
   │   ├── Phase 2: resolve track/tech-stack.md `TODO:` via user dialogue
   │   ├── Phase 2.5: Agent Teams (Researcher ↔ Architect)
   │   ├── Phase 3: plan creation → user approval
   │   └── Phase 4: create track artifacts (metadata.json, plan.md, spec.md, verification.md, registry.md)
   │
   └── Alternative (plan-only, separate review branch):
       └── /track:plan-only <feature>       # Create plan/<id> branch + planning artifacts (branch=null in metadata)
           → Push plan/<id> branch + PR     # Review planning artifacts via PR
           → PR merge to main               # Planning artifacts land on main
           → /track:activate <track-id>     # Create track/<id> implementation branch from main

Architecture-focused changes:
   └── /architecture-customizer             # crate map, dependency direction, enforcement rules

Version Baseline (Phase 1) required steps:
- Create `.claude/docs/research/version-baseline-YYYY-MM-DD.md`
- Review `Cargo.toml` `rust-version`
- Review `Dockerfile` `RUST_VERSION` and tool version ARGs
- Update `track/tech-stack.md` MSRV and changelog

3. Implementation phase (choose one)
   ├── /track:full-cycle <task>              # Autonomous full cycle inside Claude Code
   ├── /track:implement                      # Parallel implementation via Agent Teams
   └── Manual implementation (TDD cycle)

Note:
- When long-form external guides (DB, security, infrastructure) are needed,
  check `knowledge/external/guides.json` and `knowledge/external/POLICY.md` first,
  and only read `.cache/external-guides/` raw content when necessary

4. Review phase (choose one)
   ├── /track:review                         # Review → fix → review cycle (zero findings gate)
   └── PR review workflow (`cargo make track-pr-review`) when a GitHub PR is needed or already exists

5. Completion
   └── /track:status                         # Check progress
   └── /track:commit <message>               # Guarded commit
```

## Autonomous Workflow Details

### `/track:full-cycle <task>`

`/track:full-cycle` is the canonical autonomous implementation path.
It keeps the public `/track:*` interface stable while running the full loop inside Claude Code:

1. Resolve the current track and map `<task>` to approved scope in `metadata.json`
2. Mark the target task `in_progress`
3. Read `spec.md`, rendered `plan.md`, `verification.md`, and required conventions
4. Implement with Agent Teams and focused validation
5. Run the local review loop until findings are zero
6. Run `cargo make ci`
7. Update `verification.md` and mark the task `done`

There is no second autonomous queue/orchestrator to keep in sync.

### `/track:implement`

Use this when the user wants interactive progress updates, narrower task ownership, or staged implementation.
It shares the same SSoT (`metadata.json`) and verification contract as `/track:full-cycle`.

## TDD Cycle (Manual)

When tools-daemon is running (`cargo make tools-up`), exec-based commands run without container startup overhead.
Before entering Agent Teams / `*-exec` fast loops, verify tools-daemon is running.
When adding dependencies or updating `Cargo.lock`, serialize that change to a single worker, then resume parallel implementation.
Parallel workers use `test-one-exec` for single-test checks; full-suite (`test-exec` / `check-exec`) is reserved for integration stages or a single worker (to avoid `target/` build lock contention).

```
1. Red Phase:
   - Write test (tests/ or #[cfg(test)])
   - Single test check (fast):
     cargo make test-one-exec {test_name}
   - Verify it fails (Red confirmation)

2. Green Phase:
   - Write minimal code to pass the test
   - Single test check: cargo make test-one-exec {test_name}
   - Full suite check: cargo make test-exec

3. Refactor Phase:
   - Refactor while keeping tests green
   - cargo make clippy-exec for idiomatic improvements
   - cargo make fmt-exec for formatting
   - cargo make test-exec to verify all tests

4. Commit:
   - `/track:commit <message>` (recommended; handles git note if needed)
   - `cargo make track-add-paths` / `cargo make track-commit-message` / `cargo make track-note` (exact wrappers for agents)
   - cargo make commit (low-level terminal alternative; no auto note)
```

## Rust Quality Gates

Gates executed by `cargo make ci` (prerequisite for `/track:commit` and `cargo make commit`):

```bash
cargo make fmt-check           # Format check
cargo make clippy              # Zero warnings
cargo make test                # All tests pass
cargo make test-doc            # Doctest pass
cargo make deny                # Dependency audit
cargo make check-layers        # Layer dependency check (including transitive)
cargo make verify-arch-docs    # Architecture doc sync check
cargo make verify-plan-progress  # plan.md state validation
cargo make verify-track-metadata # track metadata.json required fields
cargo make verify-track-registry # track registry.md sync check
cargo make verify-tech-stack     # tech-stack.md TODO resolution check
cargo make verify-orchestra      # settings.json hooks/permissions/agent definitions
cargo make verify-latest-track   # Latest track spec.md + plan.md + verification.md completeness
```

Optional (not included in `cargo make ci`):

```bash
cargo make check               # Fast compile check (local pre-check)
cargo make clippy-tests        # Test-only clippy check
cargo make machete             # Unused dependency audit
```

## Track Commands

```bash
/track:setup                  # Project initialization
/track:plan <feature>         # Research, design, plan, and create track artifacts after approval
/track:plan-only <feature>   # Create plan/<id> branch for planning artifacts; PR to main, then activate
/track:activate <track-id>   # Materialize a planning-only track: create branch and switch
/track:full-cycle <task>      # Autonomous full-cycle implementation in Claude Code
/track:implement              # Parallel implementation (interactive)
/track:review                 # Implementation review
/track:revert                 # Safe revert planning
/track:ci                     # Standard CI checks
/track:commit <message>       # Guarded commit
/track:archive <id>           # Archive completed track
/track:status                 # Progress check
/architecture-customizer      # Architecture change entry point
/guide:add                    # Add external guide index entry interactively
/conventions:add <name>       # Add project convention document
```

## Context Management

Specialist capability providers are defined in `.claude/agent-profiles.json`.
Default profile: `planner` / `reviewer` / `debugger` = Codex, `researcher` / `multimodal_reader` = Gemini.

| Context Size | Recommended Approach |
|-------------|---------------------|
| Short question | Direct invocation |
| Design review | Active `reviewer` capability (Codex in default profile) |
| Crate research | Active `researcher` capability (Gemini in default profile) |
| Full codebase analysis | Active `researcher` capability (Gemini 1M context in default profile) |
| Parallel implementation/review | Agent Teams |

## plan.md Task State

| Marker | Meaning |
|--------|---------|
| `[ ]` | Todo |
| `[~]` | In progress |
| `[x]` | Done |
| `[x] abc1234` | Done (with commit hash) |
