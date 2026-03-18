<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Bash file-write guard via existing guard extension (CON-07)

Close CON-07 pragmatically: permissions.deny for command-name patterns, extend existing block-direct-git-ops guard to detect output redirects via conch-parser AST, and document accepted residual risks

## Permissions Deny Layer

Add permissions.deny entries for file-write commands and matching FORBIDDEN_ALLOW entries. Simple, fast first defense.

- [x] Add permissions.deny entries for file-write commands (touch, cp, mv, install, chmod, chown) and corresponding FORBIDDEN_ALLOW entries in orchestra.rs

## Extend Existing Guard (output redirects + write commands)

Extend block-direct-git-ops guard's existing conch-parser CommandVisitor to also flag output redirect AST nodes (Write/Append/Clobber kinds) and tee/sed-i command names. Reuses all existing infrastructure: AST parsing, recursive bash -c inspection, fail-closed on parse error, selftest framework.

- [x] Extend existing block-direct-git-ops guard policy to also detect output redirect AST nodes and tee/sed-i command names, reusing the existing conch-parser CommandVisitor infrastructure

## Residual Risk Documentation

Document accepted risks that are impractical to guard against (FD duplication like 2>&1, exotic shell re-entry, named pipes, process substitution) in a convention doc.

- [x] Document accepted residual risks (FD duplication, exotic shell re-entry, named pipes, process substitution, /proc writes) in project-docs/conventions/bash-write-guard.md
