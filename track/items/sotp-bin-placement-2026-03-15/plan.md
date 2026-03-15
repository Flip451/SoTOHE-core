<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# sotp バイナリを bin/sotp に配置し cargo run を排除

sotp CLI バイナリのビルド成果物を bin/sotp に配置し、Makefile.toml のホスト側 wrapper タスクの cargo run --quiet -p cli 呼び出しと hook の SOTP_CLI_BINARY デフォルトを bin/sotp に統一する。
bootstrap タスクで自動ビルド+配置し、テンプレート利用者は cargo make bootstrap 後すぐに bin/sotp を使える。

## bin/ ディレクトリと .gitignore 設定

bin/ ディレクトリを作成し、.gitignore に /bin/sotp を追加。bin/.gitkeep でディレクトリ構造を git 管理する。

- [x] bin/.gitkeep を作成。.gitignore に /bin/sotp を追加（bin/.gitkeep は追跡対象のまま）。

## build-sotp タスク追加と bootstrap 統合

Makefile.toml に build-sotp タスク（cargo build -p cli --release + cp）を追加し、bootstrap の依存に組み込む。

- [x] Makefile.toml に build-sotp タスクを追加: ホスト側で cargo build -p cli --release を実行し、target/release/sotp を bin/sotp にコピー。bootstrap スクリプト内の適切な位置で build-sotp を呼び出す。bootstrap の前提条件チェックに cargo/rustc の存在確認を追加する。前提: テンプレート利用者はホスト側に Rust toolchain を持つ（Rust テンプレートのため妥当）。

## Makefile.toml の cargo run 置換

ホスト側 wrapper タスク（非 -local）の cargo run --quiet -p cli -- を bin/sotp に置換する。コンテナ内 -local タスクは対象外。

- [x] Makefile.toml 内のホスト側 wrapper タスク（非 -local）の 'cargo run --quiet -p cli --' を 'bin/sotp' に置換する。コンテナ内 -local タスクは対象外（引き続き cargo build 成果物を直接使用）。

## hook の SOTP_CLI_BINARY デフォルト変更

.claude/settings.json の SOTP_CLI_BINARY デフォルトを bin/sotp に変更する。

- [x] .claude/settings.json の hook コマンドを変更: SOTP_CLI_BINARY env override は維持。デフォルトのフォールバックチェーンを SOTP_CLI_BINARY → $CLAUDE_PROJECT_DIR/bin/sotp → cargo run --quiet -p cli にする。パスは $CLAUDE_PROJECT_DIR で project root に固定し cwd 非依存にする。これにより fresh clone でも bootstrap 前に hook が動作する。

## ドキュメントと検証スクリプト更新

SOTP_CLI_BINARY 参照箇所のドキュメント更新、verify_orchestra_guardrails.py のスニペット更新。

- [x] SOTP_CLI_BINARY 参照箇所のドキュメント更新（.claude/docs/DESIGN.md 等）。verify_orchestra_guardrails.py と test_verify_scripts.py のスニペット期待値更新。scripts/test_make_wrappers.py の全ホスト側 wrapper タスク body 期待値を bin/sotp に更新し、未カバーの wrapper（track-branch-create, track-branch-switch, track-activate, track-resolve, track-pr-push, track-pr-ensure, track-pr, track-transition, track-sync-views 等）のテストも追加する。

## CI 検証

- [x] cargo make ci が通ることを確認。bin/sotp が存在しない状態での bootstrap → CI フローを検証。
