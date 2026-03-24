# Strategy Documents

プロジェクトの戦略的 SSoT（Single Source of Truth）文書を管理するディレクトリ。

## Purpose

- プロジェクトのロードマップ、ビジョン、進捗を永続的に記録する
- git 管理下に置くことで clone した人にも見える状態にする
- ADR 自動導出（`sotp adr suggest`）のスキャン対象にする

## Files

| File | Description |
|------|-------------|
| `TODO.md` | 全 TODO 項目の詳細リスト（カテゴリ別） |
| `TODO-PLAN.md` | ロードマップ（Phase 構成 + 依存関係 + 見積もり） |
| `vision.md` | プロジェクトビジョン（ハーネス vs テンプレート出力の区別） |
| `progress-tracker.md` | 進捗管理（Gantt + バーンダウン + 完了ログ） |
| `TODO-PLAN-v4-draft.md` | sotp CLI 外部ツール化のドラフト計画 |

## Rules

- ファイル名に日付サフィックスを付けない（git 履歴で変更日を管理）
- 旧版は `tmp/` に残す（gitignore 済みのローカルアーカイブ）
- 変更時は git commit で記録する（PR diff で変更が見える）
