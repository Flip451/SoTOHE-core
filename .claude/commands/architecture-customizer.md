---
description: Customize or migrate the workspace architecture safely.
---

Canonical wrapper for workspace architecture changes in this template.

Execution:
- Clarify the target architecture in Japanese with a concrete crate map before any edits.
- Translate the target architecture into explicit dependency rules and denial reasons.
- Update `architecture-rules.json` first, then enforcement, then crate layout, then documentation.
- Run the architecture validation gates in order and stop if any gate fails.
- The default template uses `apps/` and `libs/`, but other workspace roots are allowed if enforcement and docs stay in sync.

Required workflow:
1. Define the target crate map as workspace member paths.
2. Define which crates may depend on which crates.
3. Update architecture enforcement:
   - `Cargo.toml`
   - `architecture-rules.json`
   - `deny.toml`
   - `scripts/check_layers.py`
   - `Makefile.toml`
4. Update crates and dependency edges.
5. Update architecture-facing documents:
   - `track/tech-stack.md`
   - `track/workflow.md`
   - `CLAUDE.md` when the file tree changes
6. Run:
   - `python3 -m py_compile scripts/check_layers.py`
   - `cargo fmt --all -- --check`
   - `cargo make check-layers`
   - `cargo make verify-arch-docs`
   - `cargo deny check -D warnings`

Behavior:
- Treat this command as the single entry point for layered architecture migration.
- Do not edit implementation crates before enforcement rules are updated.
- Do not leave architecture docs behind the actual crate map.

Output format:
1. New crate map
2. Enforced dependency rules
3. Files changed
4. Validation results
