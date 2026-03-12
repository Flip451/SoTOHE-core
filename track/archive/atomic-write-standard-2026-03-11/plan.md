<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Atomic Write Standardization: reuse Rust atomic_write_file across Python scripts

SSoT-01: Standardize all critical file writes on Rust atomic_write_file.
Expose as sotp file write-atomic CLI subcommand for Python script delegation.
Depends on filelock-migration track for atomic_write_file implementation.
metadata.json writes already handled by FsTrackStore; this track covers remaining scripts.

## CLI Subcommand

sotp file write-atomic --path <path> reads content from stdin and writes atomically.
Reuses infrastructure::track::atomic_write_file.

- [x] sotp file write-atomic CLI subcommand — expose infrastructure::track::atomic_write_file as CLI tool for Python scripts

## Python Script Migration

Replace direct write_text/open('w') with subprocess call to sotp file write-atomic.
external_guides.py and track_markdown.py are primary targets.

- [x] Migrate external_guides.py save_registry() to use sotp file write-atomic
- [x] Migrate track_markdown.py plan.md/registry.md writes to use sotp file write-atomic

## Verification

Confirm FsTrackStore already covers metadata.json.
Test CLI atomic write for crash-safety.

- [x] Verify metadata.json writes already use atomic pattern via FsTrackStore (from filelock-migration track)
- [x] Tests — verify atomic write CLI produces complete files and cleans up on failure
