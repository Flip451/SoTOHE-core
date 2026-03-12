# Spec: Atomic Write Standardization

## Goal

Rust の `atomic_write_file` を CLI サブコマンドとして公開し、残りの Python スクリプトのファイル書き込みも crash-safe にする。

## Scope

### In scope

- **CLI layer** (`apps/cli/src/commands/`):
  - `sotp file write-atomic --path <path>` — stdin からコンテンツを読み、アトミックに書き込み
  - `infrastructure::track::atomic_write_file` を再利用
- **Python scripts** (`scripts/`):
  - `external_guides.py` の `save_registry()` を `sotp file write-atomic` に委譲
  - `track_markdown.py` の plan.md / registry.md 書き込みを委譲
- 検証: `FsTrackStore` が既に metadata.json をアトミック書き込みしていることの確認

### Out of scope

- ログファイル（append モード、アトミック書き込み不要）
- バイナリファイル

## Constraints

- `atomic_write_file` は filelock-migration トラックで実装済みであること（依存関係）
- stdin から読む最大サイズは 10MB（metadata.json / registry 用途では十分）
- exit 0 = 成功、exit 1 = 失敗

## Acceptance Criteria

1. `sotp file write-atomic --path <path>` が stdin からアトミック書き込みを実行
2. `external_guides.py` が `sotp file write-atomic` を使用
3. `track_markdown.py` が `sotp file write-atomic` を使用
4. 書き込み中断シミュレーションで部分ファイルが残らない
5. `cargo make ci` が全チェック通過

## Resolves

- TODO SSoT-01: JSON 更新のアトミック性欠如（部分対応）
