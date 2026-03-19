# Verification: Nutype Migration

## Scope Verified

- [ ] T001: nutype added to workspace and domain Cargo.toml
- [ ] T002: TrackId, TaskId, CommitHash, TrackBranch migrated
- [ ] T003: NonEmptyString migrated with sanitize(trim)
- [ ] T004: ReviewConcern migrated with sanitize(trim, lowercase)
- [ ] T005: All call sites updated (new → try_new)
- [ ] T006: Boilerplate removed, tests updated

## Manual Verification Steps

1. `cargo make ci` passes
2. ids.rs line count < 250
3. `grep -c '#\[nutype' libs/domain/src/ids.rs libs/domain/src/review.rs` confirms 6 nutype declarations exist (5 in ids.rs, 1 in review.rs)
4. No manual `impl fmt::Display` blocks remain for migrated types in ids.rs or review.rs (ReviewConcern)
5. No manual `fn as_str` or `fn new` methods remain for migrated types (nutype generates try_new and derives AsRef)
6. `grep -rn "TrackId::new\|TaskId::new\|CommitHash::new\|TrackBranch::new\|NonEmptyString::new\|ReviewConcern::new" libs/ apps/` shows no legacy `::new()` constructors (all replaced by `::try_new()`)

## Result

- (pending)

## Open Issues

- (pending)

## Verified At

- (pending)
