# Layer & Responsibility Violation Audit (2026-06-21)

The `apps/cli-composition` crate has undergone significant responsibility dilution, accumulating presentation rendering, multi-step orchestration, stringly-typed error boundaries, filesystem adapter implementations, and a stateless god-facade that now spans 51 public methods across 27 implementation files. Rather than functioning as a pure DI wiring root as mandated by ADR 2026-05-27-0110, `cli-composition` has become the de-facto application layer, absorbing concerns that hexagonal architecture assigns to other roles: primary-adapter rendering (stdout/stderr string assembly), usecase application services (business sequencing and I/O interactor logic), and infrastructure (port-implementing adapter structs). The `CliApp` unit struct carries no injected state, making per-context isolation and independent testability impossible. The cli binary itself (ADR 2026-04-30-0848) additionally performs direct file I/O and JSON serialisation that must be delegated. Collectively these violations block the ADR 2026-06-21-1328 decomposition plan, which proposes introducing a `cli_driver` primary adapter, typed `CompositionError`, per-context composers, and moving orchestration interactors to `usecase`.

---

## Summary

| Category | Count | Max Severity | Enforcement |
|---|---|---|---|
| presentation-in-composition-root | 5 | high | design-level-only |
| json-output-assembly-in-composition-root | 4 | high | design-level-only |
| pre-formatted-stdout-stderr-in-composer-methods | 7 | high | design-level-only |
| multi-step-orchestration | 6 | high | design-level-only |
| stringly-typed-error-boundary | 1 | high | design-level-only |
| god-facade | 1 | high | design-level-only |
| cli-bin-business-logic | 1 | high | design-level-only |
| cli-composition-god-facade | 1 | high | design-level-only |
| cli-composition-orchestration-leak | 1 | medium | design-level-only |
| cli-composition-presentation-leak | 1 | high | design-level-only |
| cli-composition-stringly-typed-error | 1 | high | design-level-only |
| adapter-outside-infrastructure | 7 | high | warning-only |

---

## Findings by Category

### presentation-in-composition-root

- `apps/cli-composition/src/cmd_outcome.rs:12-33` — **ADR 2026-06-21-1328: all rendering must move to `cli_driver` primary adapter render module** — `render_outcome` assembles user-facing stdout banner strings (`--- {label} ---`, `--- {label} PASSED ---`, `--- {label} FAILED ---`) and is called from at least three production sites (signal.rs, signal_layer_chain.rs, verify.rs); it is a pure presentation function with no DI wiring purpose. — _severity: high_ — `pub(crate) fn render_outcome(...) { let mut lines = vec![format!("--- {label} ---")]; ... lines.push(format!("--- {label} PASSED ---")); CommandOutcome::success(Some(lines.join("\n"))) }`

- `apps/cli-composition/src/signal.rs:23-47` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — `merge_outcomes` reads `.stdout`/`.stderr` strings from multiple `CommandOutcome` values, merges them, and appends `--- {label} PASSED ---` / `--- {label} FAILED ---` banners. This is pure text aggregation/presentation called from `signal_check_gate` to produce the combined gate output. — _severity: high_ — `fn merge_outcomes(label: &str, outcomes: Vec<CommandOutcome>) -> CommandOutcome { let mut all_lines: Vec<String> = vec![format!("--- {label} ---")]; ... all_lines.push(summary); CommandOutcome { stdout: Some(all_lines.join("\n")), ... } }`

- `apps/cli-composition/src/verify.rs:8-11` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — `render_skip` constructs a multi-line user-visible skip banner (`--- {label} ---\n[SKIP] {reason}\n--- {label} SKIPPED ---`) using `format!` and places it in `CommandOutcome.stdout`; it is a rendering helper with no wiring role. — _severity: high_ — `fn render_skip(label: &str, reason: &str) -> CommandOutcome { let stdout = format!("--- {label} ---\n[SKIP] {reason}\n--- {label} SKIPPED ---"); CommandOutcome::success(Some(stdout)) }`

- `apps/cli-composition/src/review_v2/results.rs:24-244` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module; CliApp god-facade dissolved into per-context composers** — `render_review_results_str` is a ~220-line rendering engine that assembles the entire human-readable review results report: per-scope state indicators (`[+]`/`[-]`/`[.]`), round history, findings blocks (severity, location, category), history list, and an approval summary line, using `writeln!`, `format!`, and three local rendering helpers. — _severity: high_ — `pub(crate) fn render_review_results_str(...) -> Result<String, String> { let _ = writeln!(out, "Review results (v2 scope-based):"); ... let _ = writeln!(out, "  {indicator} {scope}: {state}{suffix}"); ... let _ = writeln!(out, "Summary: {approved_count} approved, ..."); }`

- `apps/cli-composition/src/pr/poll.rs:514-547` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — `format_review_summary` assembles the full user-facing PR review result text block (header, PR number, review ID, state, inline comment count, review body, numbered findings list) via multiple `format!` calls and `.join("\n")` inside the composition root. — _severity: high_ — `pub(super) fn format_review_summary(pr: &str, result: &usecase::pr_review::PrReviewResult) -> String { let mut lines = Vec::new(); lines.push(format!("PR: #{pr}")); ... lines.push(format!("  {}. {}: {}", i + 1, location, f.body)); lines.join("\n") }`

---

### json-output-assembly-in-composition-root

- `apps/cli-composition/src/guard.rs:42-47` — **ADR 2026-06-21-1328: output formatting belongs in `cli_driver` render module, not DI wiring layer** — `guard_check` assembles the `{"decision": ..., "reason": ...}` JSON verdict payload using `serde_json::json!` directly inside the composer method body, then converts it to a string for `CommandOutcome.stdout`. Formatting the output envelope is a presentation responsibility. — _severity: high_ — `let json = serde_json::json!({ "decision": decision_str, "reason": reason, }); let stdout = json.to_string(); Ok(CommandOutcome { stdout: Some(stdout), ... })`

- `apps/cli-composition/src/hook.rs:241-248` — **ADR 2026-06-21-1328: output formatting belongs in `cli_driver` render module** — `hook_dispatch_user_prompt_submit` constructs a `hookSpecificOutput` JSON envelope using `serde_json::json!` and serialises it for `CommandOutcome.stdout`; assembling the wire protocol JSON for Claude Code's hook response belongs in the `cli_driver` primary adapter render module. — _severity: high_ — `let stdout = usecase::skill_compliance::check_compliance_render(&prompt).map(|ctx| { serde_json::json!({ "hookSpecificOutput": { "hookEventName": "UserPromptSubmit", "additionalContext": ctx, } }).to_string() });`

- `apps/cli-composition/src/track/ops.rs:177-192` — **ADR 2026-06-21-1328: output formatting belongs in `cli_driver` render module; ADR 2026-05-27-0110: CliApp must not carry multi-step orchestration** — `track_next_task_resolved` assembles the task status JSON payload `{"task_id": ..., "description": ..., "status": ...}` via `serde_json::json!` inside the composer body (two sites in ops.rs, two parallel sites in track/mod.rs). `track_task_counts_resolved` additionally hand-builds JSON via a raw `format!` with embedded `{{...}}` braces. — _severity: high_ — `serde_json::json!({ "task_id": task.task_id, "description": task.description, "status": task_status, }) ... Ok(CommandOutcome::success(Some(payload.to_string())))` / `let json = format!(r#"{{"total":{total},"todo":{},...}}", counts.todo, ...);`

- `apps/cli-composition/src/semantic_dup/measure_quality.rs:92-106` — **ADR 2026-06-21-1328: output formatting belongs in `cli_driver` render module** — `dup_index_measure_quality` assembles the metrics JSON output object (mean_cosine, cosine_std_dev, cosine_percentiles, above_threshold_rate) using `serde_json::json!` and serialises with `serde_json::to_string_pretty` inside the composer method body. — _severity: high_ — `let json = serde_json::to_string_pretty(&serde_json::json!({ "mean_cosine": metrics.mean_cosine, ... })).map_err(|e| format!("failed to serialize metrics to JSON: {e}"))?; Ok(CommandOutcome::success(Some(json)))`

---

### pre-formatted-stdout-stderr-in-composer-methods

- `apps/cli-composition/src/review_v2/mod.rs:424-452` — **ADR 2026-06-21-1328; ADR 2026-05-27-0110: cli_composition must not carry presentation/output-formatting** — `review_check_approved` constructs user-visible `[OK]`/`[WARN]`/`[BLOCKED]` messages using `format!` directly and places them in `CommandOutcome.stderr`. Three distinct message templates are built inline. — _severity: high_ — `format!("[WARN] No review.json found. Allowing commit for PR-based review ({count} scope(s)).") / format!("[BLOCKED] Review not approved. Required scopes:\n{}", display.join("\n")) / Ok(CommandOutcome { stdout: None, stderr: Some(msg), exit_code })`

- `apps/cli-composition/src/git.rs:179-201` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — `git_switch_and_pull` pushes `format!("Switching to {branch}...")`, `format!("Pulling latest from origin/{branch}...")`, `format!("[OK] On {branch}, up to date.")`, and `"[WARN] Pull failed..."` into a `Vec<String>` and joins them for `CommandOutcome.stdout`. — _severity: medium_ — `stdout_lines.push(format!("Switching to {branch}...")); ... stdout_lines.push(format!("[OK] On {branch}, up to date.")); Ok(CommandOutcome::success(Some(stdout_lines.join("\n"))))`

- `apps/cli-composition/src/pr.rs:101-116, 160-203, 277-280, 385-389` — **ADR 2026-06-21-1328; ADR 2026-05-27-0110: CliApp must not carry presentation** — Multiple `CliApp` methods in `pr.rs` assemble user-facing formatted output directly: `pr_status` builds `format!("PR: {url}")` + check result strings; `pr_wait_and_merge` builds `[BLOCKED]` finding lines; `pr_trigger_review` formats a `[OK] Posted '@codex review'...\nTRIGGER_TIMESTAMP=...` multi-line string; `pr_pr_review` formats a `=== PR Review Result: PASS ===` multi-line banner. Approximately 10 distinct format sites across this file alone. — _severity: high_ — `let mut lines = vec![format!("PR: {url}")]; lines.push(format!("[FAIL] Failed checks: {}", names.join(", "))); Ok(CommandOutcome { stdout: Some(lines.join("\n")), ... })` / `let stdout = format!("\n=== PR Review Result: PASS ===\nPR: #{pr_number}\nZero findings detected...");`

- `apps/cli-composition/src/track/mod.rs:226-231, 263, 292, 318-325, 372-377, 425-430, 460-465, 490-495, 599` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — Nine call sites across `track/mod.rs` build user-facing text via `format!` and `.join("\n")` before placing it in `CommandOutcome.stdout`. `sync_views_to_stdout` (lines 117-130) produces `[OK] Rendered: {path}` and warning strings; `track_transition` (226-231) formats `[OK] {task_id}: transitioned to {target_status}...`; `track_branch_create`, `track_branch_switch`, `track_resolve`, `track_add_task`, `track_set_override`, `track_clear_override`, and `track_archive` each assemble their own user-facing lines. — _severity: high_ — `fn sync_views_to_stdout(...) -> Vec<String> { ... format!("[OK] Rendered: {}", ...) ... vec![format!("warning: operation persisted but sync-views failed: {err}")] }`

- `apps/cli-composition/src/domain.rs:51-55` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — `domain_export_schema` constructs `format!("[OK] Schema written to {}", path.display())` and places it in `CommandOutcome.stderr`. — _severity: medium_ — `Ok(CommandOutcome { stdout: None, stderr: Some(format!("[OK] Schema written to {}", path.display())), exit_code: 0, })`

- `apps/cli-composition/src/ref_verify.rs:135-158, 264-269` — **ADR 2026-06-21-1328: rendering must move to `cli_driver` primary adapter render module** — `ref_verify_run` (135-158) constructs three user-visible stderr messages via `format!`: `[OK] Semantic reference verification passed`, `[BLOCKED] Semantic review confirmed {pair_count} production failure(s)...`, and `[ESCALATE] Human review required...`. `ref_verify_check_approved` (264-269) formats a `[BLOCKED] ref-verify check-approved failed: {n} pair(s) without Pass cache:\n{...}` block. — _severity: high_ — `stderr: Some(format!("[BLOCKED] Semantic review confirmed {pair_count} production failure(s). Resolve the failures before committing."))` / `stderr: Some(format!("[BLOCKED] ref-verify check-approved failed: {} pair(s) without Pass cache:\n{}", missing_or_non_pass.len(), missing_or_non_pass.join("\n")))`

- `apps/cli-composition/src/dry.rs:437-492` — **ADR 2026-06-21-1328: presentation-formatting belongs in the `cli_driver` primary adapter render module, not cli_composition** — `dry_results` manually formats each `DryCheckRecord` into display strings with rich field-level rendering (pair key, changed path, verdict label, score/threshold/base, rationale, recorded_at) using `format!` and match-on-verdict dispatch. cli_composition should produce wired drivers/use cases while `cli_driver` renders typed output DTOs. — _severity: medium_ — `for record in &results.records { lines.push(format!("  pair: [{} ({})] <-> [{} ({})]\", ..., record.pair_key().low().path(), ...)); ... let verdict_str = match record.verdict() { DryCheckVerdict::NotAViolation => ..., ... }; lines.push(format!("  verdict: {verdict_str}")); }`

---

### multi-step-orchestration

- `apps/cli-composition/src/signal.rs:437-567` — **ADR 2026-06-21-1328: multi-step orchestration belongs in usecase; ADR 2026-05-27-0110: CliApp facade must be THIN** — `signal_check_gate` manually sequences four signal chains (ADR-user, spec-ADR, catalog-spec, impl-catalog), resolves strictness for each from a gate matrix, calls each chain's infrastructure verifier inline, and aggregates results via `merge_outcomes`. This is business sequencing (what checks run, in what order, with what parameters, how failures aggregate) — not DI wiring. — _severity: high_ — `let chain0 = { ... infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict(...) }; let chain1 = { ... infrastructure::verify::spec_states::verify_from_spec_json(...) }; let chain2 = match signal_check_layer_chain_with_strict(...); let chain3 = match signal_check_layer_chain_with_strict(...); Ok(merge_outcomes(gate_label, vec![chain0, chain1, chain2, chain3]))`

- `apps/cli-composition/src/signal.rs:167-194` — **ADR 2026-06-21-1328: multi-step I/O sequences belong in usecase** — `signal_calc_spec_adr` performs a full read→decode→evaluate→mutate→re-encode→write cycle entirely in cli_composition: `std::fs::read_to_string`, `spec_codec::decode`, `doc.evaluate_signals()`, `doc.set_signals()`, `spec_codec::encode`, `atomic_write_file`. Steps 2-5 are business logic that belong in a usecase interactor. — _severity: medium_ — `let json_content = std::fs::read_to_string(&spec_json_path)...; let mut doc = spec_codec::decode(&json_content)...; let counts = doc.evaluate_signals(); doc.set_signals(counts); let encoded = spec_codec::encode(&doc)...; atomic_write_file(&spec_json_path, format!("{encoded}\n").as_bytes())...`

- `apps/cli-composition/src/track/fixpoint_resolve.rs:214-291` — **ADR 2026-06-21-1328: multi-step diff-fragment computation belongs in usecase** — `build_current_fragment_refs` manually sequences: CWD mutation for git discovery, `list_changed_hunks` via infrastructure, `extract_code_fragments` via infrastructure, path normalization loop, changed-path filtering, `fragments_overlapping_hunks` domain call, and fragment-ref derivation loop. This entire pipeline is business logic that belongs in a usecase interactor. — _severity: medium_ — `let getter = GitDryCheckDiffGetter; let changed_hunks_result = getter.list_changed_hunks(base); ... let raw_fragments = extract_code_fragments(canonical_root)...; let mut normalized: Vec<CodeFragment> = ...; ... let diff_fragments = fragments_overlapping_hunks(&candidates, &changed_hunks); let mut refs = BTreeSet::new(); for fragment in &diff_fragments { ... refs.insert(r); } Ok(refs)`

- `apps/cli-composition/src/track/fixpoint_resolve.rs:428-518` — **ADR 2026-05-27-0110 / ADR 2026-06-21-1328: conditional multi-branch orchestration must not reside in cli_composition** — Inside `CliApp::fixpoint_resolve`, a large `if usecase_dry_config.enabled { ... } else { ... }` block manually orchestrates: diff-base resolution (with its own CWD guard), corpus-root manifest branching on `symlink_metadata` result, workspace-root selection between two code paths, `build_current_fragment_refs` invocation, and final adapter construction. The dry-gate enabled/disabled branching and all corpus-fingerprint conditional logic is business sequencing. — _severity: medium_ — `let (current_fragment_refs, dry_approval): (...) = if usecase_dry_config.enabled { let original_cwd = std::env::current_dir()...; std::env::set_current_dir(&repo_root)...; let base_result = resolve_dry_diff_base_for_track(...); ... let (approval_workspace_root, current_corpus_fingerprint) = match std::fs::symlink_metadata(&corpus_root_manifest_path) { ... }; let refs = build_current_fragment_refs(...)?; ... } else { (BTreeSet::new(), Arc::new(NoOpDryApprovalService) ...) };`

- `apps/cli-composition/src/dry.rs:318-425` — **ADR 2026-06-21-1328: business aggregation beyond adapter wiring belongs outside cli_composition** — After `interactor.run_dry_check` returns, `dry_write` reads the store again (`store_for_summary.read_records`) to compute `records_before`/`records_after`/`records_appended`, and derives `pairs_checked` via `saturating_sub`. This post-execution metric aggregation is business logic; the interactor should return these counts. — _severity: low_ — `let records_before = store_for_summary.read_records()...; ... let dry_result = interactor.run_dry_check(...); ... let records_after = store_for_summary.read_records()...; let records_appended = records_after.saturating_sub(records_before); let pairs_checked = records_appended; Ok(dry_write_outcome(&findings, pairs_checked, records_appended, diff_fragments_processed))`

- `apps/cli-composition/src/dry.rs:560-592` — **ADR 2026-06-21-1328: looping over domain objects to build derived state belongs in usecase** — `dry_check_approved` manually iterates `diff_fragments` to build a `BTreeSet<FragmentRef>` via `fragment_ref_of` (imported from `usecase::dry_check`). Executing a usecase-owned domain-derivation function inside a cli_composition loop is business sequencing that belongs in a usecase interactor. — _severity: low_ — `let mut current_fragment_refs: BTreeSet<domain::dry_check::FragmentRef> = BTreeSet::new(); for fragment in &diff_fragments { let fragment_ref = fragment_ref_of(fragment)...; current_fragment_refs.insert(fragment_ref); }`

---

### multi-step-orchestration (cli bin)

*(See cli-bin-business-logic below — the cli binary violation is categorised separately.)*

---

### stringly-typed-error-boundary

- `apps/cli-composition/src` (representative: `domain.rs:24`, `dry.rs:240`, `verify.rs:115`, `git.rs:15`, `ref_verify.rs:89`) — **ADR 2026-05-27-0110 D2 / ADR 2026-06-21-1328 D2: public method error type must be typed CompositionError, not String** — The audited 51-method `CliApp` public surface uses stringly typed composition boundaries such as `Result<CommandOutcome, String>`. No `CompositionError` type exists anywhere in the source tree; it appears only in ADR prose. The stringly-typed boundary forces callers to interpret opaque error strings instead of pattern-matching typed variants, making error handling fragile and unauditable. — _severity: high_ — `pub fn domain_export_schema(&self, ...) -> Result<CommandOutcome, String>` / `pub fn dry_write(&self, ...) -> Result<CommandOutcome, String>` / `.map_err(|e| format!("atomic write failed: {e}"))`

---

### cli-composition-stringly-typed-error

- `apps/cli-composition/src/arch.rs:12, 22, 31, 40, 49` — **ADR 2026-05-27-0110 D2; ADR 2026-06-21-1328: introduces typed CompositionError** — All four public `CliApp` methods in `arch.rs` return `Result<CommandOutcome, String>`, representative of the pervasive ~255 `.map_err(|e| format!(...))` and `Err(format!(...))` sites across the entire crate that must be replaced with `CompositionError` variants. — _severity: high_ — `pub fn arch_tree(&self, project_root: &Path) -> Result<CommandOutcome, String>;`

---

### god-facade

- `apps/cli-composition/src/lib.rs:160` (plus impl blocks across 27 files) — **ADR 2026-05-27-0110: CliApp must be THIN; ADR 2026-06-21-1328: god-facade must be dissolved into per-context composers** — `CliApp` is declared as a unit struct (`pub struct CliApp;`) with no fields, holding no state and performing no real dependency injection. Its implementation is spread across 27 `impl CliApp` blocks covering 20 distinct command families (verify, dry, track, pr, review_v2, signal, git, arch, hook, semantic_dup, etc.) exposing 51 public methods. The single unit-struct accumulating all command dispatch with no injected dependencies makes it impossible to scope, test, or replace individual command families independently. — _severity: high_ — `pub struct CliApp;  // lib.rs:160, no fields` / 27 impl blocks across arch.rs, conventions.rs, demo.rs, domain.rs, dry.rs, dry_fix_runner.rs, file.rs, git.rs, guard.rs, hook.rs, pr.rs, ref_verify.rs, review_v2/mod.rs, semantic_dup/*, signal.rs, telemetry.rs, track/*, verify.rs

---

### cli-composition-god-facade

- `apps/cli-composition/src/lib.rs:160-167` — **ADR 2026-05-27-0110 D2 (god-object avoidance); ADR 2026-06-21-1328** — Restatement with additional evidence: 51 public methods spread across 27 impl files including `review_v2/mod.rs` (1297 lines), `review_v2/run.rs` (1044 lines), and `dry_fix_runner.rs` (600 lines). ADR 2026-06-21-1328 uses the same 51 public methods / 27 files baseline and declares the spirit of ADR 2026-05-27-0110 violated. Required fix: dissolve into bounded-context per-context composer structs. — _severity: high_ — `pub struct CliApp; — 51 public methods across 27 files`

---

### cli-composition-presentation-leak

- `apps/cli-composition/src/pr.rs:38, 67, 73, 84, 209-235` — **ADR 2026-06-21-1328 D1/D3: presentation/output-formatting must move into `cli_driver` primary adapters** — cli_composition production code contains direct output emission (`println!`/`eprintln!`) and JSON response formatting at multiple call sites, including `pr.rs` and `pr/poll.rs`. The `render_outcome` helper in `cmd_outcome.rs` also formats verification output. This output emission and response formatting must not live in the DI wiring layer. — _severity: high_ — `println!("Pushing {} to origin...", ctx.branch); eprintln!("[ERROR] {err}"); render_outcome() in cmd_outcome.rs`

---

### cli-composition-orchestration-leak

- `apps/cli-composition/src/pr/poll.rs:248-396` — **ADR 2026-06-21-1328 D4: multi-step orchestration belongs in usecase application services** — `pr/poll.rs` (lines 248-396) implements a PR review polling loop with retry/timeout/fallback logic (`loop`, `thread::sleep`, deadline tracking, recovery path) directly in cli_composition. Similarly, `dry_fix_runner.rs` (600 lines) implements a multi-step fix cycle and `review_v2/run.rs` (1044 lines) orchestrates a review cycle with CWD management. These are application-level integration concerns, not DI wiring. — _severity: medium_ — `eprintln!("Polling for Codex review on PR #{pr} (interval={interval}s, timeout={timeout}s)..."); loop { ... thread::sleep(Duration::from_secs(delay_seconds)); }`

---

### cli-bin-business-logic

- `apps/cli/src/main.rs:247-294` — **ADR 2026-05-27-0110 D3 / ADR 2026-06-21-1328 D8: cli bin must parse, dispatch, and emit only** — `emit_archived_track_subcommand` performs file I/O (`std::fs::create_dir_all`, `std::fs::OpenOptions`), JSON serialisation (`serde_json::json!`, `serde_json::to_vec`), and wall-clock access (`chrono::Utc::now()`) directly in the cli binary. This telemetry path should be delegated to an infrastructure adapter wired by `cli_composition` and invoked through `cli_driver`; the cli bin should only dispatch to the driver and emit the resulting `CommandOutcome`. — _severity: high_ — `let event = serde_json::json!({ ... "timestamp": chrono::Utc::now().to_rfc3339() }); std::fs::create_dir_all(parent); std::fs::OpenOptions::new().append(true).create(true).open(&path)`

---

### adapter-outside-infrastructure

- `apps/cli-composition/src/track/fixpoint_resolve.rs:84-122` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `FsReviewGateStateAdapter` implements `usecase::fixpoint_resolve::ReviewGateStatePort` directly in cli_composition, performing filesystem review state reads by delegating to `crate::review_v2::approved::check_approved_str`. No corresponding infrastructure implementation exists. — _severity: high_ — `struct FsReviewGateStateAdapter { items_dir: PathBuf }` / `impl ReviewGateStatePort for FsReviewGateStateAdapter { fn review_status(...) }`

- `apps/cli-composition/src/track/fixpoint_resolve.rs:131-165` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `FsRefVerifyGateStateAdapter` implements `usecase::fixpoint_resolve::RefVerifyGateStatePort` inside cli_composition by querying ref-verify gate state through `CliApp::ref_verify_check_approved` (itself a filesystem read). No infrastructure implementation of this port exists. — _severity: high_ — `struct FsRefVerifyGateStateAdapter { items_dir: PathBuf }` / `impl RefVerifyGateStatePort for FsRefVerifyGateStateAdapter { fn ref_verify_status(...) }`

- `apps/cli-composition/src/track/mod.rs:101-113` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `LazyBranchReader` implements `usecase::track_resolution::BranchReaderPort` in cli_composition by wrapping `infrastructure::git_cli::SystemGitRepo`, which already directly implements `BranchReaderPort` in `libs/infrastructure`. This creates a redundant second adapter outside the designated layer (justified in a comment as a UFCS trait-disambiguation workaround, but still a structural violation). — _severity: medium_ — `struct LazyBranchReader { project_root: PathBuf }` / `impl BranchReaderPort for LazyBranchReader { fn current_branch(&self) -> ... { let repo = infrastructure::git_cli::SystemGitRepo::discover_from(...); ... } }`

- `apps/cli-composition/src/dry/tier_telemetry.rs:115-198` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `RecordingDryAgent<A>` is a telemetry-wrapping decorator that implements `usecase::dry_check::DryCheckAgentPort` inside cli_composition. Its `pub(super)` visibility confirms it is wired at runtime, not test-only. Even decorator adapters must reside in libs/infrastructure. — _severity: medium_ — `pub(super) struct RecordingDryAgent<A> { inner: A, ... }` / `impl<A: DryCheckAgentPort> DryCheckAgentPort for RecordingDryAgent<A> { fn judge(...) }`

- `apps/cli-composition/src/dry/persistent_index.rs:278-316` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `NullInsertIndexProxy` implements `usecase::semantic_dup::SemanticIndexPort` inside cli_composition, wrapping `LanceDbSemanticIndexAdapter` and suppressing inserts while delegating search and delete. It also manages `PersistentIndexLock` (filesystem-level LanceDB state). This port-implementing decorator must live in libs/infrastructure. — _severity: medium_ — `pub(super) struct NullInsertIndexProxy { inner: Arc<LanceDbSemanticIndexAdapter>, _cache_lock: PersistentIndexLock }` / `impl SemanticIndexPort for NullInsertIndexProxy { fn insert(...) { Ok(()) } fn search(...) { self.inner.search(...) } }`

- `apps/cli-composition/src/semantic_dup/measure_quality.rs:23-52` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `NoopSemanticIndexPort` implements `usecase::semantic_dup::SemanticIndexPort` as a null-object adapter in production scope (wired via `Arc::new` at line 84). Null-object adapters satisfying port contracts belong in libs/infrastructure as companion stubs alongside real adapters, not in the composition root. — _severity: low_ — `struct NoopSemanticIndexPort;` / `impl SemanticIndexPort for NoopSemanticIndexPort { fn insert(...) { Ok(()) } fn search(...) { Ok(Vec::new()) } }`

- `apps/cli-composition/src/track/fixpoint_resolve.rs:49-59` — **hexagonal-architecture.md Adapter Rules: adapters belong in infrastructure only** — `NoOpDryApprovalService` implements `usecase::dry_check::DryCheckApprovalService` in cli_composition as a no-op stub used when dry-check is disabled. Any struct implementing a port trait is an adapter; it should reside in libs/infrastructure (or as a provided test-double from usecase if it requires no infrastructure access). — _severity: low_ — `struct NoOpDryApprovalService;` / `impl DryCheckApprovalService for NoOpDryApprovalService { fn check_approved(...) -> Result<DryCheckApprovalVerdict, ...> { Ok(DryCheckApprovalVerdict::Approved) } }`

---

## Cross-reference to ADRs

### ADR 2026-05-27-0110 — cli_composition dedicated crate; CliApp thin facade; typed CompositionError

Maps to violation clusters:
- **god-facade**: CliApp is explicitly a god-object contradicting the "THIN" requirement (D1/D2). Addressed in design by ADR 2026-06-21-1328 (dissolve into per-context composers), but not yet implemented.
- **stringly-typed-error-boundary**: D2 specified `Result<_, CompositionError>`; the drift to `Result<_, String>` is confirmed pervasive. Addressed in design by ADR 2026-06-21-1328 D2, but not yet implemented.
- **cli-bin-business-logic**: D3 required the cli bin to depend on cli_composition only. `emit_archived_track_subcommand` violates this and is not yet addressed by any implemented fix.
- **presentation-in-composition-root** and **pre-formatted-stdout-stderr**: indirectly targeted by the "THIN" requirement but not explicitly named in 0110; fully addressed in design by 2026-06-21-1328.

### ADR 2026-06-21-1328 — cli_composition responsibility purification

This is the primary governing ADR for the majority of violations. Its decisions map to clusters as follows:

| Decision | Cluster addressed | Status |
|---|---|---|
| D1/D3: move rendering into `cli_driver` primary adapters | presentation-in-composition-root, json-output-assembly-in-composition-root, pre-formatted-stdout-stderr, cli-composition-presentation-leak | Proposed, not implemented |
| D2: make `cli_composition` wire-only and introduce typed `CompositionError` | stringly-typed-error-boundary, cli-composition-stringly-typed-error | Proposed, not implemented |
| D2/D6(e): dissolve god-facade into per-context composition roots and drivers | god-facade, cli-composition-god-facade | Proposed, not implemented |
| D4: move orchestration interactors to `usecase` | multi-step-orchestration, cli-composition-orchestration-leak | Proposed, not implemented |
| D7: migrate port-implementing adapters to `libs/infrastructure` | adapter-outside-infrastructure | Proposed, not implemented |
| D8: remove direct telemetry I/O from the cli bin | cli-bin-business-logic | Proposed, not implemented |

### ADR 2026-04-30-0848 — cli must not reference domain directly; usecase exposes DTOs

Maps to:
- **cli-bin-business-logic** (`apps/cli/src/main.rs:247-294`): the cli binary reaches for `chrono`, `serde_json`, and `std::fs` directly, bypassing cli_composition entirely for the telemetry emission path. Open and not addressed.

### ADR 2026-04-14-1531 — domain must be serde-free (CN-05)

No confirmed violation in the confirmed finding list. Not directly implicated by any finding above.

### ADR 2026-06-21-1420

Not directly referenced by the confirmed findings; governs a separate concern from the clusters above.

---

## Prioritized Remediation

The ordering below reflects severity, architectural leverage (fixing one item unblocks or simplifies others), and alignment with ADR 2026-06-21-1328's decisions.

1. **Introduce typed `CompositionError` (ADR 1328 D2) — high severity, maximum leverage**
   Replace every stringly typed public `CliApp` command boundary across the 51-method surface with `Result<CommandOutcome, CompositionError>` or an equivalent typed composition result, define `CompositionError` with typed variants (I/O failure, codec failure, interactor failure, etc.), and replace all `Err(format!(...))` / `.map_err(|e| format!(...))` sites. This is a prerequisite for dissolving the god-facade (step 4) and for the cli bin to meaningfully handle typed errors from per-context composers. Corresponds to stringly-typed-error-boundary violations at `domain.rs:24`, `dry.rs:240`, `verify.rs:115`, `git.rs:15`, `ref_verify.rs:89`, and `arch.rs:12-49`.

2. **Move rendering into `cli_driver` primary adapters (ADR 1328 D1/D3) — high severity, broadest surface**
   Create `apps/cli-driver` (`cli_driver`) with layer-internal render modules and migrate all string-formatting functions and helpers out of cli_composition: `render_outcome` (`cmd_outcome.rs:12-33`), `merge_outcomes` (`signal.rs:23-47`), `render_skip` (`verify.rs:8-11`), `render_review_results_str` (`review_v2/results.rs:24-244`), `format_review_summary` (`pr/poll.rs:514-547`), `dry_results` field-level formatting (`dry.rs:437-492`), and all `format!`-to-stdout/stderr sites across `review_v2/mod.rs`, `git.rs`, `pr.rs`, `track/mod.rs`, `domain.rs`, and `ref_verify.rs`. JSON output assembly (`guard.rs:42-47`, `hook.rs:241-248`, `track/ops.rs:177-192`, `semantic_dup/measure_quality.rs:92-106`) belongs in `cli_driver` as usecase-output-to-`CommandOutcome` rendering, behind typed DTO boundaries. `println!`/`eprintln!` calls in `pr.rs` and `pr/poll.rs` must be removed from cli_composition entirely. This addresses the largest count of violations across the most files.

3. **Move multi-step orchestration to `usecase` interactors (ADR 1328 D4) — high severity, architectural correctness**
   Extract the following sequences from cli_composition into usecase application service types: `signal_check_gate` (signal.rs:437-567, four-chain sequencing + aggregation); `signal_calc_spec_adr` (signal.rs:167-194, read→decode→evaluate→set→write cycle); `build_current_fragment_refs` (fixpoint_resolve.rs:214-291, diff-fragment pipeline); the `if usecase_dry_config.enabled` orchestration block (fixpoint_resolve.rs:428-518); the PR review polling loop (pr/poll.rs:248-396); and the `dry_fix_runner.rs` / `review_v2/run.rs` multi-step cycles. cli_composition should retain only the adapter wiring necessary to instantiate and call a single interactor method per command. The post-interactor metric aggregation in `dry_write` (dry.rs:318-425) and the `fragment_ref_of` derivation loop in `dry_check_approved` (dry.rs:560-592) should be absorbed into the interactor return types.

4. **Dissolve `CliApp` god-facade into per-context composition roots and drivers (ADR 1328 D2/D6(e)) — high severity, enables testability**
   Replace `pub struct CliApp;` with bounded-context composer structs (e.g., `VerifyComposer`, `DryCheckComposer`, `TrackComposer`, `PrComposer`, `ReviewComposer`, `SignalComposer`) that carry injected dependencies as fields. Each composer implements only the methods for its bounded context. The 27 `impl CliApp` files become 27 separate composer types or groups. This makes per-context construction, substitution, and unit testing possible. Steps 1-3 above must be completed first to reduce each composer to genuine DI wiring.

5. **Migrate port-implementing adapters to `libs/infrastructure` (hexagonal-architecture.md) — high severity for production adapters, medium for null-objects**
   Move the two high-severity adapters (`FsReviewGateStateAdapter` at fixpoint_resolve.rs:84-122 and `FsRefVerifyGateStateAdapter` at fixpoint_resolve.rs:131-165) to `libs/infrastructure` alongside their peer filesystem adapters. Move `RecordingDryAgent` (tier_telemetry.rs:115-198) and `NullInsertIndexProxy` (persistent_index.rs:278-316) to infrastructure. The null-object stubs (`NoopSemanticIndexPort`, `NoOpDryApprovalService`) may alternatively be re-homed as test-doubles in the relevant usecase crate if they require no infrastructure access. The `LazyBranchReader` UFCS workaround (track/mod.rs:101-113) should be eliminated by fixing the disambiguation at the call site rather than introducing a second adapter.

6. **Fix `emit_archived_track_subcommand` in the cli bin (ADR 0848 D3) — high severity, isolated fix**
   Move the telemetry file-write logic (file I/O, JSON serialisation, wall-clock access) from `apps/cli/src/main.rs:247-294` into an infrastructure adapter wired by `cli_composition` and invoked through `cli_driver`. The cli binary handler should dispatch to the driver and emit the `CommandOutcome`. This is the only cli-bin-level violation but directly contradicts ADR 2026-04-30-0848 D3 and ADR 2026-06-21-1328 D8.
