<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Review Escalation Threshold の機構化 (WF-36)

レビューサイクルで同質の findings が 3 連続した場合に record-round をハードブロックし、Workspace Search + Reinvention Check を強制するサーキットブレーカー機構を ReviewState に組み込む

## Phase 1: Domain 層の型定義と状態遷移

ReviewConcern newtype、ReviewEscalationState、streak ロジック、ReviewState 拡張、resolve_escalation

- [x] Domain: ReviewConcern newtype + バリデーション（空文字拒否、slug 正規化）
- [x] Domain: ReviewEscalationState 型定義 + streak 更新ロジック + threshold 判定
- [x] Domain: ReviewState に escalation フィールド追加、record_round / record_round_with_pending / check_commit_ready にブロックチェック、新 ReviewError バリアント
- [x] Domain: resolve_escalation メソッド — ReviewEscalationResolution 全フィールド（blocked_concerns, workspace_search_ref, reinvention_check_ref, decision, summary, resolved_at）の検証 + EscalationPhase::Clear 遷移 + ReviewStatus::Invalidated + code_hash クリア

## Phase 2: Usecase 層のカテゴリ正規化

ReviewFinding に category 追加、finding → ReviewConcern 変換、reviewer output schema 拡張

- [x] Usecase: ReviewFinding に category フィールド追加 + finding → ReviewConcern 正規化ロジック（category → file パス → 'other' の 3 段フォールバック）
- [x] Usecase: Reviewer output schema に category フィールド追加（Optional、後方互換）+ ReviewFinding カスタム Visitor/deny_unknown_fields の category 対応更新

## Phase 3: Infra/CLI 層の統合

metadata.json スキーマ拡張、record-round に concerns 引数追加、resolve-escalation サブコマンド新設

- [x] Infra/Codec: metadata.json スキーマ拡張 — escalation セクションの serialize/deserialize + ReviewRoundDocument に concerns 追加（#[serde(default)] で既存データとの後方互換を維持）+ 既存 review エントリに escalation フィールド不在時はデフォルト Clear を適用
- [x] CLI: record-round に --concerns 引数追加 + エスカレーションブロック時のメッセージ出力
- [x] CLI: sotp review resolve-escalation サブコマンド新設（証拠ファイルパス検証 + 解除実行）

## Phase 4: ドキュメント更新

10-guardrails.md をプロンプト指示から機構参照に更新

- [x] Docs: 10-guardrails.md をプロンプト指示から機構参照に更新 + convention doc 追加
