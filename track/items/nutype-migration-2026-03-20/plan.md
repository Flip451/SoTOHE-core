<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Migrate hand-rolled newtypes to nutype crate (6 types, ~120 lines reduced)

Replace 6 hand-rolled String newtypes with nutype macro declarations.
Retains existing validation functions (is_valid_track_id etc.) via nutype validate(with=..., error=...).
Timestamp is excluded (multi-field struct, not supported by nutype).
Constructor changes from new() to try_new(); all call sites updated.
Estimated ~120 lines of boilerplate removed.

## Dependency setup

Add nutype = "0.6" to workspace Cargo.toml
Add nutype = { workspace = true } to libs/domain/Cargo.toml
No serde feature needed (domain layer has no serde)

- [x] Add nutype 0.6 to workspace dependencies and domain Cargo.toml 5ede331

## ID newtypes migration

Convert TrackId, TaskId, CommitHash, TrackBranch from manual struct + impl to #[nutype(...)] declarations
Retain is_valid_track_id, is_valid_task_id, is_valid_commit_hash validation functions
Use validate(with = |s: &str| if is_valid_track_id(s) { Ok(()) } else { Err(ValidationError::InvalidTrackId(s.to_owned())) }, error = ValidationError)
Derive: Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef, FromStr

- [x] Migrate TrackId, TaskId, CommitHash, TrackBranch to nutype (custom validation fns retained) 5ede331

## NonEmptyString migration

Use sanitize(trim) + validate for non-empty check
Error type: ValidationError::EmptyString

- [x] Migrate NonEmptyString to nutype with sanitize(trim) 5ede331

## ReviewConcern migration

Use sanitize(trim, lowercase) + validate for non-empty check
Error type: ReviewError::InvalidConcern
Note: ReviewConcern uses ReviewError not ValidationError — nutype supports this via error = ReviewError

- [x] Migrate ReviewConcern to nutype with sanitize(trim, lowercase) and ReviewError 5ede331

## Call site migration and cleanup

Replace all Type::new(val) with Type::try_new(val) across all layers
nutype generates try_new() instead of new() for validated types
Remove manual Display impls, as_str() methods, fmt::Display blocks (nutype derives these)
Update tests: new() -> try_new(), as_str() -> as_ref() or .into_inner()

- [x] Update all call sites from new() to try_new() across domain/infrastructure/usecase/cli layers 5ede331
- [x] Remove hand-rolled boilerplate (Display, as_str, fmt impls) and update tests 5ede331
