# Bash File-Write Guard

## Overview

Bash tool file writes used to be guarded because they bypassed the file-lock hooks
(`file-lock-acquire`/`file-lock-release`), which only triggered on `Read|Edit|Write` tool
calls. Those file-lock hooks have since been removed, so the original protection target for the
AST-level file-write guard no longer exists. The retired Layer-2 blocks therefore do not protect a
current enforcement boundary.

This reference now documents the remaining Bash write guardrails and the accepted residual risks.

## Layered Defense

### Layer 1: `permissions.deny` (fastest — Claude Code blocks before hook execution)

Which commands are denied is stated in `.claude/settings.json` and nowhere else. That file is
the consumer's own (`.harness/policies/consumer-ownership.md`): SoTOHE ships a recommended default posture
and no CI gate enforces it, so a list repeated here would describe one repository's choices as
if they were the guard's contract, and would go stale the first time either side moved.
Read the file.

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

## Accepted Residual Risks

The following file-write vectors are accepted after Layer-2 retirement:

| Vector | Reason not blocked |
|--------|-------------------|
| General Bash writes via redirects, `tee`, or `sed -i` | The file-lock hooks they originally protected were removed, so this guard does not attempt to sandbox arbitrary file writes from Bash |
| Shell re-entry (`bash -c`, `sh -c`, heredocs, scripts) | Git writes are enforced at process level by git hooks, while non-git file writes are handled by normal review and CI |
| Named pipes (`mkfifo`) | Rarely used in Claude Code Bash calls; `mkfifo` is not in `permissions.allow` |
| `/proc/self/fd/N` writes | Exotic; not practical to detect without filesystem-level sandboxing |
| `dd of=file` | Not in `permissions.allow`; rare in template workflows |
| `cargo make` internal writes | Intentionally allowed — task execution is reviewed and gated rather than treated as a shell sandbox |

## Design Decision

This guard no longer treats Bash file writes as the primary enforcement surface. The old
command-string scan era tried to infer dangerous behavior from shell syntax and accumulated
blanket blocks for redirects, `tee`, and `sed -i`. Direct git-write enforcement lives at the git
process boundary through `.githooks/reference-transaction` and `.githooks/pre-push`.

The remaining Bash-side controls are intentionally narrow: whatever `permissions.deny` the
consumer has configured, and `block-test-file-deletion`, which protects test files from
redirect-based truncation. Broader file-write safety is handled by review, CI, and the normal
track workflow rather than by reimplementing a shell sandbox.
