# Review Escalation Threshold の機構化 (WF-36)

## Goal

レビューサイクルで同一カテゴリの findings が 3 回連続した場合に、`record-round` をハードブロックし、
Workspace Search → Reinvention Check → Decision の 3 ステップを強制するサーキットブレーカー機構を実装する。

現状はプロンプト指示のみで遵守されないため、Rust の型システムと状態遷移で強制する。

## Scope

### In Scope

- `ReviewState` に `ReviewEscalationState` を合成し、concern streak を追跡
- `ReviewConcern` newtype で concern カテゴリを型安全に表現
- `record_round` / `record_round_with_pending` / `check_commit_ready` でエスカレーション中のハードブロック
- `resolve_escalation` メソッドで証拠（workspace search / reinvention check）を要求して解除
- `ReviewFinding` に `category` フィールド追加（Optional、後方互換）
- file パスからの concern 自動導出（フォールバック）
- metadata.json スキーマ拡張（`review.escalation` セクション、各ラウンドに `concerns` 追加）
- `sotp review resolve-escalation` CLI サブコマンド新設
- `10-guardrails.md` をプロンプト指示から機構参照に更新

### Out of Scope

- findings の全文保存（concern リストのみ保存）
- reviewer 側の category 自動分類 AI ロジック（reviewer はフィールドを出力するだけ）
- エスカレーション後の自動 Web 検索実行（人間/オーケストレーターが実行）
- PostToolUse hook による findings カテゴリ自動分類

## Constraints

- Domain 層は外部 workspace 依存ゼロ（`std` + `thiserror` のみ — 既存依存）
- 同期のみ（async 不可）
- typed deserialization 必須（`serde_json::Value` 手動走査禁止）
- レイヤー分離: domain→nothing, usecase→domain, infra→domain, cli→all
- metadata.json が SSoT
- `ReviewRoundResult` の既存シグネチャ変更は後方互換を考慮（既存 3-arg `new()` を維持）
- **Rust の代数的データ型（enum with data）を活用**: エスカレーション状態（Clear / Blocked）を enum variant のデータで表現し、不正な状態組み合わせを型レベルで排除する。`Option<ReviewEscalationBlock>` + 別フィールドの `status` ではなく、`enum EscalationPhase { Clear, Blocked(ReviewEscalationBlock) }` のように状態とデータを一体化する
- **concerns の正規化**: `Vec<ReviewConcern>` は格納前に重複排除 + ソート済み（`BTreeSet` 経由）とする。streak カウント・resolution matching はソート済みリスト同士の比較で行い、順序差異による不一致を排除する
- **streak リセットルール**: closed cycle 完了時に、その cycle の concerns に含まれない concern の streak は 0 にリセット（`concern_streaks` から除去）する。これにより `A → B → A → A` のような非連続パターンでは A の streak は 2（連続 2 回）となり、誤ったエスカレーションを防止する
- **concerns と verdict の整合性**: `zero_findings` ラウンドの concerns は空リスト必須、`findings_remain` ラウンドの concerns は 1 件以上必須。domain 層で `record_round` 時にバリデーションし、不整合の場合は `ReviewError::InvalidConcern` を返す

## Acceptance Criteria

- [ ] 同一 concern が 3 連続 closed cycle で出現 → 3 回目のラウンドは正常に記録されるが、`EscalationPhase` が `Blocked(ReviewEscalationBlock)` に遷移する
- [ ] エスカレーション `blocked` 中の `record_round` / `record_round_with_pending` / `check_commit_ready` は他のバリデーション（hash 検証、escalation 順序等）より先に `EscalationActive` チェックを短絡評価し、`Err(ReviewError::EscalationActive)` を返す。同様に `resolve_escalation` は `EscalationNotActive` を最初にチェックする
- [ ] `resolve_escalation` で `ReviewEscalationResolution` を受け取り、全必須フィールド（blocked_concerns, workspace_search_ref, reinvention_check_ref, decision, summary, resolved_at）を検証。空文字は拒否。`blocked_concerns` が現在の `EscalationPhase::Blocked` の concerns と一致しない場合は `Err(ReviewError::ResolutionConcernMismatch)` を返す
- [ ] `resolve_escalation` 後に `ReviewStatus::Invalidated` + `code_hash = None` + `EscalationPhase::Clear` にリセットし、`last_resolution` に resolution record が永続化される
- [ ] reviewer が `category` フィールドを出力しない場合でも file パスから concern を自動導出。category も file も null の場合は `other` にフォールバック
- [ ] metadata.json に `review.escalation` セクションが正しく serialize/deserialize される。`recent_cycles` は最大 10 件を FIFO で保持し、超過分は古い方から削除される
- [ ] 既存テスト（ReviewState の record_round / check_commit_ready）が全てパス
- [ ] `cargo make ci` がパス
