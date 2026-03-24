<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# knowledge/strategy/ 移動 — 戦略文書を git 管理下に

tmp/（gitignore 済み）にある戦略的 SSoT 文書を knowledge/strategy/ に移動し git 管理下に置く。
日付サフィックスを除去して SSoT 化。旧版は tmp/ に残す（git 履歴で参照可能）。
ADR 自動導出（sotp adr suggest）のスキャン対象にするための前提条件。

## ディレクトリ基盤

T001: knowledge/strategy/README.md を作成。
ディレクトリの目的（戦略的 SSoT の永続化）とファイル一覧を記載。

- [ ] knowledge/strategy/README.md 作成（ディレクトリ目的 + ファイル一覧）

## ファイル移動

T002: 5 ファイルを移動。
tmp/TODO.md → knowledge/strategy/TODO.md
tmp/TODO-PLAN-2026-03-22.md → knowledge/strategy/TODO-PLAN.md
tmp/vision-2026-03-22.md → knowledge/strategy/vision.md
tmp/progress-tracker-2026-03-22.md → knowledge/strategy/progress-tracker.md
tmp/TODO-PLAN-v4-draft-2026-03-23.md → knowledge/strategy/TODO-PLAN-v4-draft.md
.gitignore に !knowledge/strategy/ 除外ルールを追加（tmp/ は gitignore だが knowledge/ は git 管理対象）。

- [ ] 戦略文書 5 ファイルを tmp/ → knowledge/strategy/ に移動 + 日付サフィックス除去 + .gitignore 除外設定

## 参照修正

T003: 旧 tmp/ パス参照を全リポジトリから検索・修正。
対象: 移動ファイル内の相互参照 + knowledge/adr/*.md + project-docs/conventions/*.md + その他 git tracked ファイル。
Grep で tmp/TODO, tmp/vision, tmp/progress-tracker, tmp/TODO-PLAN を検索し全て更新（track/items/ 配下の計画文書は除外 — 計画文書は旧パスを意図的に文書化している）。
T004: CLAUDE.md + memory + knowledge/adr/README.md の参照を更新。

- [ ] 全リポジトリの旧 tmp/ パス参照を knowledge/strategy/ パスに修正（移動ファイル内 + ADR + conventions + その他 git tracked ファイル）
- [ ] CLAUDE.md + memory + knowledge/adr/README.md 参照更新
