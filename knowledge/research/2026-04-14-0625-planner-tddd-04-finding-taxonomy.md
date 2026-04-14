# Planner Output — tddd-04-finding-taxonomy-cleanup-2026-04-14

> **Provider**: Claude Opus (subagent_type: Plan)
> **Saved at**: 2026-04-14 06:25 UTC
> **Briefing**: `tmp/planner/tddd-04-finding-taxonomy-briefing.md`
> **Track**: `tddd-04-finding-taxonomy-cleanup-2026-04-14`

---

## Section 1 — Design Decision Summary

**Option B (full rename) is the correct choice.** Option A violates the domain-purity constraint codified in `knowledge/conventions/hexagonal-architecture.md` and reinforced by ADR 2026-03-25-0000 (the diff-scope ADR explicitly rejects moving serde into the domain). Option C loses the non-empty-message invariant enforced by `Finding::new()` → `FindingError::EmptyMessage`, which `convert_findings_to_domain` relies on via `filter_map` to silently discard malformed reviewer output — the invariant is load-bearing. Option B preserves the hexagonal separation (domain validated type vs. usecase DTO), eliminates the HashMap collision in `code_profile_builder.rs`, and follows the established cascade-rename precedent set by the `TraitPort` → `SecondaryPort` rename in tddd-02 (~5 files, ~39 occurrences, no backward-compat aliases). Both proposed new names (`ReviewerFinding` for type 1, `VerifyFinding` for type 2) are confirmed unique last-segment names across the workspace (grep shows zero current uses of either name). The enum-first rule from `.claude/rules/04-coding-principles.md` does not bear on naming, but the "make illegal states unrepresentable" principle confirms keeping the validated newtype separate from the serde DTO.

**New names: `domain::review_v2::Finding` → `ReviewerFinding`; `domain::verify::Finding` → `VerifyFinding`.**

Rationale for `ReviewerFinding`: the type lives in the `review_v2` module, represents the reviewer's verdict output, and is the domain-validated counterpart to `usecase::review_workflow::ReviewFinding` (the serde DTO). The `Reviewer` prefix aligns with the `CodexReviewer` adapter name and makes the pairing explicit: `ReviewFinding` (DTO, wire) ↔ `ReviewerFinding` (domain, validated). `CodexFinding` is too implementation-specific (ties to Codex, not to the domain concept). `ReviewRemark` and `CritiqueItem` are less idiomatic in this Rust codebase.

Rationale for `VerifyFinding`: the type lives in `domain::verify`, is produced by `sotp verify` subcommands, and has a `Severity` enum (`Info`/`Warning`/`Error`). `Diagnostic` is too generic and could collide with compiler/linter terminology. `VerificationIssue` is verbose. `CheckResult` implies a pass/fail struct rather than a per-finding record. `VerifyFinding` mirrors the module path directly and is what a reader would expect.

---

## Section 2 — Rename Table

| Old symbol | New symbol | File (primary) | Cascade notes |
|---|---|---|---|
| `struct Finding` (review_v2) | `struct ReviewerFinding` | `libs/domain/src/review_v2/types.rs:210` | Inner type of `NonEmptyReviewerFindings` and `Verdict`/`FastVerdict` variants |
| `NonEmptyFindings` | `NonEmptyReviewerFindings` | `libs/domain/src/review_v2/types.rs:265` | Contains `Vec<ReviewerFinding>`; used in `Verdict::FindingsRemain` and `FastVerdict::FindingsRemain` |
| `FindingError` | `ReviewerFindingError` | `libs/domain/src/review_v2/error.rs:23` | Returned by `ReviewerFinding::new()` |
| `FindingError::EmptyMessage` | `ReviewerFindingError::EmptyMessage` | `libs/domain/src/review_v2/error.rs:25` | Error variant name |
| `VerdictError::EmptyFindings` | `VerdictError::EmptyFindings` | `libs/domain/src/review_v2/error.rs:18` | Variant name is semantically correct; the containing enum is unchanged; **keep** |
| `Verdict::findings_remain(findings: Vec<Finding>)` | `Verdict::findings_remain(findings: Vec<ReviewerFinding>)` | `libs/domain/src/review_v2/types.rs:305` | Signature update only; function name unchanged |
| `FastVerdict::findings_remain(findings: Vec<Finding>)` | `FastVerdict::findings_remain(findings: Vec<ReviewerFinding>)` | `libs/domain/src/review_v2/types.rs:334` | Same |
| `NonEmptyFindings::new(findings: Vec<Finding>)` | `NonEmptyReviewerFindings::new(findings: Vec<ReviewerFinding>)` | `libs/domain/src/review_v2/types.rs:272` | Constructor update |
| `NonEmptyFindings::as_slice() -> &[Finding]` | `NonEmptyReviewerFindings::as_slice() -> &[ReviewerFinding]` | `libs/domain/src/review_v2/types.rs:279` | Return type update |
| `NonEmptyFindings::into_vec() -> Vec<Finding>` | `NonEmptyReviewerFindings::into_vec() -> Vec<ReviewerFinding>` | `libs/domain/src/review_v2/types.rs:283` | Return type update |
| `pub use … Finding` (review_v2/mod.rs) | `pub use … ReviewerFinding` | `libs/domain/src/review_v2/mod.rs:22` | Re-export name |
| `pub use … FindingError` (review_v2/mod.rs) | `pub use … ReviewerFindingError` | `libs/domain/src/review_v2/mod.rs:16` | Re-export name |
| `pub use … NonEmptyFindings` (review_v2/mod.rs) | `pub use … NonEmptyReviewerFindings` | `libs/domain/src/review_v2/mod.rs:22` | Re-export name |
| `use domain::review_v2::{…, Finding, …}` | `use domain::review_v2::{…, ReviewerFinding, …}` | `libs/infrastructure/src/review_v2/codex_reviewer.rs:10` | Import update |
| `fn convert_findings_to_domain(…) -> Vec<Finding>` | `fn convert_findings_to_domain(…) -> Vec<ReviewerFinding>` | `libs/infrastructure/src/review_v2/codex_reviewer.rs:225` | Function body + return type; name kept |
| `Finding::new(…)` call in `convert_findings_to_domain` | `ReviewerFinding::new(…)` | `libs/infrastructure/src/review_v2/codex_reviewer.rs:231` | Constructor call |
| `f: &domain::review_v2::Finding` in `finding_to_review_finding` | `f: &domain::review_v2::ReviewerFinding` | `apps/cli/src/commands/review/codex_local.rs:155` | Parameter type |
| Test helpers `fn finding(msg)` / `fn finding_full()` | Return type `ReviewerFinding` | `libs/domain/src/review_v2/tests.rs:10,14` | Helper return type |
| `Finding::new(…)` in test helpers | `ReviewerFinding::new(…)` | `libs/domain/src/review_v2/tests.rs:11,15` | Constructor |
| `use super::error::{…, FindingError, …}` in tests | `use super::error::{…, ReviewerFindingError, …}` | `libs/domain/src/review_v2/tests.rs:3` | Import |
| `Err(FindingError::EmptyMessage)` in test assertions | `Err(ReviewerFindingError::EmptyMessage)` | `libs/domain/src/review_v2/tests.rs:228,235` | Pattern match |
| Doc comment `Finding` mentions in `# Errors` | Update to `ReviewerFinding` | `libs/domain/src/review_v2/types.rs:206,219,222` | Load-bearing doc (`# Errors` section names the error type) |
| `/// Errors from \`Finding::new\` construction.` | `/// Errors from \`ReviewerFinding::new\` construction.` | `libs/domain/src/review_v2/error.rs:21` | Load-bearing doc |
| `/// A non-empty collection of findings.` / `Vec<Finding>` mention | Update inner type ref | `libs/domain/src/review_v2/types.rs:260–265` | Doc prose reference |
| `/// Converts \`usecase::review_workflow::ReviewFinding\` slice to domain \`Finding\` vec.` | `… to domain \`ReviewerFinding\` vec.` | `libs/infrastructure/src/review_v2/codex_reviewer.rs:224` | Load-bearing doc |
| `/// Converts a domain \`Finding\` to a \`ReviewFinding\` for JSON serialization.` | `… domain \`ReviewerFinding\` …` | `apps/cli/src/commands/review/codex_local.rs:153` | Load-bearing doc |
| `struct Finding` (verify) | `struct VerifyFinding` | `libs/domain/src/verify.rs:28` | Inner element of `VerifyOutcome` |
| `Finding::new(severity, message)` | `VerifyFinding::new(severity, message)` | `libs/domain/src/verify.rs:35` | Constructor; same signature |
| `Finding::error(message)` | `VerifyFinding::error(message)` | `libs/domain/src/verify.rs:40` | Convenience constructor |
| `Finding::warning(message)` | `VerifyFinding::warning(message)` | `libs/domain/src/verify.rs:45` | Convenience constructor |
| `impl fmt::Display for Finding` | `impl fmt::Display for VerifyFinding` | `libs/domain/src/verify.rs:60` | Impl block |
| `VerifyOutcome { findings: Vec<Finding> }` | `VerifyOutcome { findings: Vec<VerifyFinding> }` | `libs/domain/src/verify.rs:69` | Field type |
| `VerifyOutcome::from_findings(Vec<Finding>)` | `VerifyOutcome::from_findings(Vec<VerifyFinding>)` | `libs/domain/src/verify.rs:79` | Parameter type |
| `VerifyOutcome::findings() -> &[Finding]` | `VerifyOutcome::findings() -> &[VerifyFinding]` | `libs/domain/src/verify.rs:94` | Return type |
| `VerifyOutcome::add(finding: Finding)` | `VerifyOutcome::add(finding: VerifyFinding)` | `libs/domain/src/verify.rs:99` | Parameter type |
| Tests in `libs/domain/src/verify.rs` using `Finding::error`, `Finding::warning` | `VerifyFinding::error`, `VerifyFinding::warning` | `libs/domain/src/verify.rs:146,155,162–165,179` | 9 call sites in test module |
| `use domain::verify::{Finding, …}` in infra verify files | `use domain::verify::{VerifyFinding, …}` | All `libs/infrastructure/src/verify/*.rs` files | All `Finding::error / warning` calls become `VerifyFinding::…` |
| `use domain::verify::{Finding, …}` in `libs/usecase/src/merge_gate.rs` | `use domain::verify::{VerifyFinding, …}` | `libs/usecase/src/merge_gate.rs` | All `Finding::error / warning` calls |
| `use domain::verify::{Finding, …}` in `libs/usecase/src/task_completion.rs` | `use domain::verify::{VerifyFinding, …}` | `libs/usecase/src/task_completion.rs` | All `Finding::error / warning` calls |
| `use crate::verify::{Finding, …}` in `libs/domain/src/tddd/consistency.rs` | `use crate::verify::{VerifyFinding, …}` | `libs/domain/src/tddd/consistency.rs` | All constructor calls |
| `use crate::verify::{Finding, …}` in `libs/domain/src/spec.rs` | `use crate::verify::{VerifyFinding, …}` | `libs/domain/src/spec.rs` | All constructor calls |
| `domain::verify::Finding::error(…)` calls in `apps/cli/src/commands/verify.rs` | `domain::verify::VerifyFinding::error(…)` | `apps/cli/src/commands/verify.rs` | Fully-qualified calls + return type annotation |
| Doc comment `/// A single verification finding.` | `VerifyFinding` | `libs/domain/src/verify.rs:26` | Load-bearing doc update |
| `knowledge/conventions/source-attribution.md` prose mention of `Finding::warning` | Update to `VerifyFinding::warning` | `knowledge/conventions/source-attribution.md:29` | Documentation prose |

**Symbols explicitly confirmed NOT needing rename:**

- `VerdictError::EmptyFindings` — variant name semantically correct; keep as-is.
- `Verdict`, `FastVerdict`, `VerifyOutcome`, `Severity` — unchanged types.
- `REVIEW_OUTPUT_SCHEMA_JSON` — the JSON `$defs/finding` key is a JSON schema identifier, not a Rust type name; no update needed.
- `usecase::review_workflow::ReviewFinding` — already distinct; unchanged.
- `usecase::pr_review::PrReviewFinding` — already distinct; unchanged.
- `domain::auto_phase::FindingSeverity` — different concept; `Finding` here is an adjective in a compound word; **keep** (unrelated type).
- Historical track documentation files in `track/items/tddd-02-*/` — do not retroactively rename; acknowledge in the ADR as historical record.
- **Exception**: `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` is the **live catalogue source** referenced from `architecture-rules.json`. It is NOT a frozen historical artifact and IS updated by T006 to remove the `"Finding"` reference entry and add three `declare` entries. (Note added 2026-04-14: this clarification was needed after full-model review round 2 surfaced the ambiguity.)

---

## Section 3 — Data-Flow Diagram

```mermaid
flowchart TD
    subgraph review_flow["Codex local review flow"]
        J["reviewer JSON output (wire format)"]
        J -->|serde_json from_str| DTO["usecase::review_workflow::ReviewFinding (Serialize + Deserialize DTO)"]
        DTO -->|convert_findings_to_domain filter_map| DM["domain::review_v2::ReviewerFinding (non-empty message invariant)"]
        DM -->|NonEmptyReviewerFindings::new| NEF["NonEmptyReviewerFindings"]
        NEF -->|Verdict::findings_remain| VRD["Verdict::FindingsRemain"]
        VRD --> CALLER["usecase / CLI caller"]
    end

    subgraph emit_flow["CLI emit path (codex-local)"]
        FV["FastVerdict::FindingsRemain(NonEmptyReviewerFindings)"]
        FV -->|as_slice iter + finding_to_review_finding| DTO2["usecase::review_workflow::ReviewFinding"]
        DTO2 -->|serde_json to_string| STDOUT["JSON stdout ReviewFinalPayload"]
    end

    subgraph verify_flow["sotp verify flow"]
        CLI_CMD["sotp verify subcommand"]
        CLI_CMD -->|VerifyFinding::error or warning| VF["domain::verify::VerifyFinding (Severity + message)"]
        VF -->|VerifyOutcome::from_findings or add or merge| VO["domain::verify::VerifyOutcome (Vec VerifyFinding)"]
        VO -->|returned to CLI| CLI_OUT["CLI: render + ExitCode"]
    end
```

---

## Section 4 — Task List

> **Note (track materialization 2026-04-14):** This planner output defined 8 tasks
> (T001-T008), with T007 for ADR writing and T008 for the CI gate. During track
> creation, the ADR (`knowledge/adr/2026-04-14-0625-finding-taxonomy-cleanup.md`) and
> ADR 0002 §3.B amendment were completed in the planning phase (not as an implementation
> task), so T007 was collapsed: the planning-phase T007 (ADR writing) is omitted and
> the planning-phase T008 (CI gate) becomes track T007. The canonical task list and
> ordering are in `metadata.json` (7 tasks, T001-T007), which is the SSoT.

```
T001 — rename domain::verify::Finding to VerifyFinding (cascade across verify.rs + domain consumers)
  Files:
    libs/domain/src/verify.rs
    libs/domain/src/tddd/consistency.rs
    libs/domain/src/spec.rs
  Verification:
    cargo test -p domain (expect pass)
    grep verify::Finding\\b libs/ apps/ (expect zero)
    cargo clippy -p domain (expect clean)

T002 — rename usecase consumers of domain::verify::Finding to VerifyFinding
  Files:
    libs/usecase/src/merge_gate.rs
    libs/usecase/src/task_completion.rs
  Verification:
    cargo test -p usecase (expect pass)
    grep verify::Finding\\b libs/usecase/ (expect zero)
    cargo clippy -p usecase (expect clean)

T003 — rename infra + CLI consumers of domain::verify::Finding to VerifyFinding
  Files:
    libs/infrastructure/src/verify/architecture_rules.rs
    libs/infrastructure/src/verify/canonical_modules.rs
    libs/infrastructure/src/verify/convention_docs.rs
    libs/infrastructure/src/verify/doc_links.rs
    libs/infrastructure/src/verify/doc_patterns.rs
    libs/infrastructure/src/verify/domain_strings.rs
    libs/infrastructure/src/verify/latest_track.rs
    libs/infrastructure/src/verify/layers.rs
    libs/infrastructure/src/verify/module_size.rs
    libs/infrastructure/src/verify/orchestra.rs
    libs/infrastructure/src/verify/spec_attribution.rs
    libs/infrastructure/src/verify/spec_coverage.rs
    libs/infrastructure/src/verify/spec_frontmatter.rs
    libs/infrastructure/src/verify/spec_signals.rs
    libs/infrastructure/src/verify/spec_states.rs
    libs/infrastructure/src/verify/tech_stack.rs
    libs/infrastructure/src/verify/usecase_purity.rs
    libs/infrastructure/src/verify/view_freshness.rs
    apps/cli/src/commands/verify.rs
  Verification:
    cargo test -p infrastructure -p cli (expect pass)
    grep verify::Finding\\b libs/infrastructure/ apps/ (expect zero)
    cargo clippy -p infrastructure -p cli (expect clean)

T004 — rename domain::review_v2::Finding to ReviewerFinding (cascade within review_v2 module)
  Files:
    libs/domain/src/review_v2/types.rs
    libs/domain/src/review_v2/error.rs
    libs/domain/src/review_v2/mod.rs
    libs/domain/src/review_v2/tests.rs
  Verification:
    cargo test -p domain (expect pass)
    grep review_v2::Finding\\b libs/domain/src/review_v2/ (expect zero unupdated refs)
    cargo clippy -p domain (expect clean)

T005 — rename infra + usecase-tests + CLI consumers of domain::review_v2::Finding to ReviewerFinding
  Files:
    libs/infrastructure/src/review_v2/codex_reviewer.rs
    libs/infrastructure/src/review_v2/persistence/review_store.rs
    libs/infrastructure/src/review_v2/persistence/tests.rs
    libs/usecase/src/review_v2/tests.rs
    apps/cli/src/commands/review/codex_local.rs
  Note: persistence/review_store.rs and persistence/tests.rs contain live `Finding` references
  (import, field type, Vec<Finding>, Finding::new calls). libs/usecase/src/review_v2/tests.rs
  imports `domain::review_v2::Finding` and uses `Finding::new`. Not updating these would leave
  cargo test -p infrastructure and cargo test -p usecase broken after T004.
  Verification:
    cargo test -p infrastructure -p usecase -p cli (expect pass)
    grep review_v2::Finding\\b libs/infrastructure/src/review_v2/ libs/usecase/src/review_v2/ apps/cli/src/commands/review/ (expect zero)
    cargo clippy -p infrastructure -p usecase -p cli (expect clean)

T006 — update domain-types.json catalogue (delete reference entry, add new entries)
  Files:
    track/items/tddd-01-multilayer-2026-04-12/domain-types.json
  Actions:
    - Delete the "Finding" reference entry (the 4th entry in the type_definitions array, the collision suppressor)
    - Add "ReviewerFinding" entry: action=declare, kind=value_object
    - Add "NonEmptyReviewerFindings" entry: action=declare, kind=value_object
    - Add "VerifyFinding" entry: action=declare, kind=value_object
    - Regenerate signals via sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain
  Verification:
    sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain (expect red=0 yellow=0, blue >= previous N + 2)
    same-name type collision grep (expect zero)
    grep "Finding" reference entry (expect zero)

T007 — write new ADR 2026-04-14-0625-finding-taxonomy-cleanup.md + amend ADR 0002 §3.B
  Files:
    knowledge/adr/2026-04-14-0625-finding-taxonomy-cleanup.md (new)
    knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md (amend §3.B Resolution subsection)
  Verification:
    sotp verify arch-docs (expect clean)
    grep finding-taxonomy-cleanup knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md (expect one hit)

T008 — run full CI gate and confirm collision-free baseline
  Files: none (verification only)
  Verification:
    cargo make ci (all layers + deny + verify-arch-docs + merge-gate)
    sotp track type-signals --layer domain (blue=N+2, yellow=0, red=0)
    grep "same-name type collision" from track-baseline-capture (expect zero)
```

---

## Section 5 — Acceptance Criteria

- `cargo make check-layers` passes (layer dependency graph unchanged).
- `cargo make deny` passes (no new crate dependencies introduced).
- `cargo test --workspace` passes with zero test failures.
- `cargo clippy --workspace -- -D warnings` passes with no warnings.
- `sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain` reports `yellow=0 red=0` and `blue` count increased by at least 2 relative to the pre-rename baseline (three new `declare` entries replace one `reference` entry; net delta = +3 new − 1 removed = +2 blue minimum).
- No `same-name type collision for Finding` warning in stderr of `cargo make track-baseline-capture <track> --layer domain --force`.
- `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` contains zero entries with `"name": "Finding"`.
- `grep -r "verify::Finding\b" libs/ apps/` returns zero matches.
- `grep -r "review_v2::Finding\b" libs/ apps/` returns zero matches.
- `grep -r 'struct Finding\b' libs/domain/src/` returns zero matches.
- `sotp verify arch-docs` passes (ADR links resolve, canonical module docs compile).
- `cargo make ci` passes end-to-end.

---

## Section 6 — Canonical Blocks

The following blocks must be copied verbatim into `plan.md`.

### Block 1 — verify.rs `VerifyFinding` definition

```rust
/// A single verification finding.
#[derive(Debug, Clone)]
pub struct VerifyFinding {
    severity: Severity,
    message: String,
}

impl VerifyFinding {
    /// Creates a new finding.
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self { severity, message: message.into() }
    }

    /// Creates an error-level finding.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(Severity::Error, message)
    }

    /// Creates a warning-level finding.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message)
    }

    /// Returns the severity level.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for VerifyFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.severity, self.message)
    }
}
```

### Block 2 — review_v2/types.rs `ReviewerFinding` + `NonEmptyReviewerFindings` definitions

```rust
// ── ReviewerFinding ───────────────────────────────────────────────────────

/// A single reviewer finding with optional location metadata.
///
/// Invariant: `message` is non-empty (enforced by constructor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerFinding {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    category: Option<String>,
}

impl ReviewerFinding {
    /// Creates a new reviewer finding.
    ///
    /// # Errors
    /// Returns `ReviewerFindingError::EmptyMessage` if `message` is empty or whitespace-only.
    pub fn new(
        message: impl Into<String>,
        severity: Option<String>,
        file: Option<String>,
        line: Option<u64>,
        category: Option<String>,
    ) -> Result<Self, ReviewerFindingError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(ReviewerFindingError::EmptyMessage);
        }
        Ok(Self { message, severity, file, line, category })
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn severity(&self) -> Option<&str> {
        self.severity.as_deref()
    }

    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub fn line(&self) -> Option<u64> {
        self.line
    }

    pub fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }
}

// ── NonEmptyReviewerFindings ──────────────────────────────────────────────

/// A non-empty collection of reviewer findings.
///
/// Guarantees at least one `ReviewerFinding` is present. The inner `Vec` is private —
/// construction only through `new()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyReviewerFindings(Vec<ReviewerFinding>);

impl NonEmptyReviewerFindings {
    /// Creates a validated non-empty reviewer findings collection.
    ///
    /// # Errors
    /// Returns `VerdictError::EmptyFindings` if `findings` is empty.
    pub fn new(findings: Vec<ReviewerFinding>) -> Result<Self, VerdictError> {
        if findings.is_empty() {
            return Err(VerdictError::EmptyFindings);
        }
        Ok(Self(findings))
    }

    pub fn as_slice(&self) -> &[ReviewerFinding] {
        &self.0
    }

    pub fn into_vec(self) -> Vec<ReviewerFinding> {
        self.0
    }
}
```

### Block 3 — review_v2/error.rs `ReviewerFindingError` definition

```rust
/// Errors from `ReviewerFinding::new` construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewerFindingError {
    #[error("finding message must not be empty or whitespace-only")]
    EmptyMessage,
}
```

### Block 4 — codex_reviewer.rs `convert_findings_to_domain` updated signature

```rust
/// Converts `usecase::review_workflow::ReviewFinding` slice to domain `ReviewerFinding` vec.
fn convert_findings_to_domain(
    findings: &[usecase::review_workflow::ReviewFinding],
) -> Vec<ReviewerFinding> {
    findings
        .iter()
        .filter_map(|f| {
            ReviewerFinding::new(
                &f.message,
                f.severity.clone(),
                f.file.clone(),
                f.line,
                f.category.clone(),
            )
            .ok()
        })
        .collect()
}
```

### Block 5 — codex_local.rs `finding_to_review_finding` updated signature

```rust
/// Converts a domain `ReviewerFinding` to a `ReviewFinding` for JSON serialization.
fn finding_to_review_finding(
    f: &domain::review_v2::ReviewerFinding,
) -> usecase::review_workflow::ReviewFinding {
    usecase::review_workflow::ReviewFinding {
        message: f.message().to_owned(),
        severity: f.severity().map(str::to_owned),
        file: f.file().map(str::to_owned),
        line: f.line(),
        category: f.category().map(str::to_owned),
    }
}
```

### Block 6 — domain-types.json updated entries (replace the "Finding" reference entry, 4th in type_definitions)

```json
{
  "name": "ReviewerFinding",
  "description": "Domain-validated reviewer finding. Invariant: message is non-empty. Counterpart to usecase::review_workflow::ReviewFinding (serde DTO).",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
},
{
  "name": "NonEmptyReviewerFindings",
  "description": "Non-empty collection of ReviewerFinding values. Used as the inner payload of Verdict::FindingsRemain and FastVerdict::FindingsRemain.",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
},
{
  "name": "VerifyFinding",
  "description": "Structured error or warning produced by sotp verify subcommands. Has a Severity (Info/Warning/Error) and a message string.",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
}
```

### Block 7 — Rename summary table (for ADR and plan.md)

| Old name | New name | Layer | Note |
|---|---|---|---|
| `domain::review_v2::Finding` | `domain::review_v2::ReviewerFinding` | domain | Validated newtype |
| `domain::review_v2::NonEmptyFindings` | `domain::review_v2::NonEmptyReviewerFindings` | domain | Collection invariant |
| `domain::review_v2::FindingError` | `domain::review_v2::ReviewerFindingError` | domain | Constructor error |
| `domain::verify::Finding` | `domain::verify::VerifyFinding` | domain | Verify subcommand output |
| `VerdictError::EmptyFindings` | (unchanged) | domain | Variant semantically correct |
| `convert_findings_to_domain` | (unchanged) | infrastructure | Private fn; return type updated |
| `finding_to_review_finding` | (unchanged) | apps/cli | Private fn; param type updated |

### Block 8 — Data-flow diagram (mermaid)

```mermaid
flowchart TD
    subgraph review_flow["Codex local review flow"]
        J["reviewer JSON output (wire format)"]
        J -->|serde_json from_str| DTO["usecase::review_workflow::ReviewFinding (Serialize + Deserialize DTO)"]
        DTO -->|convert_findings_to_domain filter_map| DM["domain::review_v2::ReviewerFinding (non-empty message invariant)"]
        DM -->|NonEmptyReviewerFindings::new| NEF["NonEmptyReviewerFindings"]
        NEF -->|Verdict::findings_remain| VRD["Verdict::FindingsRemain"]
        VRD --> CALLER["usecase / CLI caller"]
    end

    subgraph emit_flow["CLI emit path (codex-local)"]
        FV["FastVerdict::FindingsRemain(NonEmptyReviewerFindings)"]
        FV -->|as_slice iter + finding_to_review_finding| DTO2["usecase::review_workflow::ReviewFinding"]
        DTO2 -->|serde_json to_string| STDOUT["JSON stdout ReviewFinalPayload"]
    end

    subgraph verify_flow["sotp verify flow"]
        CLI_CMD["sotp verify subcommand"]
        CLI_CMD -->|VerifyFinding::error or warning| VF["domain::verify::VerifyFinding (Severity + message)"]
        VF -->|VerifyOutcome::from_findings or add or merge| VO["domain::verify::VerifyOutcome (Vec VerifyFinding)"]
        VO -->|returned to CLI| CLI_OUT["CLI: render + ExitCode"]
    end
```

---

## Section 7 — Risks and Rollback Plan

### Risks

1. **Silent drop of empty-message findings**: `convert_findings_to_domain` uses `filter_map(…ok())` to discard `ReviewerFindingError::EmptyMessage`. After rename, the filter behaviour is identical — but if someone introduces a new constructor call elsewhere without the `filter_map`, silently-dropped findings could surface as data loss. Mitigation: the existing test `test_convert_findings_to_domain_skips_empty_message` catches this after rename propagation.

2. **`domain-types.json` catalogue out-of-sync**: If T006 regenerates the baseline before T004/T005 compile cleanly, the baseline captures stale type names. Mitigation: enforce compile-clean gate before T006 (`cargo build -p domain` must succeed).

3. **`sotp track type-signals --layer domain` blue count change**: After rename, the suppression reference entry is removed and three new `declare` entries are added. If the new names are not yet in the compiled rustdoc JSON (because T004 is not yet committed), the signals evaluator will see yellow instead of blue for the new entries. Mitigation: ensure T006 runs after T004 has compiled cleanly and the rustdoc JSON has been regenerated.

4. **ADR cascade**: ADR `2026-04-04-1456-review-system-v2-redesign.md` contains an in-line Rust snippet referencing `Finding`. This is a historical ADR — do not retroactively edit snippets. The new ADR should note that historical ADRs use the old names.

5. **`knowledge/conventions/source-attribution.md` prose mention**: The file at line 29 references `Finding::warning` as a prose example. This is a convention doc checked by `sotp verify arch-docs` / `cargo make verify-arch-docs`. **This must be updated** (to `VerifyFinding::warning`) or the CI check may fail if it validates code references.

6. **`knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` in-line code snippets**: Occurrences of `Finding::error(…)` and `Finding::warning(…)` in pseudo-code blocks may exist. These are historical design descriptions — leave them and note in the new ADR. If `sotp verify arch-docs` lints these, they would need updating; verify this with a dry-run before T007.

7. **`FindingSeverity` in `domain::auto_phase`**: This is an unrelated enum (`P1`/`P2`/`P3`). The name starts with `Finding` as a compound adjective, not as a type reference. No rename needed. Mitigation: all renames should be targeted (`struct Finding`, `enum Finding`, `use.*Finding`, not global string replace).

8. **`VerdictError::EmptyFindings` kept**: the variant name references the old concept ("Findings"). This is a judgment call — the variant name accurately describes what the error means ("the findings collection is empty"). Renaming it to `EmptyReviewerFindings` would be over-engineering. This is the accepted trade-off.

### Rollback Plan

Each task is one focused commit. If any task commit causes CI regression:

- T001/T002/T003 (VerifyFinding cascade): revert via `git revert <commit>`. These are independent of the review_v2 rename.
- T004/T005 (ReviewerFinding cascade): revert via `git revert <commit>`. The collision warning reappears but is suppressed by the existing reference entry until T006 is also reverted.
- T006 (catalogue update): revert via `git revert <commit>`. The reference entry suppressor is restored.
- T007 (CI gate): T007 is a verification-only task (0 diff lines except for optional historical note additions). If a CI failure is found during T007, identify and revert the specific earlier task that caused the regression rather than reverting T007 itself. Note: in the canonical 7-task track plan, T007 is the CI gate, NOT the ADR writing (the ADR was written during the planning phase and has no implementation commit).

If T006 is reverted without reverting T004/T005, the catalogue will reference `ReviewerFinding` and `VerifyFinding` which now exist in compiled code — the TDDD evaluator will show blue for those entries and zero collision warnings, which is the correct stable state for a partial deploy.

### Additional Tests Recommended

1. A compile-only integration test that `use domain::review_v2::ReviewerFinding;` resolves (catches accidental re-export omission from `mod.rs`).
2. A test asserting `domain::review_v2::ReviewerFindingError::EmptyMessage` is returned by `ReviewerFinding::new("", …)` (already present, just confirm the rename propagated correctly).
3. A test asserting `domain::verify::VerifyFinding::error("x").severity() == Severity::Error` (already present in `verify.rs`, confirm rename propagated).
4. A dedicated test in `codex_reviewer.rs` asserting that `convert_findings_to_domain` returns zero elements when all DTO findings have empty messages (partially exists as `test_convert_findings_to_domain_skips_empty_message`; confirm it still compiles and passes after rename).
