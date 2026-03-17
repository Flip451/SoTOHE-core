# Verification — Phase 1: sotp CLI Hardening

## Scope Verified

- [ ] T001: TrackDocumentV2 unknown field preservation
- [ ] T002: collect_track_branch_claims skip-and-warn
- [ ] T003: resolve_track_id_from_branch TrackId validation
- [ ] T004: parse_body_findings extended format support
- [ ] T005: block-test-file-deletion hook
- [ ] T006: Task description immutability validation
- [ ] T007: sotp verify spec-attribution
- [ ] T008: sotp verify spec-frontmatter

## Manual Verification Steps

### T001: TrackDocumentV2 unknown field preservation

1. (pass) Create metadata.json with an extra field `"custom_field": "value"`, run decode+encode, verify the field is preserved in output
2. (regression) Verify that a JSON without extra fields round-trips correctly (no spurious fields added)
3. (edge case) Verify that known fields (e.g. `id`, `title`, `tasks`) are deserialized into their proper struct fields, NOT into the `extra` map

### T002: collect_track_branch_claims skip-and-warn

3. (pass) Run collect_track_branch_claims with all valid metadata.json files, verify all tracks returned
4. (fail gracefully) Create a track directory with invalid metadata.json alongside valid ones, verify valid tracks are still returned
5. (stderr) Same as step 4, verify stderr contains a warning message identifying the broken file

### T003: resolve_track_id_from_branch TrackId validation

6. (pass) Call resolve_track_id_from_branch with `track/valid-feature-name`, verify it returns `valid-feature-name`
7. (fail) Call resolve_track_id_from_branch with `track/` followed by invalid characters (e.g. special chars), verify `InvalidTrackId` error
8. (edge case) Call resolve_track_id_from_branch with `track/` (empty suffix), verify `InvalidTrackId` error

### T004: parse_body_findings extended format support

8. (pass numbered) Pass review body with `1. finding text here`, verify it's parsed as a finding
9. (pass plus) Pass review body with `+ finding text here`, verify it's parsed as a finding
10. (negative) Pass review body with plain paragraph text (no list prefix), verify it is NOT parsed as a finding

### T005: block-test-file-deletion hook

11. (block tests/) Trigger hook with a Bash rm command targeting `tests/foo.rs`, verify exit code 2
12. (block *_test.rs) Trigger hook with a Bash rm command targeting `src/user_test.rs`, verify exit code 2
13. (block test_*.rs) Trigger hook with a Bash rm command targeting `src/test_user.rs`, verify exit code 2
14. (allow) Trigger hook with a Bash rm command targeting a non-test file (`src/lib.rs`), verify exit code 0
15. (fail-closed) Send malformed/empty JSON to the hook stdin, verify exit code 2 (fail-closed on internal error)

### T006: Task description immutability validation

13. (reject) Load existing track, change a task description, attempt save, verify error
14. (accept unchanged) Load existing track, save without changing any descriptions, verify success
15. (accept new task) Load existing track, add a new task with a new ID, save, verify success

### T007: sotp verify spec-attribution

18. (fail S-prefix) Create spec.md with `### S-AUTH-01` line lacking `[source: ...]`, run verify, check error output
19. (fail REQ-prefix) Create spec.md with `### REQ-DATA-01` line lacking `[source: ...]`, run verify, check error output
20. (pass S-prefix) Create spec.md with `### S-AUTH-01` line containing `[source: PRD]`, run verify, check it passes
21. (pass REQ-prefix) Create spec.md with `### REQ-DATA-01` line containing `[source: user-interview]`, run verify, check it passes
20. (pass no requirements) Create spec.md with no requirement lines (`### S-` / `### REQ-`), run verify, check it passes
21. (exemption) Create spec.md with non-requirement lines (e.g. `## Scope`, `## Constraints`, bullet items) lacking `[source: ...]`, run verify, check it passes (non-requirement lines are exempt)

### T008: sotp verify spec-frontmatter

24. (fail no frontmatter) Create spec.md without YAML frontmatter, run verify, check error output
25. (fail missing version) Create spec.md with YAML frontmatter containing only `status` but missing `version`, verify error
26. (fail missing status) Create spec.md with YAML frontmatter containing only `version` but missing `status`, verify error
27. (pass) Create spec.md with valid YAML frontmatter (`status`, `version`), run verify, check it passes

## Result

- pending

## Open Issues

- none

## Verified At

- pending
