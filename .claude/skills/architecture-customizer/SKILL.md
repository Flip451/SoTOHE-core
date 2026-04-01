---
name: architecture-customizer
description: Customize or migrate the Rust workspace architecture (layer names, crate boundaries, dependency direction) safely. Use when users want to change layered architecture structure, rename/move crates, switch architecture style, or update enforcement rules across architecture-rules.json, Cargo.toml, deny.toml, scripts/check_layers.py, Makefile tasks, and track docs.
---

# /architecture-customizer — Workspace Architecture Migration Workflow

Use this workflow when architecture changes are requested.

## Workflow

1. Clarify target architecture in Japanese with concrete crate map.
2. Translate target architecture into explicit dependency rules and deny reasons.
3. Update enforcement first, then code layout, then docs.
4. Run architecture checks and stop if any gate fails.

## Step 1: Define Target Crate Map

Write the target map before any edits using workspace member paths:

```text
<root-a>/<crate-a>
<root-a>/<crate-b>
<root-b>/<crate-c>
...
```

Default template examples often use `apps/<entry>` and `libs/<layer>`, but other roots are allowed if `architecture-rules.json`, enforcement, and docs are updated together.

Define which crates may depend on which crates.

## Step 2: Update Enforcement Rules

1. Update workspace members in `Cargo.toml`.
2. Update `architecture-rules.json` first.
3. Update layer policy in `deny.toml` (`deny = [...]` wrappers).
4. Update `scripts/check_layers.py` crate names and forbidden edges.
5. Update `Makefile.toml` task names if any crates were renamed (e.g., `check-layers-local` already references crate names via `scripts/check_layers.py`).

## Step 3: Update Crates

1. Create/move/rename crate directories.
2. Update each crate `Cargo.toml` dependency edges.
3. Ensure composition root crate wires dependencies.

## Step 4: Update Documentation

1. Update `track/tech-stack.md` workspace structure and rule text.
2. Update `track/workflow.md` quality gates and layer check steps.
3. Update `track/code_styleguides/rust.md` module layout example.
4. Update `CLAUDE.md` file tree if crate map changed.

## Step 5: Validation Gates

Run in this order:

```bash
python3 -m py_compile scripts/check_layers.py
cargo fmt --all -- --check
cargo make check-layers
cargo make verify-arch-docs
cargo deny check -D warnings
```

If any command fails, fix architecture rules before implementation work.

## Output Contract

Report with:

1. New crate map
2. Enforced dependency rules
3. Files changed
4. Validation results
