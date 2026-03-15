# Verification: sotp バイナリを bin/sotp に配置

## 自動検証

- [ ] `bin/sotp` と `target/release/sotp` を両方削除した状態で `cargo make bootstrap` → `bin/sotp` が再生成されること（clean-artifact bootstrap）
- [ ] `cargo make ci` 通過

## 手動検証

- [ ] `bin/sotp --version` が正常に実行できること
- [ ] `bin/sotp` が `.gitignore` に含まれ git 管理外であること
- [ ] Makefile.toml のホスト側 wrapper タスク（非 `-local`）が `bin/sotp` を使用しており、`cargo run --quiet -p cli` が残っていないこと
- [ ] `.claude/settings.json` の hook が `$CLAUDE_PROJECT_DIR/bin/sotp` 存在時に優先し、不在時に `cargo run` にフォールバックすること
- [ ] `bin/sotp` を削除した状態でも hook が `cargo run` フォールバックで動作すること（bootstrap デッドロック回避）
- [ ] `build-sotp` タスクが Docker compose ではなくホスト側の `cargo build` を使用していること
- [ ] `SOTP_CLI_BINARY` 環境変数による override が引き続き機能すること
- [ ] `bin/.gitkeep` が git 管理下にあること

## 結果

- verified_at: (未実施)
- 結果: (未実施)
