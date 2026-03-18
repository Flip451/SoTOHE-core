# Verification — Bash File-Write Guard (CON-07)

## Scope Verified

- [ ] T001: Permissions deny + FORBIDDEN_ALLOW
- [ ] T002: Extend existing guard for output redirects + write commands
- [ ] T003: Residual risk documentation

## Manual Verification Steps

### T001: Permissions deny layer

1. permissions.deny includes `Bash(touch :*)`, `Bash(cp :*)`, `Bash(mv :*)`, `Bash(install :*)`, `Bash(chmod :*)`, `Bash(chown :*)`
2. FORBIDDEN_ALLOW includes matching entries
3. `cargo make ci` passes

### T002: Guard extension

**Output redirect detection (AST-level):**
4. `echo foo > file.txt` is blocked (Write redirect)
5. `echo foo >> file.txt` is blocked (Append redirect)
6. `cmd 2> err.log` is blocked (stderr Write redirect)
7. `cmd >| force.txt` is blocked (Clobber redirect)

**FD duplication NOT blocked:**
8. `cmd 2>&1` is allowed (DupWrite — not a file write)
9. `cmd 1>&2` is allowed

**Input redirect NOT blocked:**
10. `cmd < input.txt` is allowed

**Write command detection:**
11. `tee output.txt` is blocked
12. `cmd | tee output.txt` is blocked
13. `sed -i 's/a/b/' file` is blocked

**Recursive shell inspection:**
14. `bash -c 'echo > file'` is blocked
15. `sh -c 'tee out'` is blocked
16. `dash -c 'cp a b'` is blocked (shell allowlist)
17. `zsh -c 'echo > f'` is blocked (shell allowlist)
18. `ash -c 'tee f'` is blocked (shell allowlist)

**Normal commands allowed:**
16. `cargo make test` is allowed
17. `git status` is allowed
18. `head file.txt` is allowed

**Existing selftests:**
19. All existing `block-direct-git-ops` selftests still pass
20. New selftests include both allow and block cases for redirects/write-commands

### T003: Documentation

21. `project-docs/conventions/bash-write-guard.md` exists
22. Documents accepted residual risks (FD duplication, exotic shell re-entry beyond allowlist, named pipes, process substitution, /proc writes)
23. Documents the layered defense architecture

## Result

- (pending)

## Verified At

- (pending)
