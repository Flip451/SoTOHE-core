# Planner Design Review: TSUMIKI-05/06/07 Hearing UX Improvement

Date: 2026-04-01
Provider: Claude Code (Codex fallback due to usage limit)
Briefing: tmp/planner-briefing-hearing-ux.md

## 1. Review of Three Changes for Completeness and Consistency

### TSUMIKI-05: Structured Question UX

The proposal is coherent and self-contained (SKILL.md-only). The current Step 4a presents a static Markdown wall, and replacing it with `AskUserQuestion` + `multiSelect: true` is a clear improvement. However, several interactions between the three category types need explicit protocol:

- Yellow items have three options including "Modify (will explain)" — this requires a mandatory follow-up `AskUserQuestion` with `multiSelect: false` (free text prompt). The briefing mentions capturing this but does not specify how many Yellow-items with "Modify" trigger follow-ups versus being batched.
- Red items should get options derived from context, but the skill text does not specify how Claude should generate those options. A concrete rule is needed (e.g., "generate 2-3 alternatives based on the item text and codebase context, always include 'Other (free text)'").
- Missing items are cleanly handled: "Add to spec / Not needed / Need more info". "Need more info" should also trigger a follow-up question.

One gap: the current SKILL.md Step 4a already instructs "update spec.json after hearing." TSUMIKI-05 must preserve this post-hearing update logic exactly — the source tagging rules (adding `feedback — ...` to upgrade signals) are in SKILL.md Step 4a and must not be lost when replacing the question format.

### TSUMIKI-06: Workload Mode Selection

The proposal is sound. The three modes map cleanly to real use cases (new feature, focused iteration, quick patch). However, the interaction between mode and the existing differential hearing logic (TSUMIKI-03) is underspecified:

- "Quick" mode as described ("Show Blue summary, ask 'any changes?'") bypasses the per-item classification entirely. This means Quick skips both the `bin/sotp track signals` evaluation and the detailed question flow. This needs to be explicit in the instruction.
- "Focused" mode says "skip researcher/planner, differential hearing on Yellow/Red/Missing only." This is essentially the current differential hearing, but without Phase 1 Steps 1-2 and without Phase 1.5 (planner review). The spec signals command still runs.

Consistency concern: SKILL.md Phase 1.5 says "すべての機能で planner capability による設計レビューを実施する" (mandatory for all features). TSUMIKI-06 needs to explicitly state whether "Focused" and "Quick" modes exempt from Phase 1.5. Given that these modes are designed for minor updates, the exemption is reasonable but must be called out as a named exception to the existing rule.

### TSUMIKI-07: Hearing Record Schema Extension

The proposed JSON shape is well-structured. The `signal_delta` field tracks the count change, which gives useful at-a-glance history. The `mode` field will tie TSUMIKI-06 mode selection back to the record. This is the only change in the set that touches Rust domain and infrastructure code.

One architectural concern: `hearing_history` is auditing/operational metadata, not spec content. The existing `content_hash` computation explicitly excludes `signals` and `domain_state_signals` from the hash, for exactly this reason — they are derived/bookkeeping data. The same exclusion must apply to `hearing_history`. This must be confirmed explicitly in the design.

## 2. Answers to Design Questions

### TSUMIKI-05 Questions

**Q1. Single AskUserQuestion batch vs one-by-one?**

Recommendation: **grouped batch with a 5-item cap per call.** A single call for all Yellow/Red/Missing items is feasible only when the count is small (3-5 total). When there are more, a single `multiSelect` with 15+ items becomes cognitively overwhelming and defeats the purpose. The skill should group items into batches of at most 5, presenting one `AskUserQuestion` call per batch. Group by category (all Yellow first, then Red, then Missing) within each batch.

**Q2. Should Blue items be presented?**

Yes, but briefly and passively. Show a single summary line: "N confirmed items (Blue) — will carry forward unchanged. Reply to flag any you want to revisit." This matches the current SKILL.md Step 4a format exactly and can be a simple text block rather than an `AskUserQuestion` call.

**Q3. How should "Modify" responses be captured?**

Immediately after receiving a batch response where any item has "Modify (will explain)" selected, issue a follow-up `AskUserQuestion` with `multiSelect: false` for each such item individually. The follow-up answer becomes the new `text` value, with `feedback — {answer}` appended to `sources`.

### TSUMIKI-06 Questions

**Q1. Where should mode selection happen?**

Before Step 1 (researcher). Insert a new Step 0. Use `AskUserQuestion` with options: "Full (complete pipeline)", "Focused (skip research phases, run hearing only)", "Quick (show Blue summary, open-ended change request)". Also show current spec status (Blue/Yellow/Red counts, last update).

**Q2. Should "Focused" mode skip Phase 1.5 (planner review)?**

Yes. The planner review is designed for non-trivial implementation changes. In Focused mode, the user is updating requirements — no new Rust code is being designed. The skill should state: "Phase 1.5 is skipped in Focused and Quick modes. If the hearing reveals architectural changes, re-run in Full mode."

**Q3. What about Phase 2 (Agent Teams research)?**

Skip entirely in both Focused and Quick modes. Phase 2 is for new feature discovery. In Focused/Quick, the codebase is already understood and a spec already exists.

### TSUMIKI-07 Questions

**Q1. hearing_history in spec.json or a separate file?**

Keep in spec.json. The pattern is established: `signals`, `domain_state_signals`, `approved_at`, and `content_hash` are all non-spec metadata that live in spec.json already.

**Q2. signal_delta as [before, after] or separate objects?**

Separate named fields `before` and `after`, each containing `{ "blue": u32, "yellow": u32, "red": u32 }`. Self-documenting and reuses existing `SignalCountsDto`.

**Q3. Should render_spec() include hearing history?**

Lightweight summary: last 5 entries as a compact table (date, mode, items added/modified) in a `## Hearing History` section after Signal Summary.

**Q4. Is hearing_history append-only?**

Yes. Only `append_hearing_record()` method; no remove or replace.

## 3. Edge Cases and Risks

### TSUMIKI-05

- Empty hearing target: all Blue → short-circuit to Quick equivalent
- "Modify" on Red: offer "Provide correct text" instead of "Modify"
- AskUserQuestion option count limit: cap batches at 5 items
- "Need more info" for Missing: note as pending, optionally add as Red for next hearing

### TSUMIKI-06

- Mode selection with no spec.json: warn and fall back to Full
- Quick mode: does NOT invoke TSUMIKI-05 structured questions (free text only)
- Focused mode and tech-stack.md: warn but allow if not about tech decisions

### TSUMIKI-07

- **content_hash exclusion**: hearing_history MUST be excluded from hash computation
- Backward compatibility: `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
- Timestamp validation: reuse existing `Timestamp` type
- SKILL.md counting rules must be explicit (when to read before/after signals)
- Render cap: last 5 entries in spec.md

## 4. Recommended Task Ordering

```
T001: TSUMIKI-06 — Mode selection (SKILL.md Step 0 insertion)
  ↓ mode names finalized
T002: TSUMIKI-07 — HearingRecord Rust types (domain + infra + render)
  ↓ schema stable, mode enum available
T003: TSUMIKI-05 — Structured questions (SKILL.md Step 4a rewrite)
  ↓ references mode gate + hearing_history schema
T004: CI validation (cargo make ci)
```

## Canonical Blocks

```rust
// libs/domain/src/spec.rs  (or a new libs/domain/src/hearing.rs)

/// Workload mode for a hearing session.
///
/// Determines which pipeline phases were executed during the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HearingMode {
    /// Full pipeline: researcher + planner + differential hearing.
    Full,
    /// Skip researcher/planner; run differential hearing only.
    Focused,
    /// Show Blue summary only; accept free-text change requests.
    Quick,
}

impl HearingMode {
    /// Returns the canonical string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Focused => "focused",
            Self::Quick => "quick",
        }
    }
}

/// Signal counts snapshot for before/after comparison in a hearing session.
///
/// Mirrors `SignalCounts` but is always concrete (no Option wrapping needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HearingSignalSnapshot {
    blue: u32,
    yellow: u32,
    red: u32,
}

impl HearingSignalSnapshot {
    #[must_use]
    pub fn new(blue: u32, yellow: u32, red: u32) -> Self {
        Self { blue, yellow, red }
    }

    #[must_use]
    pub fn blue(&self) -> u32 { self.blue }
    #[must_use]
    pub fn yellow(&self) -> u32 { self.yellow }
    #[must_use]
    pub fn red(&self) -> u32 { self.red }
}

impl From<SignalCounts> for HearingSignalSnapshot {
    fn from(c: SignalCounts) -> Self {
        Self::new(c.blue(), c.yellow(), c.red())
    }
}

/// Delta between before and after signal counts for a hearing session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HearingSignalDelta {
    before: HearingSignalSnapshot,
    after: HearingSignalSnapshot,
}

impl HearingSignalDelta {
    #[must_use]
    pub fn new(before: HearingSignalSnapshot, after: HearingSignalSnapshot) -> Self {
        Self { before, after }
    }

    #[must_use]
    pub fn before(&self) -> &HearingSignalSnapshot { &self.before }
    #[must_use]
    pub fn after(&self) -> &HearingSignalSnapshot { &self.after }
}

/// A single hearing session record.
///
/// Append-only; once written, entries are never modified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HearingRecord {
    date: Timestamp,
    mode: HearingMode,
    signal_delta: HearingSignalDelta,
    questions_asked: u32,
    items_added: u32,
    items_modified: u32,
}

impl HearingRecord {
    /// Creates a new hearing record.
    #[must_use]
    pub fn new(
        date: Timestamp,
        mode: HearingMode,
        signal_delta: HearingSignalDelta,
        questions_asked: u32,
        items_added: u32,
        items_modified: u32,
    ) -> Self {
        Self { date, mode, signal_delta, questions_asked, items_added, items_modified }
    }

    #[must_use]
    pub fn date(&self) -> &Timestamp { &self.date }
    #[must_use]
    pub fn mode(&self) -> HearingMode { self.mode }
    #[must_use]
    pub fn signal_delta(&self) -> &HearingSignalDelta { &self.signal_delta }
    #[must_use]
    pub fn questions_asked(&self) -> u32 { self.questions_asked }
    #[must_use]
    pub fn items_added(&self) -> u32 { self.items_added }
    #[must_use]
    pub fn items_modified(&self) -> u32 { self.items_modified }
}
```

```rust
// libs/infrastructure/src/spec/codec.rs additions

/// DTO for a single hearing session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HearingRecordDto {
    pub date: String,
    pub mode: String,
    pub signal_delta: HearingSignalDeltaDto,
    pub questions_asked: u32,
    pub items_added: u32,
    pub items_modified: u32,
}

/// DTO for signal delta (before + after snapshots).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HearingSignalDeltaDto {
    pub before: SignalCountsDto,
    pub after: SignalCountsDto,
}

// In SpecDocumentDto, add:
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub hearing_history: Vec<HearingRecordDto>,
```

```rust
// libs/infrastructure/src/spec/render.rs addition

fn render_hearing_history(doc: &SpecDocument) -> String {
    let history = doc.hearing_history();
    if history.is_empty() {
        return String::new();
    }
    let mut out = String::from("## Hearing History\n\n");
    out.push_str("| Date | Mode | Questions | Added | Modified |\n");
    out.push_str("|------|------|-----------|-------|----------|\n");
    // Show last 5 entries, most recent first.
    let display: Vec<_> = history.iter().rev().take(5).collect();
    for record in display {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            record.date(),
            record.mode().as_str(),
            record.questions_asked(),
            record.items_added(),
            record.items_modified(),
        ));
    }
    out.push('\n');
    out
}
```
