# Verification — TDDD-04: Finding 型 Taxonomy クリーンアップ

## Known Accepted Deviations (C4 completion commit)

- **C4 review sequencing**: `metadata.json status: "done"` and `track.status: "done"` are set in the C4 completion commit, which is then reviewed in the `other` scope. During the C4 review itself, `review.json` will have interim `findings_remain` rounds appended. This is expected and does not indicate an implementation problem — it reflects that the review of C4 artifacts runs after the commit, not before. `review.json` content is not in scope for the C4 completion commit (it is gitignored / not modified by C4).
- **review.json archival**: Timestamp-based archival (as done in tddd-02) is deferred to a separate chore commit before PR merge, not part of the C4 completion commit.
- **C4 commit hash in T007**: T007 references C3 commit (3ce2bbd, the last code-verified commit) not the C4 chore commit itself. This matches the tddd-02 T017 backfill pattern.

## Scope Verified

- [x] `domain::verify::Finding` が `VerifyFinding` にリネームされ、`libs/domain/src/verify.rs` の struct / constructors / methods / `impl Display` / `VerifyOutcome` field-method シグネチャが全て更新されている (T001)
- [x] `libs/domain/src/tddd/consistency.rs` と `libs/domain/src/spec.rs` の import と `Finding::error / warning` 呼び出しが `VerifyFinding::…` に更新されている (T001)
- [x] `libs/usecase/src/merge_gate.rs` と `libs/usecase/src/task_completion.rs` の import と呼び出しが `VerifyFinding::…` に更新されている (T002)
- [x] `libs/infrastructure/src/verify/*.rs` の 18 ファイル (architecture_rules / canonical_modules / convention_docs / doc_links / doc_patterns / domain_strings / latest_track / layers / module_size / orchestra / spec_attribution / spec_coverage / spec_frontmatter / spec_signals / spec_states / tech_stack / usecase_purity / view_freshness) の import と呼び出しが `VerifyFinding::…` に更新されている (T003)
- [x] `apps/cli/src/commands/verify.rs` の import / 呼び出し / 返り値型注釈が `VerifyFinding` に更新されている (T003)
- [x] `knowledge/conventions/source-attribution.md` line 29 の `Finding::warning` prose mention が `VerifyFinding::warning` に更新されている (T003)
- [x] `domain::review_v2::Finding` が `ReviewerFinding` にリネームされ、`libs/domain/src/review_v2/types.rs` の struct / NonEmptyFindings → NonEmptyReviewerFindings / `Verdict::findings_remain` + `FastVerdict::findings_remain` 引数型が更新されている (T004)
- [x] `libs/domain/src/review_v2/error.rs` の `FindingError` → `ReviewerFindingError` にリネームされ、variant `EmptyMessage` は保持されている (T004)
- [x] `libs/domain/src/review_v2/mod.rs` の `pub use` re-export 名が `ReviewerFinding` / `NonEmptyReviewerFindings` / `ReviewerFindingError` に更新され、旧名は残存しない (T004)
- [x] `libs/domain/src/review_v2/tests.rs` の test helper / assertion / import が新名に更新されている (T004)
- [x] `libs/infrastructure/src/review_v2/codex_reviewer.rs` の `convert_findings_to_domain` が `-> Vec<ReviewerFinding>` を返し、内部で `ReviewerFinding::new(…)` を呼んでいる (T005)
- [x] `libs/infrastructure/src/review_v2/persistence/review_store.rs` の import / `findings: &[ReviewerFinding]` 引数型 / `Vec<ReviewerFinding>` / `ReviewerFinding::new` 呼び出しが更新されている (T005)
- [x] `libs/infrastructure/src/review_v2/persistence/tests.rs` の import / `fn sample_finding() -> ReviewerFinding` 戻り値型 / `ReviewerFinding::new` 呼び出しが更新されている (T005)
- [x] `libs/usecase/src/review_v2/tests.rs` の `use domain::review_v2::{..., Finding, ...}` が `ReviewerFinding` に更新され、`Finding::new` 呼び出しが `ReviewerFinding::new` に更新されている (T005)
- [x] `apps/cli/src/commands/review/codex_local.rs` の `finding_to_review_finding` 引数型が `&domain::review_v2::ReviewerFinding` に更新されている (T005)
- [x] `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` の `"Finding"` reference entry が削除され、代わりに 4 つの declare entries (`VerifyFinding` / `ReviewerFinding` / `NonEmptyReviewerFindings` / `ReviewerFindingError`) が追加されている。全て `kind=value_object` (ReviewerFindingError は `kind=enum`)、`approved=true` (T006)
- [x] `bin/sotp track type-signals tddd-01-multilayer-2026-04-12 --layer domain` の出力が `blue=7 yellow=0 red=0 (total=7, undeclared=0, skipped=110)` で、blue count が更新前より増加している (T006)
- [x] `bin/sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer domain --force` の stderr に `same-name type collision for Finding` warning が含まれない (T006, T007)
- [x] `cargo make ci` (fmt-check + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-arch-docs) が全通過する (T007)

## Manual Verification Steps

### 1. `verify::Finding` → `VerifyFinding` rename 検証 (T001-T003)

```bash
# 旧名が残存しないこと
rg 'verify::Finding\b' libs/ apps/              # expect: zero matches
rg '([^a-zA-Z_]|^)Finding::error([^a-zA-Z_]|$)' libs/ apps/   # expect: zero matches (both-side char-class boundary; catches `Finding::error` with or without parens, e.g. in doc comments)
rg '([^a-zA-Z_]|^)Finding::warning([^a-zA-Z_]|$)' libs/ apps/ # expect: zero matches
rg 'use domain::verify::Finding' libs/ apps/    # expect: zero matches
rg 'use crate::verify::Finding' libs/           # expect: zero matches

# 新名が存在すること
rg 'VerifyFinding' libs/ apps/                  # expect: many matches
rg 'struct VerifyFinding' libs/domain/src/verify.rs  # expect: 1 match
rg 'impl VerifyFinding' libs/domain/src/verify.rs    # expect: 1 match
rg 'impl fmt::Display for VerifyFinding' libs/domain/src/verify.rs  # expect: 1 match

# convention 更新
rg '([^a-zA-Z_]|^)Finding::warning' knowledge/conventions/    # expect: zero (char-class boundary prevents false match on VerifyFinding::warning where y precedes F)
rg 'VerifyFinding::warning' knowledge/conventions/source-attribution.md  # expect: 1 match
```

### 2. `review_v2::Finding` → `ReviewerFinding` rename 検証 (T004-T005)

```bash
# 旧名が残存しないこと (review_v2 module 内および consumer 側)
rg 'review_v2::Finding\b' libs/ apps/           # expect: zero matches
rg 'NonEmptyFindings\b' libs/ apps/             # expect: zero matches (all become NonEmptyReviewerFindings)
rg '([^a-zA-Z_]|^)FindingError([^a-zA-Z_]|$)' libs/domain/src/review_v2/  # expect: zero matches (char-class boundary prevents match on ReviewerFindingError where F is preceded by letter r)
rg '([^a-zA-Z_]|^)Finding([^a-zA-Z_]|$)' libs/domain/src/review_v2/mod.rs  # expect: zero matches (char-class boundary ensures standalone name; does not match ReviewerFinding/NonEmptyReviewerFindings/ReviewerFindingError where F is preceded by a letter)

# 新名が存在すること
rg 'struct ReviewerFinding' libs/domain/src/review_v2/types.rs        # expect: 1 match
rg 'struct NonEmptyReviewerFindings' libs/domain/src/review_v2/types.rs  # expect: 1 match
rg 'enum ReviewerFindingError' libs/domain/src/review_v2/error.rs    # expect: 1 match
rg 'pub use .*ReviewerFinding' libs/domain/src/review_v2/mod.rs      # expect: matches
rg 'Vec<ReviewerFinding>' libs/infrastructure/src/review_v2/codex_reviewer.rs  # expect: 1 match (convert_findings_to_domain return type)
rg 'ReviewerFinding' libs/infrastructure/src/review_v2/persistence/review_store.rs  # expect: matches (import + findings: &[ReviewerFinding] + Vec<ReviewerFinding>)
rg 'ReviewerFinding' libs/infrastructure/src/review_v2/persistence/tests.rs  # expect: matches (import + fn sample_finding return type)
rg 'ReviewerFinding' libs/usecase/src/review_v2/tests.rs  # expect: matches (import + Finding::new calls)
rg '&domain::review_v2::ReviewerFinding' apps/cli/src/commands/review/codex_local.rs  # expect: 1 match (finding_to_review_finding param)

# VerdictError::EmptyFindings 維持確認
rg 'VerdictError::EmptyFindings' libs/domain/src/review_v2/   # expect: 4+ matches (preserved)
```

### 3. `domain::auto_phase::FindingSeverity` が誤 rename されていないこと

```bash
rg 'FindingSeverity' libs/domain/src/auto_phase.rs  # expect: matches (unchanged)
rg 'ReviewerFindingSeverity' libs/                  # expect: zero matches (should NOT exist)
rg 'VerifyFindingSeverity' libs/                    # expect: zero matches (should NOT exist)
```

### 4. TDDD catalogue 更新検証 (T006)

```bash
# "Finding" reference entry 削除
jq '.type_definitions[] | select(.name == "Finding") | .name' \
  track/items/tddd-01-multilayer-2026-04-12/domain-types.json  # expect: no output

# 4 new declare entries 存在
jq '.type_definitions[] | .name' \
  track/items/tddd-01-multilayer-2026-04-12/domain-types.json
# expect output includes: "VerifyFinding", "ReviewerFinding", "NonEmptyReviewerFindings", "ReviewerFindingError"

# signals 再生成
cargo make build-sotp
bin/sotp track type-signals tddd-01-multilayer-2026-04-12 --layer domain
# expect output: [OK] type-signals: blue=7 yellow=0 red=0 (total=7, undeclared=0, skipped=110)
```

### 5. Collision warning 消失確認 (T006, T007)

```bash
# baseline capture stderr から collision warning が消えていること
bin/sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer domain --force 2>&1 \
  | grep -i 'same-name type collision' || echo '[OK] no collision warning'
# expect: [OK] no collision warning
```

### 6. JSON wire format 非変更確認 (T005)

```bash
# REVIEW_OUTPUT_SCHEMA_JSON の $defs/finding key が変更されていないこと
rg '"finding":\s*\{' libs/usecase/src/review_workflow/verdict.rs
# expect: 1 match (JSON schema field name unchanged)

rg '"findings":\s*\{' libs/usecase/src/review_workflow/verdict.rs
# expect: 1 match (JSON schema field name unchanged)
```

### 7. 歴史的 ADR の historical record 確認 (T007)

```bash
# ADR 2026-04-12-1200 内の historical pseudo-code は旧名のまま残存
rg 'Finding::error|Finding::warning' knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md | wc -l
# expect: 33 matches (historical, untouched; verified 2026-04-14)

# ADR 2026-04-04-1456 内の historical Rust snippet も旧名のまま残存
rg '\bFinding\b' knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md | wc -l
# expect: 8 matches (historical, untouched; verified 2026-04-14)

# verify-arch-docs を実行して historical snippets が lint エラーにならないか確認
cargo run --quiet -p cli -- verify arch-docs
# expect: exit 0 (arch-docs does not validate pseudo-code inside ADR code blocks;
# tddd-02 CI 通過実績 2026-04-14 で確認済み)
```

### 8. 最終 CI gate (T007)

```bash
cargo make fmt                 # ensure normalized formatting
cargo make ci                  # fmt-check + clippy + test + deny + check-layers + verify-*
# expect: all tasks pass
```

## Result

| Task | Title | Commit | Review Scopes (fast+final per scope) |
|------|-------|--------|---------------------------------------|
| T001–T003 (C1) | verify::Finding → VerifyFinding cascade | cc5bd63 | domain(1+1) + infra(1+1) + usecase(1+1) + cli(1+1) + harness-policy(1+1) = 10 rounds |
| T004–T005 (C2) | review_v2::Finding → ReviewerFinding cascade | ab744a3 | domain(1+1) + infra(1+1) + usecase(1+1) + cli(1+1) = 8 rounds |
| T006 (C3) | domain-types.json catalogue update + baseline regen | 3ce2bbd | — (catalogue operational, no reviewer required) |
| T007 (C3) | CI gate + self-review + task completion | 3ce2bbd | — (verification only) |

**Review cycles**: C1 (T001–T003) was reviewed across 5 scopes × fast+final = 10 rounds, all `zero_findings` on first pass. C2 (T004–T005) was reviewed across 4 scopes × fast+final = 8 rounds, all `zero_findings` on first pass. Total: 18 rounds across 5 non-`other` scopes, all `zero_findings`. The `other` scope covered track artifact reviews (C1–C4) with multiple fix-review iterations.

**Key verification results**:
- `cargo make ci` passes end-to-end after each of C1, C2, C3, C4.
- All residual greps for `verify::Finding\b`, `review_v2::Finding\b`, `struct Finding\b`, `FindingError` (standalone), `NonEmptyFindings` return zero matches in `libs/` and `apps/`.
- `FindingSeverity` in `libs/domain/src/auto_phase.rs` preserved (19 occurrences; unrelated P1/P2/P3 priority enum).
- Historical ADR references preserved exactly as expected: 33 occurrences of `Finding::error|warning` in `2026-04-12-1200-strict-spec-signal-gate-v2.md`, 8 occurrences of bare `Finding` in `2026-04-04-1456-review-system-v2-redesign.md`.
- `bin/sotp track type-signals tddd-01-multilayer-2026-04-12 --layer domain` reports `blue=7 yellow=0 red=0 (total=7, undeclared=0, skipped=110)`.
- `bin/sotp track baseline-capture tddd-01-multilayer-2026-04-12 --layer domain --force` completes without the `same-name type collision for Finding` warning.
- `bin/sotp track signals tddd-04-finding-taxonomy-cleanup-2026-04-14` reports `blue=38 yellow=0 red=0 (total=38)`.

### Open Issues

No open issues remaining for TDDD-04. The originally flagged follow-up tracks are unchanged:

- `domain-serde-ripout-YYYY-MM-DD`: move `Serialize` derives out of `libs/domain/src/{schema,catalogue}.rs` and remove the `serde` dependency from `libs/domain/Cargo.toml`. Not required by TDDD-04 and intentionally deferred to avoid scope bloat. Source: ADR `2026-04-14-0625-finding-taxonomy-cleanup.md` D6.
- `historical-adr-lint-resolution-YYYY-MM-DD`: only needed if `sotp verify arch-docs` is later changed to lint ADR Rust code blocks at the type-reference level. Currently CI does not flag the 33 + 8 historical `Finding` occurrences.

## verified_at

2026-04-14
