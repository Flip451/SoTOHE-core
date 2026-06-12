# T010 E2E Verification Observations

Date: 2026-06-12

## Summary

All 11 acceptance criteria verified. Full CI (`cargo make ci`) passed (exit 0).

---

## Per-AC Results

### AC-01 — No subscriber init in domain/usecase; init only in cli-composition

**Result: PASS**

- Method: Read `libs/domain/Cargo.toml` and `libs/usecase/Cargo.toml` — neither lists `tracing` or `tracing-subscriber` as a dependency.
- Evidence: `apps/cli/src/main.rs:114` calls `cli_composition::telemetry_wiring::init_tracing_subscriber()` at the `main()` entry point. `apps/cli-composition/src/telemetry_wiring.rs:39-47` contains the single `OnceLock`-guarded `tracing_subscriber::fmt().try_init()` call.
- Domain/usecase source trees have no tracing imports (confirmed by absence in Cargo.toml — no transitive concern since neither crate declares tracing as a dependency).

### AC-02 — track operation subcommand emits TrackSubcommand event to telemetry.jsonl

**Result: PASS**

- Method: Inspected `track/items/track-workflow-telemetry-2026-06-10/logs/telemetry.jsonl` (live file written during this track session).
- Evidence (line 1): `{"event_type":"TrackSubcommand","schema_version":1,"track_id":"track-workflow-telemetry-2026-06-10","command":"track type-signals","exit_code":0,"duration_ms":18803,"timestamp":"2026-06-10T17:13:45.528975634+00:00"}`
- Required fields present: `command`, `exit_code`, `duration_ms`, `track_id`.
- Code path: `apps/cli/src/main.rs` `execute_track_with_telemetry()` captures `Instant::now()` before dispatch and calls `emit_track_subcommand` after.

### AC-03 — GateEval events contain gate_name / verdict / reason_summary / duration_ms

**Result: PASS**

- Method: Inspected telemetry.jsonl from line 307 onward (current code, no input_hash).
- Evidence (line 307): `{"event_type":"GateEval","schema_version":1,"track_id":"track-workflow-telemetry-2026-06-10","gate_name":"verify-layers","verdict":"ok","reason_summary":"--- verify layers ---\n[OK] All checks passed.\n--- verify layers PASSED ---","duration_ms":321,...}`
- Required fields present: `gate_name`, `verdict`, `reason_summary`, `duration_ms`, `timestamp`.
- Note: Early events (lines 53–79) include an extra `input_hash` field from an earlier intermediate build before the field was removed. Current code (`TelemetryEvent::GateEval` struct in `libs/infrastructure/src/telemetry/mod.rs:65-80`) has no `input_hash` field. Recent events confirm the current behavior is compliant.
- ReviewRound event: confirmed by unit tests in `apps/cli-composition/src/telemetry_wiring.rs` (test `test_emit_review_round_writes_review_round_event_with_required_fields`).

### AC-04 — HookBlock and AdvisoryHookFired recorded; allow path not recorded

**Result: PASS**

- Method: Code inspection of `apps/cli/src/main.rs` `execute_hook_with_telemetry()` (lines 241–301).
- Evidence: Block path (`outcome.exit_code == 2`) calls `emit_hook_block`. Advisory path with non-None stdout calls `emit_advisory_hook_fired`. All other outcomes (`is_block == false && !is_advisory`) fall through with no emit (comment: "All other paths (allow, advisory with no injection): no emit (OS-03)").
- Unit test `test_emit_hook_block_writes_hook_block_event` and `test_emit_advisory_hook_fired_writes_advisory_hook_fired_event` in `telemetry_wiring.rs` cover the emit paths; `test_no_emit_on_allow_path_leaves_no_file` confirms no file created when nothing emitted.

### AC-05 — SOTP_TELEMETRY=0 kills writes; SOTP_TELEMETRY_DIR redirects output

**Result: PASS**

- Method: Code inspection of `apps/cli-composition/src/telemetry_wiring.rs:94-110` (`resolve_telemetry_writer_inner`) and `libs/infrastructure/src/telemetry/writer.rs`.
- Evidence: `resolve_telemetry_writer_inner` returns `None` when `config.is_enabled()` is false (kill switch). Unit test `test_resolve_telemetry_writer_returns_none_when_kill_switch_set` in `telemetry_wiring.rs` verifies `SOTP_TELEMETRY=0` returns `None`. `TelemetryConfig::from_env` reads `SOTP_TELEMETRY_DIR` for override. Test `test_resolve_telemetry_writer_relative_items_dir_anchors_at_repo_root` confirms custom dir write.

### AC-06 — No file open on display-only commands (lazy init)

**Result: PASS**

- Method: Code inspection and unit test.
- Evidence: `TelemetryWriter::write` uses a `OnceLock<File>` / lazy-init pattern — file is only opened on first `write()` call. `execute_track_with_telemetry` short-circuits immediately for display-only commands (returns without constructing writer). `test_no_emit_on_allow_path_leaves_no_file` asserts no file created when no emit happens.

### AC-07 — Makefile test tasks set SOTP_TELEMETRY=0

**Result: PASS**

- Method: Read `Makefile.toml` lines 167-217.
- Evidence: `test-local` (line 170), `test-doc-local` (line 176), `test-nocapture-local` (line 183), `llvm-cov-local` (line 215) all declare `env = { SOTP_TELEMETRY = "0" }`.

### AC-08 — `bin/sotp telemetry report` aggregates and exits 0; skip count present

**Result: PASS**

- Method: Code inspection of `libs/infrastructure/src/telemetry/report.rs` and `apps/cli-composition/src/telemetry.rs`.
- Evidence: `TelemetryReport::aggregate()` collects `PhaseDurationSummary` from `TrackSubcommand` events, `TelemetryErrorEntry` from `NonZeroExit` events, `TelemetryHookBlockEntry` from `HookBlock` events. `skipped_lines` counter increments on any parse/skip. `TelemetryReportError::TrackNotFound` maps to exit code 1.
- Unit tests in `report.rs` cover normal aggregation, corrupted-line skipping, unknown schema_version skipping, missing file empty result, and TrackNotFound.
- CLI dispatch test `test_telemetry_report_dispatch_via_run_cli_succeeds_with_existing_track` in `apps/cli/src/main.rs` confirms routing.

### AC-09 — Each JSONL line has schema_version; no file-header pattern

**Result: PASS**

- Method: Read telemetry.jsonl — every line contains `"schema_version":1`.
- Evidence: All observed lines in `telemetry.jsonl` include `"schema_version":1` as a top-level field. `TelemetryEvent` struct uses `#[serde(tag = "event_type")]` inline tagging with `schema_version` as a per-variant field (not a file header).

### AC-10 — .gitignore excludes track/items/**/logs/

**Result: PASS**

- Method: Read `.gitignore`.
- Evidence: `.gitignore` line 90: `track/items/**/logs/`

### AC-11 — No telemetry on non-track/* branches

**Result: PASS**

- Method: Code inspection of `resolve_telemetry_writer_inner` (`telemetry_wiring.rs:94-110`) and `resolve_telemetry_context_from_branch` (lines 122-133).
- Evidence: `resolve_telemetry_context_from_branch` calls `resolve_track_id_from_branch` which returns `None` for non-`track/*` branches. `resolve_telemetry_writer_inner` returns `None` when `track_id` arg is `None`. Unit test `test_resolve_telemetry_writer_returns_none_when_no_track_id` confirms `None` track_id yields `None` writer.

---

## CI Result

`cargo make ci` passed (exit 0) — all gates passed:
- fmt-check, clippy, nextest, deny, check-layers
- verify-domain-purity, verify-usecase-purity, verify-view-freshness
- verify-plan-artifact-refs, verify-adr-signals, verify-spec-states
- verify-catalogue-spec-refs, verify-catalogue-spec-signals

## Cleanup

No test residue was found under `track/items/` (the existing `logs/telemetry.jsonl` is the legitimate runtime log for this track, gitignored per AC-10 and not version-controlled).

## Notes

- Early GateEval entries in `telemetry.jsonl` (approx lines 53–79) contain a now-removed `input_hash` field from an earlier intermediate build. These are parse-compatible (serde ignores unknown fields on deserialization via `deny_unknown_fields` absent), and the current `TelemetryEvent::GateEval` struct does not emit this field. Lines from 2026-06-11 onward lack `input_hash`, confirming compliance.
- T011 (`bin/sotp track archive` includes `logs/` in move) remains in `todo` status and is out of scope for T010.
