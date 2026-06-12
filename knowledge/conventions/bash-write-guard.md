# Bash File-Write Guard (CON-07)

## Overview

Bash tool file writes used to be guarded because they bypassed the file-lock hooks
(`file-lock-acquire`/`file-lock-release`), which only triggered on `Read|Edit|Write` tool
calls. Those file-lock hooks have since been removed, so the original protection target for the
AST-level file-write guard no longer exists. ADR
`2026-06-10-1630-git-hooks-process-level-enforcement` D4 supersedes the retired Layer-2 blocks.

This convention now documents the remaining Bash write guardrails and the accepted residual risks.

## Layered Defense

### Layer 1: `permissions.deny` (fastest — Claude Code blocks before hook execution)

Commands denied: `touch`, `cp`, `mv`, `install`, `chmod`, `chown`.

These commands are also in `FORBIDDEN_ALLOW` to prevent future addition to `permissions.allow`.

### Layer 2: Retired `block-direct-git-ops` file-write blocks

The `block-direct-git-ops` AST-level file-write blocks are retired. The guard no longer blocks:

- output/writable redirects: `>`, `>>`, `>|`, `<>`, `N>`, `N>>`
- `tee`
- `sed -i`

These blocks existed to prevent Bash writes from bypassing file-lock hooks. After the file-lock
hooks were removed, the protection target disappeared and the blocks only added friction to normal
shell usage. Direct git-write enforcement moved from command-string scanning to process-level git
hooks (`reference-transaction` and `pre-push`) with `SOTP_GUARDED_GIT` token checks. The remaining
Claude Code guard keeps precise direct-git checks, `SOTP_GUARDED_GIT` keyword blocking, and
`bin/sotp` overwrite protection.

Test-file truncation is handled separately by `block-test-file-deletion`, which checks output
redirect targets for `tests/` paths.

### Layer 3: `FORBIDDEN_ALLOW` (CI enforcement)

Prevents file-write command patterns from being added to `permissions.allow` in settings.json.

## Accepted Residual Risks

The following file-write vectors are accepted after Layer-2 retirement:

| Vector | Reason not blocked |
|--------|-------------------|
| General Bash writes via redirects, `tee`, or `sed -i` | Allowed by ADR D4. The file-lock hooks they originally protected were removed, so CON-07 no longer attempts to sandbox arbitrary file writes from Bash |
| Shell re-entry (`bash -c`, `sh -c`, heredocs, scripts) | Also allowed by ADR D4. Git writes are enforced at process level by git hooks, while non-git file writes are handled by normal review and CI |
| Named pipes (`mkfifo`) | Rarely used in Claude Code Bash calls; `mkfifo` is not in `permissions.allow` |
| `/proc/self/fd/N` writes | Exotic; not practical to detect without filesystem-level sandboxing |
| `dd of=file` | Not in `permissions.allow`; rare in template workflows |
| `cargo make` internal writes | Intentionally allowed — cargo make tasks run in Docker containers with their own isolation |

## Design Decision

CON-07 no longer treats Bash file writes as the primary enforcement surface. The old
command-string scan era tried to infer dangerous behavior from shell syntax and accumulated
blanket blocks for redirects, `tee`, and `sed -i`. After ADR D4, direct git-write enforcement lives
at the git process boundary through `.githooks/reference-transaction` and `.githooks/pre-push`.

The remaining Bash-side controls are intentionally narrow: `permissions.deny` blocks a small set of
high-risk file commands, `FORBIDDEN_ALLOW` prevents those commands from entering
`permissions.allow`, and `block-test-file-deletion` protects test files from redirect-based
truncation. Broader file-write safety is handled by review, CI, and the normal track workflow rather
than by reimplementing a shell sandbox.
