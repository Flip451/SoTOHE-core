<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Phase 1 Safety Hardening: is_test_file path normalization + forbid(unsafe_code)

Phase 1 remaining 2 items: GAP-05 (is_test_file path normalization) and GAP-06 (forbid unsafe_code). Both S-difficulty safety improvements.

## Path normalization for test file detection

Normalize path components in is_test_file using std::path::Path::components() before string pattern matching
Add tests for relative path traversal patterns (../, ./, multi-level ..)

- [x] Add path component normalization to is_test_file (GAP-05) 83140b1

## Forbid unsafe code in library crates

Add #![forbid(unsafe_code)] to libs/domain/src/lib.rs, libs/infrastructure/src/lib.rs, libs/usecase/src/lib.rs
Verify build passes (no existing unsafe code in these crates)

- [x] Add #![forbid(unsafe_code)] to 3 lib crate roots (GAP-06) 83140b1
