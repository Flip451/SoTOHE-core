# CI Verification Hardening

## Goal

TODO.md の CI 関連 4 件（WF-23, WF-09, WF-11, WF-13）を修正し、CI パイプラインの堅牢性と使い勝手を改善する。

## Scope

### WF-23: コンテナ CI エントリポイント (MEDIUM)

**課題**: `ci-local` / `ci-rust-local` が `private = true` のため、GitHub Actions の CI コンテナ内から `cargo make ci-local` を直接呼べない（`--allow-private` が必要）。

**対策**: 公開タスク `ci-container` / `ci-rust-container` を追加。CI ワークフローをこれに移行。

### WF-09: コードブロック内 TODO 誤検知 (LOW)

**課題**: `verify_latest_track_files.py` の `placeholder_lines()` がマークダウンのフェンスドコードブロック内の `TODO` を誤検知する。

**対策**: フェンスドコードブロック（``` マーカー）内の行をスキップするようリファクタ。

### WF-11: 計画フェーズでの tech-stack TODO ブロック (LOW)

**課題**: `verify_tech_stack_ready.py` が tech-stack.md の `TODO` で CI 全体をブロック。計画フェーズでは意図的に TODO が残る場合がある。

**対策**: 全トラックが `planned` ステータスなら TODO を許容。`in_progress` / `done` のトラックが存在すれば従来通りブロック。metadata 読取不可時は fail-closed。

### WF-13: verification.md 英語見出しハードコード (LOW)

**課題**: `VERIFICATION_SCAFFOLD_LINES` が英語のみ（"scope verified", "manual verification steps" 等）。verification.md は日本語可とドキュメント化されている。

**対策**: `scaffold_placeholder_lines()` のヘッディング行スキップロジックを修正し、scaffold パターンのマッチング対象にヘッディング行も含める。`normalize_scaffold_line()` を更新して `#` プレフィックスも除去する（現在はリストマーカーのみ除去）。日本語等価表現を `VERIFICATION_SCAFFOLD_LINES` に追加。

## Constraints

- Python スクリプト + Makefile.toml + GitHub Actions のみ変更（Rust コード変更なし）
- 既存 CI が引き続きパスすること（後方互換性）
- fail-closed ポリシー維持
- 全修正にテストを付与

## Acceptance Criteria

1. `cargo make ci-container` がコンテナ内で直接実行可能
2. フェンスドコードブロック内の `TODO` で `verify-latest-track` が失敗しない
3. 全トラック `planned` 時に tech-stack.md の TODO で CI がブロックされない
4. 日本語の verification.md scaffold 見出しが正しく検出される
5. 全既存テスト + 新規回帰テストがパス
6. `cargo make ci` がパス
