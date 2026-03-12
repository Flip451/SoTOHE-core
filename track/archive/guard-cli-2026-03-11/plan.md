<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Shell Command Guard CLI

Replace Python hook's fragile regex/shlex command parsing with deterministic Rust-based shell parsing.
Use conch-parser (vendored, patched) for POSIX shell AST parsing in domain layer.
Implement as sotp guard check CLI subcommand that Python hooks delegate to.
All parsing and policy logic in domain layer (pure computation, no I/O).

## Domain Types

Define verdict types and parse error in domain::guard

- [x] verdict.rs: Decision, GuardVerdict, ParseError type definitions in domain::guard

## Shell Parser (conch-parser adapter)

Thin adapter over conch-parser: AST walking to extract SimpleCommand argv lists, command substitution extraction, depth-limited nesting

- [x] parser.rs: Shell AST parsing via conch-parser adapter (control operators, quoting, command substitution, nesting)

## Guard Policy

Policy engine: env/launcher skip, env -S/--split-string handling, git subcommand detection, variable substitution bypass, shell/python -c nesting, find -exec/xargs patterns

- [x] policy.rs: Guard policy (env/launcher skip, env -S handling, git subcommand detection, variable substitution bypass, recursive nesting)

## CLI Integration

Wire guard check subcommand in apps/cli (binary: sotp), JSON output, exit codes

- [x] CLI guard check subcommand with JSON output and exit codes (binary name: sotp)

## Hook Migration

Rewrite block-direct-git-ops.py as thin wrapper delegating to sotp guard check

- [x] Python hook simplification: rewrite block-direct-git-ops.py as thin CLI delegation wrapper

## Validation

Integration tests, CI pass, verification

- [x] Integration tests and CI pass confirmation
