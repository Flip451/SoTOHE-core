<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
---

# CC-SDD-02 明示的承認ゲート（spec.json approved_at + 自動降格）

## Goal

spec.json の status フィールドを String から SpecStatus enum (Draft/Approved) に型昇格し、明示的な承認ゲートを導入する。
approved_at タイムスタンプと content_hash による自動降格で、承認後の仕様変更を検出する。

## Scope

### In Scope
- SpecDocument の status を String から SpecStatus enum (Draft, Approved) に型昇格 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- SpecDocument に approved_at: Option<Timestamp> フィールドを追加 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- SpecDocument に content_hash: Option<String> フィールドを追加（承認時のコンテンツハッシュ保持用） [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- approve() / is_approval_valid() / effective_status() メソッドを SpecDocument に実装 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- spec/codec.rs で SpecStatus + approved_at + content_hash を serialize/deserialize [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T002]
- infrastructure 層で SHA-256 content hash 計算ロジックを実装 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T002]
- codec decode 時に content_hash 不一致で auto-demote する [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T002]
- spec/render.rs で承認ステータスバッジと approved_at を表示 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T003]
- sotp spec approve <track-dir> CLI コマンドを追加 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T004]
- Makefile.toml に cargo make spec-approve / track-record-round / track-check-approved ラッパーを追加し、permissions.allow に登録する [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T005]
- /track:plan skill の spec.json 生成後に承認フロー案内を追加 [source: knowledge/strategy/TODO-PLAN.md §Phase 2 2-4] [tasks: T005]
- DESIGN.md に CC-SDD-02 の設計決定（SpecStatus enum, content hash auto-demotion）を記録 [source: convention — .claude/docs/DESIGN.md] [tasks: T006]
- TRACK_TRACEABILITY.md に spec 承認ステータスの更新ルールを追記 [source: convention — TRACK_TRACEABILITY.md] [tasks: T006]
- 統合テストで承認→コンテンツ変更→自動降格の end-to-end フローを検証 [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T006]

### Out of Scope
- CI ゲートとしての verify-spec-approved（follow-up track） [source: discussion — 2026-03-24 CC-SDD-02 計画]
- /track:implement での承認チェックブロック強制（follow-up track） [source: discussion — 2026-03-24 CC-SDD-02 計画]
- 既存 completed tracks の spec.json への retroactive 更新 [source: inference — historical artifacts are immutable records]

## Constraints
- domain 層は I/O を含まない（hexagonal purity）。SHA-256 計算は infrastructure 層で行う [source: convention — project-docs/conventions/hexagonal-architecture.md]
- spec.json schema_version は 1 のまま（新フィールドは後方互換な Option） [source: inference — backward compatibility with existing spec.json files]
- TDD (Red-Green-Refactor) に従う [source: convention — .claude/rules/05-testing.md]
- domain 層に SHA-256 クレート依存を追加しない。hash 値は String として受け渡す [source: convention — project-docs/conventions/hexagonal-architecture.md]

## Domain States

| State | Description |
|-------|-------------|
| Draft | 仕様作成中、または承認後にコンテンツが変更され自動降格した状態 |
| Approved | 明示的に承認済み。content_hash が一致する間のみ有効。コンテンツ変更で Draft に自動降格する |

## Acceptance Criteria
- [ ] SpecDocument の status が SpecStatus enum (Draft/Approved) で表現される [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- [ ] approve() で status=Approved, approved_at 設定, content_hash 設定ができる [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- [ ] content_hash 不一致時に effective_status() が Draft を返す [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T001]
- [ ] spec.json で approved_at と content_hash が JSON として永続化・復元できる [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T002]
- [ ] codec decode 時に auto-demote が動作する（approved だが hash 不一致 → draft に降格） [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T002]
- [ ] spec.md に承認ステータスと approved_at が表示される [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T003]
- [ ] sotp spec approve がコマンドとして動作し spec.json を更新する [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T004]
- [ ] cargo make spec-approve / track-record-round / track-check-approved が permissions.allow に登録され許可プロンプトなしで動作する [source: discussion — 2026-03-24 レビュー中の許可プロンプト問題] [tasks: T005]
- [ ] /track:plan skill が spec.json 生成後に承認フロー（sotp spec approve）の案内を出力する [source: knowledge/strategy/TODO-PLAN.md §Phase 2 2-4] [tasks: T005]
- [ ] DESIGN.md に CC-SDD-02 の設計決定が記録されている [source: convention — .claude/docs/DESIGN.md] [tasks: T006]
- [ ] TRACK_TRACEABILITY.md に spec 承認ステータスの更新ルールが追記されている [source: convention — TRACK_TRACEABILITY.md] [tasks: T006]
- [ ] 統合テストで承認→コンテンツ変更→自動降格の end-to-end フローが検証される [source: discussion — 2026-03-24 CC-SDD-02 計画] [tasks: T006]
- [ ] cargo make ci が全テスト通過する [source: convention — .claude/rules/07-dev-environment.md] [tasks: T006]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md
- project-docs/conventions/typed-deserialization.md
- .claude/rules/05-testing.md
- .claude/rules/07-dev-environment.md

