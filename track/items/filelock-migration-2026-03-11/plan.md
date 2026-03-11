<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Track Store: Rust-based metadata.json locking with TrackReader/TrackWriter ports

CON-01 + INF-04: Replace racy find/save with locked read-modify-write in Rust.
Apply ISP: split TrackRepository into TrackReader (read-only) and TrackWriter (atomic mutation).
FsTrackStore reuses existing FileLockManager (fd-lock) for exclusive access.
Atomic write (tmp + fsync + rename) prevents partial-write corruption.
Python track_state_machine.py transitions to thin CLI delegation.

## Domain Ports (ISP + DIP)

Split TrackRepository into TrackReader and TrackWriter.
TrackWriter::update<F> expresses atomic read-modify-write at the port level.
Locking is an infrastructure concern — not exposed in the port signature.

- [ ] domain::repository — Split TrackRepository into TrackReader (find) and TrackWriter (create, update<F>) ports (ISP)

## Infrastructure: Track Store

TrackDocumentV2 serde types matching Python track_schema.py for round-trip compatibility.
atomic_write_file utility for crash-safe persistence.
FsTrackStore wires FileLockManager + atomic_write + codec together.

- [ ] infrastructure::track::codec — TrackDocumentV2 serde types for metadata.json schema compatibility with Python track_schema.py
- [ ] infrastructure::track::atomic_write — atomic_write_file() using tmp-in-same-dir + fsync + rename + parent fsync
- [ ] infrastructure::track::fs_store — FsTrackStore implementing TrackReader + TrackWriter with FileLockManager for exclusive metadata.json access

## UseCase Migration (SRP)

TransitionTaskUseCase uses TrackWriter::update — business logic only, no locking concern.

- [ ] usecase — Migrate TransitionTaskUseCase from find/save to TrackWriter::update closure pattern (eliminate race condition)

## CLI and Python Integration

Wire FsTrackStore into CLI composition root.
Python delegates to sotp track for metadata.json mutations.

- [ ] apps/cli — Wire FsTrackStore into CLI composition root, replace InMemoryTrackRepository usage
- [ ] Python scripts — track_state_machine.py delegates metadata.json writes to sotp track commands (thin wrapper)

## Tests

Schema compatibility: Rust ↔ Python round-trip tests.
Concurrency: parallel FsTrackStore::update simulation.

- [ ] Schema compatibility tests — verify Rust TrackDocumentV2 and Python track_schema.py produce identical metadata.json
- [ ] Concurrency tests — parallel metadata.json write simulation via FsTrackStore
