<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 10, yellow: 4, red: 0 }
---

# review.json 分離とグループ独立レビュー

## Goal

レビュー状態を metadata.json から分離し、review.json に cycle/round 履歴として保持する。
各 review group が独立に進行し、zero_findings 済みの group が他 group の再試行に巻き込まれない review model を実現する。
track/review-scope.json を policy source として維持しつつ、per-group hash と partition freeze に基づく stale 判定へ移行する.

## Scope

### In Scope
- review.json schema_version 1 を新規トラック向けに導入し、cycle 単位の base_ref/policy_hash/scope 固定と group 単位の round append-only 履歴を定義する。スキーマサンプルは ADR-2026-03-29-0947 § Schema Samples を正本とする [source: ADR-2026-03-29-0947, knowledge/strategy/TODO.md RVW-21] [tasks: T001, T002]
- track/review-scope.json を project-wide base policy とし、groups セクション（named group → glob patterns マッピング）を T003 実装時に追加する（既存ツールとの互換のため計画段階ではファイルに含めない）。per-track override は optional な track/items/<track-id>/review-groups.json で groups のみ上書き可能とする。mandatory other は全 named group の補集合として暗黙導出する。frozen partition を review cycle 開始時に確定する。default groups は domain（libs/domain）、usecase（libs/usecase）、infrastructure（libs/infrastructure）、cli（apps）、harness-policy（.claude/commands, .claude/rules, agent-profiles, settings, conventions, AGENTS.md, CLAUDE.md）の 5 named groups + other を想定 [source: ADR-2026-03-29-0947, knowledge/strategy/TODO.md RVW-24] [tasks: T003, T004]
- record-round と check-approved を per-group latest-success hash 判定に置き換え、group 間 round 一致要件を廃止する [source: ADR-2026-03-29-0947, knowledge/strategy/TODO.md RVW-31] [tasks: T005, T006]
- metadata.json から review state を除去し、status / stale reason / final 必須判定を review.json ベースに移行する [source: ADR-2026-03-29-0947, knowledge/strategy/TODO.md RVW-32] [tasks: T002, T006, T007]
- stale reason を policy_changed / partition_changed / hash_mismatch として表示し、review status CLI とテストを更新する [source: ADR-2026-03-29-0947] [tasks: T007, T008]

### Out of Scope
- review verdict provenance / attestation / write-guard による tamper-proof 化（tamper-proof-review が本トラック完了後に schema evolution として対応する） [source: track/items/tamper-proof-review-2026-03-26/spec.json]
- 旧 track の metadata.review から新 review.json への自動 migration や後方互換の維持（本トラックは新規トラック専用） [source: ADR-2026-03-29-0947]
- group 間で同一 round 番号を揃える global synchronization の再導入 [source: ADR-2026-03-29-0947]

## Constraints
- review policy と review state を分離し、track/review-scope.json は canonical policy source として残すこと [source: ADR-2026-03-29-0947]
- named groups の glob patterns は非重複でなければならない。あるパスが複数の named group にマッチした場合は fail-closed でエラーとすること [source: ADR-2026-03-29-0947]
- other group を必須とし、group partition の対象は review_operational / planning_only / other_track を除外した後の差分（TrackContent + Implementation）とする。other group はそのうち named groups に属さないパスの補集合として常に存在すること [source: ADR-2026-03-29-0947]
- final は全 group 必須とし、optional final group は導入しないこと [source: ADR-2026-03-29-0947]
- 新規ロジックは Rust 実装とし、domain/usecase/infrastructure の責務分離を維持すること [source: convention — project-docs/conventions/hexagonal-architecture.md]
- review-scope.json の metadata.json normalize rule（remove_fields: ["review"]）は defense-in-depth として残す。review 分離後は no-op だが、metadata.json への review state 再混入を防止する安全網として機能する [source: ADR-2026-03-29-0947]
- review.json は review_operational 分類のまま維持する。review_operational ファイルは review system 自体が管理する machine-managed ファイルであり、planning-only gate の通過は意図的設計。review.json の整合性は record-round / check-approved の domain logic が保証する [source: ADR-2026-03-29-0947]
- ハーネス挙動を変えるファイル（.claude/commands, .claude/rules, agent-profiles.json, settings*.json, project-docs/conventions, AGENTS.md, CLAUDE.md）は planning_only から除外し Implementation 扱いとする。groups 実装後は harness-policy group に割り当てる [source: ADR-2026-03-29-0947]
- 本トラックは新規トラック専用。旧トラックの review.json（schema_version 1 旧フォーマット）や metadata.json review state への後方互換・dual-read 対応は行わない [source: ADR-2026-03-29-0947]

## Domain States

| State | Description |
|-------|-------------|
| NoCycle | review.json が未作成の初期状態。review status は NotStarted、check-approved は planning-only 判定のみ通過可（既存動作を踏襲） |
| ReviewCycle | base_ref / policy_hash / frozen group scopes を保持する review 実行単位 |
| ReviewGroupRound | group ごとの round_type / verdict / group-scope hash を保持する append-only 履歴 |
| ReviewStalenessReason | PolicyChanged | PartitionChanged | HashMismatch |

## Acceptance Criteria
- [ ] 新規 review cycle 開始時に review.json が作成され、base_ref・policy_hash・frozen group scopes・mandatory other が記録される [source: ADR-2026-03-29-0947] [tasks: T001, T003, T004]
- [ ] record-round は review.json に group ごとの round を append-only で記録し、zero_findings 済み group は他 group の再試行で失効しない [source: ADR-2026-03-29-0947] [tasks: T005]
- [ ] check-approved は各 group の latest successful fast/final round を group-scope hash で検証し、group 間 round 一致を要求しない [source: ADR-2026-03-29-0947] [tasks: T006]
- [ ] metadata.json には review state が一切保存されず、review status / stale reason は review.json から導出される [source: ADR-2026-03-29-0947] [tasks: T002, T007]
- [ ] policy 変更、partition drift、group-scope hash mismatch の各ケースで stale reason が区別されて表示される [source: ADR-2026-03-29-0947] [tasks: T007, T008]
- [ ] named groups の glob patterns が重複する場合、partition 導出時に fail-closed でエラーとなる（non-overlapping partition の negative case） [source: ADR-2026-03-29-0947] [tasks: T003, T008]
- [ ] check-approved は、いずれかの group に successful final round が存在しない場合に fail を返す（final 必須の negative case） [source: ADR-2026-03-29-0947] [tasks: T006, T008]
- [ ] policy_changed / partition_changed / hash_mismatch で cycle が stale になった場合、新 cycle の開始が必要であり、旧 cycle の successful rounds は新 cycle の approval 判定に参入しない [source: ADR-2026-03-29-0947] [tasks: T004, T005, T006]
- [ ] review.json が未作成の新規トラックで review status は NotStarted を返し、check-approved は planning-only commit のみ許可する [source: ADR-2026-03-29-0947] [tasks: T002, T006, T008]
- [ ] cargo make ci が全チェック通過し、review workflow の主要シナリオに対する回帰テストが追加される [source: convention — task-completion-flow.md] [tasks: T008]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 4  🔴 0

