# Verification — TDDD-04: Finding 型 Taxonomy クリーンアップ

## Scope Verified

- [ ] `domain::verify::Finding` が `VerifyFinding` にリネームされ、`libs/domain/src/verify.rs` の struct / constructors / methods / `impl Display` / `VerifyOutcome` field-method シグネチャが全て更新されている (T001)
- [ ] `libs/domain/src/tddd/consistency.rs` と `libs/domain/src/spec.rs` の import と `Finding::error / warning` 呼び出しが `VerifyFinding::…` に更新されている (T001)
- [ ] `libs/usecase/src/merge_gate.rs` と `libs/usecase/src/task_completion.rs` の import と呼び出しが `VerifyFinding::…` に更新されている (T002)
- [ ] `libs/infrastructure/src/verify/*.rs` の 18 ファイル (architecture_rules / canonical_modules / convention_docs / doc_links / doc_patterns / domain_strings / latest_track / layers / module_size / orchestra / spec_attribution / spec_coverage / spec_frontmatter / spec_signals / spec_states / tech_stack / usecase_purity / view_freshness) の import と呼び出しが `VerifyFinding::…` に更新されている (T003)
- [ ] `apps/cli/src/commands/verify.rs` の import / 呼び出し / 返り値型注釈が `VerifyFinding` に更新されている (T003)
- [ ] `knowledge/conventions/source-attribution.md` line 29 の `Finding::warning` prose mention が `VerifyFinding::warning` に更新されている (T003)
- [ ] `domain::review_v2::Finding` が `ReviewerFinding` にリネームされ、`libs/domain/src/review_v2/types.rs` の struct / NonEmptyFindings → NonEmptyReviewerFindings / `Verdict::findings_remain` + `FastVerdict::findings_remain` 引数型が更新されている (T004)
- [ ] `libs/domain/src/review_v2/error.rs` の `FindingError` → `ReviewerFindingError` にリネームされ、variant `EmptyMessage` は保持されている (T004)
- [ ] `libs/domain/src/review_v2/mod.rs` の `pub use` re-export 名が `ReviewerFinding` / `NonEmptyReviewerFindings` / `ReviewerFindingError` に更新され、旧名は残存しない (T004)
- [ ] `libs/domain/src/review_v2/tests.rs` の test helper / assertion / import が新名に更新されている (T004)
- [ ] `libs/infrastructure/src/review_v2/codex_reviewer.rs` の `convert_findings_to_domain` が `-> Vec<ReviewerFinding>` を返し、内部で `ReviewerFinding::new(…)` を呼んでいる (T005)
- [ ] `libs/infrastructure/src/review_v2/persistence/review_store.rs` の import / `findings: &[ReviewerFinding]` 引数型 / `Vec<ReviewerFinding>` / `ReviewerFinding::new` 呼び出しが更新されている (T005)
- [ ] `libs/infrastructure/src/review_v2/persistence/tests.rs` の import / `fn sample_finding() -> ReviewerFinding` 戻り値型 / `ReviewerFinding::new` 呼び出しが更新されている (T005)
- [ ] `libs/usecase/src/review_v2/tests.rs` の `use domain::review_v2::{..., Finding, ...}` が `ReviewerFinding` に更新され、`Finding::new` 呼び出しが `ReviewerFinding::new` に更新されている (T005)
- [ ] `apps/cli/src/commands/review/codex_local.rs` の `finding_to_review_finding` 引数型が `&domain::review_v2::ReviewerFinding` に更新されている (T005)
- [ ] `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` の `"Finding"` reference entry が削除され、代わりに 3 つの `declare` entries (`ReviewerFinding` / `NonEmptyReviewerFindings` / `VerifyFinding`) が追加されている。全て `kind=value_object, approved=true` (T006)
- [ ] `bin/sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain` の出力が `yellow=0 red=0` で、blue count が更新前より 2 以上増加している (T006)
- [ ] `cargo make track-baseline-capture -- tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain --force` の stderr に `same-name type collision for Finding` warning が含まれない (T006, T007)
- [ ] `cargo make ci` (fmt-check + clippy -D warnings + test + deny + check-layers + verify-spec-states + verify-arch-docs) が全通過する (T007)

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

# 3 new declare entries 存在
jq '.type_definitions[] | select(.action == "declare") | .name' \
  track/items/tddd-01-multilayer-2026-04-12/domain-types.json
# expect output includes: "ReviewerFinding", "NonEmptyReviewerFindings", "VerifyFinding"

# signals 再生成
cargo make build-sotp
bin/sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain
# expect output: blue=N+2 yellow=0 red=0 (where N is the pre-rename blue count)
```

### 5. Collision warning 消失確認 (T006, T007)

```bash
# baseline capture stderr から collision warning が消えていること
cargo make track-baseline-capture -- tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain --force 2>&1 \
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
# if this fails, stop this track and open a follow-up track `historical-adr-lint-resolution` —
# historical ADR edits are explicitly out of scope for tddd-04 (per ADR 2026-04-14-0625 Consequences Negative
# and spec.json out_of_scope).
```

### 8. 最終 CI gate (T007)

```bash
cargo make fmt                 # ensure normalized formatting
cargo make ci                  # fmt-check + clippy + test + deny + check-layers + verify-*
# expect: all tasks pass
```

## Result

Pending implementation. Rows will be checked off sequentially as T001 → T007 are
committed via `/track:full-cycle`.

### Open Issues

- 以下 2 つの historical ADR に旧名参照が含まれる (2026-04-14 時点の実測): (a) `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` に 33 箇所の `Finding::error` / `Finding::warning` pseudo-code, (b) `knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md` に 8 箇所の `Finding` struct 定義 / `Vec<Finding>` / `impl Finding` を含む Rust code block。T007 の `verify-arch-docs` dry-run で lint エラーになる場合、本 track では対処せず、follow-up track `historical-adr-lint-resolution` を立てて個別対応する (本 track の scope は Finding rename のみで historical ADR 編集は含まれない)。
- `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` は tddd-01 track (既に Done) の artifact だが live catalogue source として機能しているため、本 track (tddd-04) が更新対象に含める。将来 tddd-* catalogue が共有 location に移動した場合、この cross-track 更新パターンは再評価する。

## verified_at

Not yet verified. Will be stamped on T007 completion by `/track:commit`.
