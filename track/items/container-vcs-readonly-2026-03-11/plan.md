<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Container Security Hardening: .git ro mount and secret directory exclusion

SEC-06/07/08: Codex workspace-write sandbox and container shell bypass vectors.
Mount .git read-only in all Docker Compose services to prevent direct git operations from containers.
Exclude sensitive directories (private/, config/secrets/) from container mounts to enforce AI read deny rules at OS level.

## Docker Compose .git Read-Only Mount

Change volume mount strategy: keep .:/workspace:rw for source code,
add .git:/workspace/.git:ro overlay to make .git read-only inside containers.
This prevents Codex workspace-write subprocess and `cargo make shell` from running git add/commit/push.

- [x] compose.yml: Mount .git as read-only volume overlay (.git:/workspace/.git:ro)

## Sensitive Directory Exclusion

Use tmpfs overlays for private/ and config/secrets/ inside containers,
so even if these directories exist on host, they appear empty in containers.
This enforces the AI read deny rules at OS level, covering Codex and Gemini subprocesses.

- [x] compose.yml: Exclude private/ and config/secrets/ from container bind mount via tmpfs overlays

## Dev Compose Alignment

Apply the same security mount changes to compose.dev.yml (bacon watcher).

- [x] compose.dev.yml: Apply same .git ro mount and secret exclusion changes

## Validation

Verify all cargo make tasks, CI pipeline, and -exec tasks work correctly with read-only .git.

- [x] Verify cargo make ci and all -exec tasks work with .git read-only mount

## Documentation

Update security convention doc to document container-level enforcement.

- [x] Update project-docs/conventions/security.md with container enforcement scope
