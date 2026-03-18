# Bash File-Write Guard (CON-07)

## Overview

Bash tool file writes bypass the file-lock hooks (`file-lock-acquire`/`file-lock-release`)
because those hooks only trigger on `Read|Edit|Write` tool calls. This convention documents
the layered defense and accepted residual risks.

## Layered Defense

### Layer 1: `permissions.deny` (fastest — Claude Code blocks before hook execution)

Commands denied: `touch`, `cp`, `mv`, `install`, `chmod`, `chown`.

These commands are also in `FORBIDDEN_ALLOW` to prevent future addition to `permissions.allow`.

### Layer 2: `block-direct-git-ops` guard extension (AST-level)

The existing guard policy (`libs/domain/src/guard/policy.rs`) is extended to also block:

- **Output/writable redirects**: `>`, `>>`, `>|`, `<>`, `N>`, `N>>` — detected via conch-parser AST
  redirect kinds (Write/Append/Clobber/ReadWrite). Does NOT block FD duplication (`>&`/`2>&1`).
- **`tee` command**: both standalone and in pipelines.
- **`sed -i`**: in-place file editing (detected via `-i` flag parsing).

The guard is fail-closed: unparseable commands (including Bash-only syntax like `&>`) are blocked.

### Layer 3: `FORBIDDEN_ALLOW` (CI enforcement)

Prevents file-write command patterns from being added to `permissions.allow` in settings.json.

## Accepted Residual Risks

The following file-write vectors are **not** blocked and are accepted as residual risk:

| Vector | Reason not blocked |
|--------|-------------------|
| FD duplication (`2>&1`, `1>&2`) | Not a file write — redirects between file descriptors |
| Exotic shell re-entry (`bash -c`, `sh -c`, `dash -c`, etc.) | The existing git guard catches `-c` payloads via argv substring matching for "git". Redirect/tee/sed-i detection requires AST-level parsing of the payload, which is not yet implemented. Planned for a future parser enhancement track |
| Named pipes (`mkfifo`) | Rarely used in Claude Code Bash calls; `mkfifo` is not in `permissions.allow` |
| Process substitution (`>(cmd)`, `<(cmd)`) | Bash-only syntax triggers POSIX parse error → fail-closed block |
| `/proc/self/fd/N` writes | Exotic; not practical to detect without filesystem-level sandboxing |
| Shell `-c` payload writes (`bash -c 'echo > f'`) | The current parser does not recursively re-parse `-c` payloads for redirect/tee/sed-i detection. The existing git guard uses substring matching on argv, but redirect detection requires AST-level parsing of the payload string. Planned for a future parser enhancement track |
| Heredoc body writes (`bash <<'SH'\necho > f\nSH`) | Same root cause as `-c` payloads: heredoc bodies are stored as flattened text in `redirect_texts` for git-substring matching, but not re-parsed at AST level for redirect/tee detection. Same future parser track |
| `sed -i.bak` (attached suffix) / `sed -ni` (combined flags) | Detecting `-i` inside combined flags risks false positives (`sed -finit.sed`). Only standalone `-i`, `-i=suffix`, `--in-place`, `--in-place=suffix` are caught |
| `dd of=file` | Not in `permissions.allow`; rare in template workflows |
| `cargo make` internal writes | Intentionally allowed — cargo make tasks run in Docker containers with their own isolation |

## Design Decision

Complete file-write detection is equivalent to reimplementing a security sandbox. The pragmatic
approach is to block common patterns (redirects, tee, sed -i, and command-name deny rules) and
accept the remaining edge cases as documented risks. Docker containerization of `cargo make`
tasks provides the final safety net.
