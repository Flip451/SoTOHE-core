<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TSUMIKI-05/06/07 ヒアリング UX 改善

Phase 2b: ヒアリング UX 改善。tsumiki の優れた UX パターンを取り込み、/track:plan の仕様ヒアリング品質を向上させる。
Phase 3（テスト生成パイプライン）の spec 入力品質に直結する投資。
SKILL.md プロンプト改修 + spec.json 軽微スキーマ拡張（hearing_history）。

## モード選択（TSUMIKI-06）

SKILL.md Phase 1 の前に Step 0 を挿入。AskUserQuestion で Full/Focused/Quick を選択させる。
Full: 全フェーズ実行（現行動作）。Focused: researcher/planner スキップ、差分ヒアリングのみ。Quick: Blue サマリー表示、自由記述変更のみ。
Focused/Quick モードでは Phase 1.5（planner review）と Phase 2（Agent Teams）をスキップする明示的例外を記載。
spec.json が存在しない場合は Full にフォールバック。

- [x] TSUMIKI-06 モード選択 — SKILL.md に Step 0 挿入（Full/Focused/Quick）

## HearingRecord スキーマ（TSUMIKI-07）

domain 層: HearingMode enum (Full/Focused/Quick), HearingSignalSnapshot, HearingSignalDelta, HearingRecord を追加。
infrastructure 層: HearingRecordDto, HearingSignalDeltaDto を追加。SpecDocumentDto に hearing_history フィールド追加。
render 層: render_hearing_history() で最新 5 件のテーブルを生成。
content_hash 計算から hearing_history を除外（approval 無効化防止）。
append-only: append_hearing_record() のみ、削除/変更メソッドなし。
TDD: domain 型テスト → codec roundtrip テスト → render テスト → content_hash 除外テスト。

- [x] TSUMIKI-07 HearingRecord — Rust domain + infra + render 実装（TDD）

## 構造化質問（TSUMIKI-05）

SKILL.md Step 4a の Markdown 壁を AskUserQuestion + multiSelect パターンに置換。
カテゴリ別バッチ（5 項目上限/回）: Yellow → Red → Missing の順。
Yellow: Confirm as-is / Modify / Remove。Red: コンテキストベースの選択肢 2-3 + Other。Missing: Add / Not needed / Need more info。
Modify 選択時は個別フォロー AskUserQuestion で新テキスト取得。
全項目 Blue の場合は Quick 相当にショートサーキット。
既存の spec.json 更新ロジック（source tagging rules）を維持。

- [x] TSUMIKI-05 構造化質問 — SKILL.md Step 4a を AskUserQuestion + multiSelect に書き換え

## CI 検証

cargo make ci で全チェック通過を確認。
既存テストの破壊がないこと。新規 Rust コード（T002）のカバレッジ 80% 以上。

- [ ] CI 検証 — cargo make ci 全チェック通過
