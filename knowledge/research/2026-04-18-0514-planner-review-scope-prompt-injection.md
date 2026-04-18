# Planner Output: review-scope-prompt-injection

Source: `planner` capability (Claude Opus) invocation on 2026-04-18 for track `review-scope-prompt-injection-2026-04-18`.

Referenced ADR: `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` (SSoT).

## Difficulty estimate

**M** — 7-8 files touched, range limited. `ReviewScopeConfig::new` signature change cascades across domain → infrastructure → CLI and requires an ordering rework in `run_execute_codex_local` (base_prompt is currently built before `ReviewScopeConfig` is loaded).

## Task decomposition

See `track/items/review-scope-prompt-injection-2026-04-18/metadata.json` — T001-T008 derived from this output.

## Layer-by-layer change matrix

| Layer | File | Change | New types / functions |
|-------|------|--------|-----------------------|
| domain | `libs/domain/src/review_v2/scope_config.rs` | Add `ScopeEntry`; change `scopes` type; extend `ReviewScopeConfig::new` signature; rewrite `classify` / `contains_scope` / `all_scope_names` internals | `ScopeEntry`, `ReviewScopeConfig::briefing_file_for_scope(&self, &ScopeName) -> Option<&str>` |
| domain | `libs/domain/src/review_v2/mod.rs` | `pub use` adjustment — `ScopeEntry` stays crate-private, only `briefing_file_for_scope` is pub via `ReviewScopeConfig` | — |
| infrastructure | `libs/infrastructure/src/review_v2/scope_config_loader.rs` | Add `briefing_file: Option<String>` to `GroupEntry`; update entries assembly; add loader tests | `GroupEntry.briefing_file` |
| cli | `apps/cli/src/commands/review/codex_local.rs` | Rework `run_execute_codex_local` ordering; add `append_scope_briefing_reference` pure function | `append_scope_briefing_reference(prompt: &mut String, scope: &ScopeName, config: &ReviewScopeConfig)` |
| cli | `apps/cli/src/commands/review/compose_v2.rs` | Either add `scope_config` to `ReviewV2CompositionWithCodex` or add `build_scope_config_only` helper (decision at implementation time) | — |
| config | `track/review-scope.json` | Add `plan-artifacts` group (`patterns` + `briefing_file`) | — |
| docs | `track/review-prompts/plan-artifacts.md` (new) | Severity policy body | — |
| agent | `.claude/agents/review-fix-lead.md` | Add Scope-specific severity policy Read instruction before `## Workflow` | — |
| command | `.claude/commands/track/review.md` | Step 2b: document scope-briefing auto-injection | — |

## Risks / edge cases

### 1. `ReviewScopeConfig::new` signature cascade

Called from `load_v2_scope_config` (infrastructure) and direct constructions in `#[cfg(test)]` blocks within `libs/domain/src/review_v2/scope_config.rs`, `libs/domain/src/review_v2/tests.rs`, and `libs/usecase/src/review_v2/tests.rs:155`. The usecase-layer test is cross-layer: it builds `ReviewScopeConfig` directly with the 2-tuple form. All three test modules must be updated to the new 3-tuple form in T001 to avoid CI failure when the domain signature changes.

### 2. `run_execute_codex_local` ordering problem (Q-IMPL-01)

Currently:

```
Step 2 → build_base_prompt(args)                              // scope_config unavailable
Step 3 → CodexReviewer::new(..., base_prompt)
Step 4 → build_review_v2_with_reviewer(...)                   // loads scope_config internally
```

Three resolution options:

- **(A)** Add `CodexReviewer::with_scope_briefing(path: &str) -> Self` builder, chain after composition. Cleanest typed API.
- **(B)** Add `load_scope_config_only(track_id, items_dir)` helper, pre-load before `build_base_prompt`, inject via `append_scope_briefing_reference` before passing to `CodexReviewer::new`. Fewest moving parts.
- **(C)** Add `scope_config: ReviewScopeConfig` field to `ReviewV2CompositionWithCodex`, mutate reviewer via builder after construction. Requires `ReviewScopeConfig: Clone`.

Decision deferred to implementation time. Record in `verification.md`.

### 3. `deny_unknown_fields` preserved

Adding `briefing_file: Option<String>` with `#[serde(default)]` keeps `deny_unknown_fields` intact. Typo test (`briefng_file` → reject) guards regression.

### 4. No existence check on `briefing_file` (ADR D4 consequence)

Per ADR D4, loader / composer do not verify the file exists. Reviewer's Read tool surfaces a missing file at runtime. Mitigation (ADR Open Q3, CI lint) is deferred to a follow-up track.

### 5. `ScopeEntry` visibility

`ScopeEntry` stays crate-private (not exported from `mod.rs`). Only `ReviewScopeConfig::briefing_file_for_scope` is public. Prevents leaky abstraction.

### 6. `plan-artifacts` narrows `Other` scope

Files under `track/items/<track-id>/**`, `knowledge/adr/**`, and `knowledge/research/**` were originally in `Other` before this track's bootstrap was applied. The bootstrap (`track/review-scope.json` carrying a pragmatic `plan-artifacts` entry with literal `**` patterns) already moves them to `plan-artifacts` during this review cycle. T006 upgrades the bootstrap pattern to the final `<track-id>` placeholder form. After T001+T006, the final form is active. Existing tracks' `review.json` will see `other` scope hash become `StaleHash` (normal behavior, new scope partitioning).

### 7. `review-scope.json` itself lives in `harness-policy` scope

Current `harness-policy` patterns include `track/review-scope.json`, so T006 commits are reviewed under `harness-policy` scope (plus `plan-artifacts` for track-internal docs post-T006). Expected.

### 8. `build_v2_shared` return plumbing

Currently returns `(ReviewScopeConfig, FsReviewStore, FsCommitHashStore, CommitHash)` consumed by `ReviewCycle::new`. If option (C) chosen, `ReviewScopeConfig` must be cloned to keep both in `ReviewCycle` and in the composition struct. Add `#[derive(Clone)]` to `ReviewScopeConfig` (globset `GlobMatcher` is `Clone`).

## ADR integrity check

- **D1** (`briefing_file` optional field): ✅ T002
- **D2** (`ReviewScopeConfig` / domain type change): ✅ T001
- **D3** (`plan-artifacts` scope + severity policy md): ✅ T006 / T007
- **D4** (reference-style injection): ✅ T003 (no fs::read, path-only)
- **D5** (`Other` exempt): ✅ API-level guarantee via `briefing_file_for_scope(ScopeName::Other) -> None`

ADR Consequences § Bad/Risk items all addressed:

- review-fix-lead prompt update → T004
- deny_unknown_fields typo reject → T002 test
- Open Q3 CI lint → deferred (separate track)
- schema revision documented in-track

## Type design principles (.claude/rules/04-coding-principles.md)

### `ScopeEntry` — plain struct

Not a state machine, no variant-dependent data: plain struct is correct.

```rust
// Clone is only needed if option (C) is chosen for the T003 ordering fix.
// Decision deferred to implementation time (see §Ordering problem fix options).
#[derive(Debug)]
struct ScopeEntry {
    matchers: Vec<GlobMatcher>,
    briefing_file: Option<String>,
}
```

### `briefing_file: Option<String>` not newtype

ADR D4 explicitly chose "path string as-is, validation delegated to reviewer sandbox". Adding a `BriefingPath` newtype with validation would contradict D4 intent. `Option<String>` retained.

### `ReviewScopeConfig::new` signature

`Vec<(String, Vec<String>, Option<String>)>` 3-tuple accepted as-is (introducing a named `ScopeEntryInput` intermediate would leak from loader internals without strong benefit).

## Canonical Blocks

### ScopeEntry

```rust
/// One named scope's classification matchers and optional briefing file.
///
/// This struct is crate-private. Access briefing information through
/// `ReviewScopeConfig::briefing_file_for_scope`.
// Note: add `Clone` to this derive if option (C) is chosen for the ordering fix (T003),
// which requires ReviewScopeConfig to be Clone. Decision deferred to implementation time.
#[derive(Debug)]
struct ScopeEntry {
    matchers: Vec<GlobMatcher>,
    /// Workspace-relative path to a scope-specific briefing markdown file.
    /// `None` means no scope-specific briefing (the reviewer uses the main briefing only).
    briefing_file: Option<String>,
}
```

### GroupEntry (serde)

```rust
/// Serde helper for a single group entry in review-scope.json.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupEntry {
    patterns: Vec<String>,
    /// Optional workspace-relative path to a scope-specific briefing file.
    /// When present, the CLI briefing composer appends a reference line so
    /// the reviewer reads the file via its Read tool.
    #[serde(default)]
    briefing_file: Option<String>,
}
```

### `ReviewScopeConfig::new` revised signature

```rust
pub fn new(
    track_id: &TrackId,
    entries: Vec<(String, Vec<String>, Option<String>)>,
    operational: Vec<String>,
    other_track: Vec<String>,
) -> Result<Self, ScopeConfigError> {
    // ...
    for (name, patterns, briefing_file) in entries {
        let scope_name = MainScopeName::new(name)?;
        let matchers = patterns
            .iter()
            .map(|pat| {
                // expand_track_id MUST be called here (ADR D3 / T001 requirement)
                // so that `<track-id>` placeholders in group patterns are resolved
                // to the current track before the glob is compiled.
                let expanded = expand_track_id(pat, track_id);
                compile_glob(&expanded).map_err(|source| ScopeConfigError::InvalidPattern {
                    pattern: expanded.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        scopes.insert(scope_name, ScopeEntry { matchers, briefing_file });
    }
    // ... rest unchanged
}
```

### `briefing_file_for_scope` accessor

```rust
impl ReviewScopeConfig {
    /// Returns the workspace-relative path to the scope-specific briefing file,
    /// or `None` if no briefing is configured for this scope.
    ///
    /// Always returns `None` for `ScopeName::Other` (reserved scope has no briefing).
    #[must_use]
    pub fn briefing_file_for_scope(&self, scope: &ScopeName) -> Option<&str> {
        match scope {
            ScopeName::Other => None,
            ScopeName::Main(name) => self
                .scopes
                .get(name)
                .and_then(|entry| entry.briefing_file.as_deref()),
        }
    }
}
```

### briefing composer injection

```rust
/// Appends a scope-specific severity policy reference section to `prompt`
/// if the given scope has a `briefing_file` configured.
///
/// The reviewer is expected to use its Read tool to fetch the file content.
/// No I/O is performed here — only string manipulation.
fn append_scope_briefing_reference(
    prompt: &mut String,
    scope: &domain::review_v2::ScopeName,
    scope_config: &domain::review_v2::ReviewScopeConfig,
) {
    if let Some(briefing_path) = scope_config.briefing_file_for_scope(scope) {
        // Format MUST match the ADR D4 Japanese example block exactly.
        // See knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md §D4
        prompt.push_str("\n\n## Scope-specific severity policy\n\n");
        prompt.push_str(&format!(
            "このレビューの scope は `{scope}` である。\
             以下の scope 固有 severity policy を **必ず先に Read ツールで読み込み**、\
             その方針に従って findings を選別すること:\n\n\
             - `{briefing_path}`",
        ));
    }
}
```

### `load_v2_scope_config` revised entries assembly

```rust
let entries: Vec<(String, Vec<String>, Option<String>)> = doc
    .groups
    .into_iter()
    .map(|(name, entry)| (name, entry.patterns, entry.briefing_file))
    .collect();

Ok(ReviewScopeConfig::new(track_id, entries, doc.review_operational, doc.other_track)?)
```

### plan-artifacts.md (initial body)

```markdown
# Plan Artifact Review: Severity Policy

This policy applies when the review scope is `plan-artifacts`.
Files in scope: `track/items/<track-id>/` (plan.md, spec.md, spec.json,
metadata.json, verification.md, reports/** etc.), newly authored or
revised ADRs (`knowledge/adr/**`), and planner research notes for this
track (`knowledge/research/**`).

## What to report

Report findings ONLY for the following categories:

- **factual error**: a claim that is objectively incorrect (non-existent CLI
  command, file path, ADR number, or crate that does not exist)
- **contradiction**: two or more passages in the same or related files that
  assert conflicting facts
- **broken reference**: a `[source: ...]`, `[tasks: ...]`, or cross-document
  link whose target does not exist
- **infeasibility**: a `tasks[]` dependency order or workload estimate that
  makes the plan physically unexecutable
- **timestamp inconsistency**: `updated_at` or `commit_hash` fields that
  contradict each other or the git log

## What NOT to report

- Wording nits (tone, verbosity, word choice preference)
- English/Japanese mixed writing (unless an explicit style rule is violated)
- Alternative design suggestions (the planning gate has already closed)
- Formatting preferences (heading depth, bullet style)
```

### review-fix-lead.md insert (before Workflow)

```markdown
## Scope-specific severity policy

If the main briefing contains a `## Scope-specific severity policy` section,
you MUST read the file listed there using your `Read` tool **before starting
the review**. That file defines which finding categories to report and which
to skip for this scope. Applying the wrong severity filter is
the primary cause of over-long review loops (28-round history).

Do not skip this step even if the briefing path appears to be a known file.
Always read it fresh — the policy file may have been updated since the last
review session.
```
