---
status: draft
version: "1.0"
---

# Phase 1: sotp CLI Hardening — Specification

## Feature Goal

sotp CLI の堅牢性を Phase 1 レベルに引き上げる。データロスバグ修正、入力バリデーション強化、テストファイル保護 hook 追加、タスク説明 immutability 強制、spec.md 構造検証を実施する。

## Scope

### In Scope

- `TrackDocumentV2` の未知フィールド保持（`#[serde(flatten)]`）
- `collect_track_branch_claims` の skip-and-warn 化
- `resolve_track_id_from_branch` の `TrackId` バリデーション追加
- `parse_body_findings` の番号付きリスト・`+` プレフィックス対応
- `block-test-file-deletion` PreToolUse hook（Rust 実装）
- タスク説明 immutability バリデーション（save 時）
- `sotp verify spec-attribution` サブコマンド
- `sotp verify spec-frontmatter` サブコマンド

### Out of Scope

- spec.md テンプレートの自動生成（AI 生成のまま）
- hook profile（minimal/standard/strict）の導入
- cross-artifact 整合性分析（Phase 3）

## Constraints

- 全変更は sotp CLI（Rust）内に閉じる。Python への新規投資なし [source: feedback memory — feedback_rust_first.md]
- 既存テストの破壊なし。新規コードは TDD で実装 [source: feedback memory — feedback_tdd_enforcement.md]
- `docs/architecture-rules.json` のレイヤー制約を遵守
- hook は fail-closed（PreToolUse エラー時は exit 2）

## Acceptance Criteria

1. **T001**: 未知フィールドを含む JSON が decode → encode で未知フィールドを保持する（round-trip テスト）
2. **T002**: 1 つの metadata.json が破損していても、他のトラックの走査が正常に完了する（stderr に警告出力）
3. **T003**: `track/` プレフィックス除去後の文字列が `TrackId::new()` で不正な場合、`InvalidTrackId` エラーを返す
4. **T004**: 番号付きリスト（`1. finding`）と `+ finding` を findings として認識する
5. **T005**: テストファイル（`*_test.rs`, `test_*.rs`, `tests/` 配下）の削除操作が hook でブロックされる
6. **T006**: 既存タスクの description を変更して save しようとするとエラーが返る
7. **T007**: spec.md の要件行（`### S-` または `### REQ-` プレフィックスで始まる見出し行）に `[source: ...]` タグがない場合にエラーを報告する。要件行以外の行（スコープ、制約、目標等）は検証対象外。要件行がゼロの spec.md はパスとする
8. **T008**: spec.md に YAML frontmatter（`status`, `version` 必須）がない場合にエラーを報告する
