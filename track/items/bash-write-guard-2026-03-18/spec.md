---
status: draft
version: "2.0"
---

# Spec: Bash File-Write Guard via Existing Guard Extension (CON-07)

## Goal

Close **CON-07** pragmatically: Bash tool file writes bypass file-lock hooks. Instead of building a comprehensive shell command analyzer (which is equivalent to reimplementing a security sandbox), layer three defenses:

1. **permissions.deny** — command-name patterns (fast, simple)
2. **Existing guard extension** — extend `block-direct-git-ops` to also detect output redirects and write commands via the existing conch-parser AST infrastructure
3. **Documentation** — accepted residual risks that are impractical to guard against

## Design Rationale

The existing `block-direct-git-ops` guard already has:
- conch-parser AST parsing with `CommandVisitor` trait
- Recursive `bash -c` / `sh -c` payload inspection (generalizable to any `-c` shell)
- Fail-closed on parse errors
- Selftest framework with allow/block cases

Extending this guard's `split_shell` / `SimpleCommand` pipeline is strictly better than building a parallel one: zero code duplication, proven infrastructure, existing test patterns. Note: the current guard uses flattened `SimpleCommand` values — there is no `CommandVisitor` trait to extend; the work is adding redirect-kind fields to `SimpleCommand` and adding check logic in `policy.rs`.

Complete file-write detection is an unsolvable problem (FD duplication, named pipes, process substitution, `/proc/self/fd/` writes). The pragmatic approach is to block the common patterns and document the rest.

## Scope

- `.claude/settings.json` — permissions.deny entries
- `libs/infrastructure/src/verify/orchestra.rs` — FORBIDDEN_ALLOW updates
- `libs/domain/src/guard/parser.rs` — extend `SimpleCommand` to expose redirect kinds (Write/Append/Clobber/Read/DupWrite) alongside existing `redirect_texts` (additive, not replacing)
- `libs/domain/src/guard/policy.rs` — extend existing guard's CommandVisitor to check redirect kinds and write-command names
- `project-docs/conventions/bash-write-guard.md` — residual risk documentation

## Constraints

- No new crate dependencies
- No new hook dispatch variant — reuse existing `block-direct-git-ops` hook
- Fail-closed on parse errors (existing behavior preserved)
- Must not break existing allowed Bash commands (`cargo make *`, `git read commands`, `head/tail/wc`)
- FD duplication (`2>&1`) must NOT be blocked (it's not a file write)

## Acceptance Criteria

- [ ] permissions.deny includes: `Bash(touch :*)`, `Bash(cp :*)`, `Bash(mv :*)`, `Bash(install :*)`, `Bash(chmod :*)`, `Bash(chown :*)`
- [ ] FORBIDDEN_ALLOW includes matching entries
- [ ] Existing guard extended to detect output redirect AST nodes (Write/Append/Clobber kinds — NOT DupWrite/Read)
- [ ] Existing guard extended to detect `tee` and `sed` with `-i` flag as command names in AST
- [ ] Recursive `-c` shell inspection uses an explicit allowlist of known shells (`bash`, `sh`, `dash`, `zsh`, `ash`) rather than suffix matching (avoids false positives like `ssh -c`)
- [ ] FD duplication (`2>&1`, `1>&2`) is NOT blocked
- [ ] Input redirects (`< file`) are NOT blocked
- [ ] Existing `block-direct-git-ops` selftests still pass
- [ ] New selftests added for redirect/write-command cases (both allow and block)
- [ ] Accepted residual risks documented in `project-docs/conventions/bash-write-guard.md`
- [ ] `cargo make ci` passes
