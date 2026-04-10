# START HERE (Human)

このファイルは「人間が最初に読む運用導線」です。
AI が強く自動化されているため、先に責務境界と禁止操作を確認してください。

## 1. 最短オンボーディング

1. `DEVELOPER_AI_WORKFLOW.md` の 0 章を読む
2. `track/workflow.md` の Guiding Principles と Task Workflow を読む
3. レビューや運用判断が必要なときは `TRACK_TRACEABILITY.md` の 2章（対応付けルール）と 4章（Interactive Implementation Contract）を参照する
4. 初回は `cargo make build-tools` を実行し、その後 `/track:setup` を実行する

## 2. 人間と AI の責務境界

- 人間が決めること:
  - スコープ、優先順位、受け入れ条件
  - 計画承認（`/track:plan` 後）
  - 最終コミット可否（`/track:commit` 前）
- AI が担当すること:
  - `/track:*` フロー内の実装、検証、ドキュメント更新
  - Claude Code / Agent Teams / Rust CLI を使った実装・レビューの実行オーケストレーション

## 3. 必須レビュー・承認ポイント

1. `/track:plan <feature>` 後
   - `track/items/<id>/spec.md` と `plan.md` を人間が承認してから実装へ進む
2. `/track:review` または `cargo make ci` 完了後
   - 重大指摘がないこと、`verification.md` が更新されていることを確認する
3. `/track:commit <message>` 前
   - 差分、検証結果、必要な note を確認してから実行する

## 4. 人間が修正してよい対象（原則）

判断を速くするため、まず「編集可」の対象を固定します。

通常編集してよい:

- `architecture-rules.json` の `layers[].path` で定義された workspace member 配下（例: 初期状態では `apps/**`, `libs/**`）
- `track/**`
- `docs/**`
- `project-docs/**`
- `.github/workflows/**`
- `tmp/**`（作業メモ・レポート）
- ルート直下の運用ドキュメント（例: `START_HERE_HUMAN.md`, `LOCAL_DEVELOPMENT.md`, `DEVELOPER_AI_WORKFLOW.md`）
- ルート設定ファイル（例: `Cargo.toml`, `Makefile.toml`, `deny.toml`, `compose*.yml`, `.gitignore`, `Dockerfile`）

条件付きで編集してよい:

- `scripts/**`（CI・ガードレールの変更時のみ。変更後は関連 selftest/CI を通す）
- `.claude/commands/**`, `.claude/docs/**`, `.claude/skills/**`, `.claude/rules/**`（AI 運用フローを変更する場合のみ）
- `.claude/agents/**`（エージェント役割定義を変更する場合のみ）
- `CLAUDE.md`（保守者向け運用リファレンス。運用方針を更新する場合のみ）
- `rustfmt.toml`（整形ポリシー変更時のみ）
- `Cargo.lock`（`cargo` 実行結果として更新された差分のみ受け入れる）
- `.harness/config/agent-profiles.json`, `.claude/settings*.json`（運用ポリシー変更時のみ）
- `.codex/instructions.md`, `.gemini/GEMINI.md`（利用エージェント設定を見直す場合のみ）

人間が手動編集しない:

- `target/**`（ビルド生成物）
- `.cache/**`（外部ガイドのキャッシュ）
- `.claude/logs/**`（実行ログ）

例外:
- 障害復旧で緊急に編集する場合は、理由を `verification.md` に残す

## 5. 安全運用ルール

- 実装とレビューの正規導線は `/track:*` と `cargo make track-*` の補助 wrapper に限定する
- 直接 `git add` / `git commit` は使わず、`/track:commit` か `cargo make commit` を使う
- `cargo make ci` が落ちた状態でコミットしない
