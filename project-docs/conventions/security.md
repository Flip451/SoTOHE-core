# Security Convention

## Sensitive Directories

This project defines two project-specific sensitive directories. Files in these directories must
not be committed to version control and must not be read by Claude Code.

> **Scope of enforcement**: The `Read` / `Grep` deny rules in `.claude/settings.json` apply only
> to Claude Code's own tool calls. They do **not** apply inside a Codex subprocess
> (`workspace-write` sandbox) or when Gemini CLI accesses the filesystem directly — see
> `.claude/rules/02-codex-delegation.md` for details. When using Codex with `workspace-write`,
> instruct it explicitly not to read files under `private/` or `config/secrets/`.

### `private/`

Purpose: Local certificates, TLS credentials, SSH keys, and other host-specific secrets that never
leave the developer's machine.

- **Git**: Must not be committed. Add `private/` to `.gitignore`.
- **AI read**: Prohibited. `Read(./private/**)` and `Grep(./private/**)` are in
  `.claude/settings.json` deny.
- **Typical contents**: `dev-cert.crt`, `dev-key.pem`, host-specific config with embedded credentials.

### `config/secrets/`

Purpose: Application-level secrets for local development (OAuth client IDs, API keys, database
passwords, and other credential files).

- **Git**: Must not be committed. Add `config/secrets/` to `.gitignore`.
- **AI read**: Prohibited. `Read(./config/secrets/**)` and `Grep(./config/secrets/**)` are in
  `.claude/settings.json` deny.
- **Typical contents**: `local.toml`, `oauth/client.json`, environment-specific credential files.

## Enforcement

When adding a new sensitive directory to this project:

1. Add the directory to `.gitignore`.
2. Add `Read(./new-dir/**)` and `Grep(./new-dir/**)` deny rules to `.claude/settings.json`.
3. Add corresponding entries to `EXPECTED_DENY` in `scripts/verify_orchestra_guardrails.py` so the
   verifier requires the rules in CI.
4. Add regression tests in `scripts/test_verify_scripts.py` to guard against accidental removal.
5. Document the directory purpose in this file.
