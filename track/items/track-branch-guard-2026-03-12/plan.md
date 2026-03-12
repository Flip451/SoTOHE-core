<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Track Branch Enforcement Guard — system-level guarantee that track operations run on the correct branch

Track operations (commit, transition, add_task, set_override) must be rejected when the current git branch does not match the track's metadata.json branch field.
Enforcement is system-level: Rust value objects + Python guard functions + cargo make task wrappers.
No prompt-based enforcement — all guards are deterministic checks in code.
Branch guard skip policy: (1) branch=null in metadata.json → guard skips (legacy/planning phase compatibility), (2) detached HEAD → guard rejects (ambiguous state), (3) --skip-branch-check flag → guard skips (test/CI escape hatch), (4) now parameter set (test determinism) → guard skips in Python path.
TOCTOU acceptance: branch check is a best-effort precondition, not a git-level lock. The residual race between check and git-commit is accepted as the window is sub-second and the threat model is accidental misuse, not adversarial.

## Rust Domain Layer — TrackBranch value object and TrackMetadata branch field

Add TrackBranch validated value object to ids.rs with track/<slug> format.
Add branch: Option<TrackBranch> to TrackMetadata.
Update codec to round-trip branch field.

- [x] Add TrackBranch value object to Rust domain (libs/domain/src/ids.rs) with track/<slug> format validation
- [x] Add branch field to TrackMetadata in Rust domain (track.rs) and update constructors/accessors
- [x] Update infrastructure codec (codec.rs) to serialize/deserialize branch field in metadata.json

## Rust CLI — sotp track transition branch guard

Auto-detect current git branch before transition.
Read branch from metadata.json and reject if mismatch.
Add --skip-branch-check flag for CI/testing escape hatch.

- [x] Add branch validation to sotp track transition CLI — auto-detect current git branch and reject mismatch with metadata.json branch

## Python Guard Layer — verify_track_branch() and centralized enforcement

Add verify_track_branch() that compares current git branch with metadata.json branch. Extend current_git_branch() to return 'HEAD' sentinel for detached HEAD (distinct from None for non-repo).
Centralize guard in _save_metadata() to cover ALL mutation paths (transition_task, add_task, set_track_override).
Integrate into commit_from_file() via explicit track-dir.txt file: /track:commit skill writes tmp/track-commit/track-dir.txt, commit_from_file() reads it, validates path (repo-relative under track/items/<id> with existing metadata.json), rejects on branch mismatch, and cleans up track-dir.txt on both success and failure. Add track-dir.txt to TRANSIENT_AUTOMATION_FILES. When track-dir.txt is absent (non-track commits like takt), fall back to branch-based auto-detection.
Update cargo make tasks: track-commit-message and commit-pending-message both support the track-dir.txt → branch guard flow.

- [x] Add verify_track_branch() guard function in Python (track_branch_guard.py) — compare current git branch with metadata.json branch; define skip policy for branch=null, detached HEAD, and --skip-branch-check flag. Extend current_git_branch() to distinguish detached HEAD (returns sentinel 'HEAD') from non-repo (returns None)
- [x] Centralize branch guard in _save_metadata() to cover all mutation paths: transition_task(), add_task(), set_track_override(). Add skip_branch_check parameter for test/planning bypass
- [x] Integrate branch guard into git_ops.py commit_from_file() — add --track-dir CLI option; /track:commit skill writes tmp/track-commit/track-dir.txt alongside commit-message.txt; commit_from_file() reads track-dir.txt, validates it is a repo-relative path under track/items/<id> with existing metadata.json, then rejects on branch mismatch; track-dir.txt is cleaned up (deleted) on both success and failure alongside commit-message.txt; add track-dir.txt to TRANSIENT_AUTOMATION_FILES
- [x] Update cargo make tasks: track-commit-message reads tmp/track-commit/track-dir.txt for explicit track context (written by /track:commit skill); track-transition passes track context via existing track_dir argument; commit-pending-message (takt path) uses branch-based auto-detection as fallback

## Testing and Validation

Rust unit tests for TrackBranch validation.
Python tests for guard function: mismatch rejection, null branch skip, detached HEAD rejection, --skip-branch-check bypass.
Integration test: cargo make ci passes.

- [x] Add comprehensive tests: Rust unit tests for TrackBranch, Python tests for guard function (mismatch, null branch, detached HEAD, skip flag), integration points, cargo make CI validation
