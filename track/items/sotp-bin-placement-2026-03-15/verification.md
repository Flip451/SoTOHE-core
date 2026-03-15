# Verification: sotp バイナリを bin/sotp に配置

## 自動検証

- [x] `bin/sotp` と `target/release/sotp` を両方削除した状態で `cargo make bootstrap` → `bin/sotp` が再生成されること（clean-artifact bootstrap）
- [x] `cargo make ci` 通過

## 手動検証

- [x] `bin/sotp --version` が正常に実行できること
- [x] `bin/sotp` が `.gitignore` に含まれ git 管理外であること
- [x] Makefile.toml のホスト側 wrapper タスク（非 `-local`）が `bin/sotp` を使用しており、`cargo run --quiet -p cli` が残っていないこと
- [x] `.claude/settings.json` の hook が `$CLAUDE_PROJECT_DIR/bin/sotp` 存在時に優先し、不在時に `sotp` にフォールバックすること
- [x] `bin/sotp` を削除した状態でも hook が フォールバックで動作すること（bootstrap デッドロック回避）
- [x] `build-sotp` タスクが Docker compose ではなくホスト側の `cargo build` を使用していること
- [x] `SOTP_CLI_BINARY` 環境変数による override が引き続き機能すること
- [x] `bin/.gitkeep` が git 管理下にあること

## 対象外（TODO に記載済み）

- ERR-13: コンテナ内 `-local` タスクの `cargo run` 置換
- ERR-14: `_agent_profiles.py` の `fast_model` バリデーション

## 結果

- verified_at: 2026-03-15
- 結果: 全検証項目パス。cargo make ci 通過。
