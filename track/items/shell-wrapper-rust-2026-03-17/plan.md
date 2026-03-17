<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-09: shell wrapper / cargo make 依存の縮退

Makefile.toml の script_runner="@shell" ラッパーを sotp CLI サブコマンドに集約し、quoting 脆弱性・追跡困難性・条件分岐の脆さを根本解決する

## Phase 1: sotp make サブコマンド基盤

- [x] sotp make サブコマンド基盤: clap 定義と MakeCommand enum の追加
- [x] sotp make ディスパッチ機構: 既存 sotp サブコマンドへの内部転送

## Phase 2: 高優先度マイグレーション (quoting/安全性)

- [x] 高優先度: commit タスクの Rust 化 (sotp make commit — CI + git commit をアトミックに)
- [x] 高優先度: note タスクの Rust 化 (sotp make note — git notes add を安全に)
- [x] 高優先度: track-commit-message の Rust 化 (sotp make track-commit-message — CI + commit-from-file)
- [x] 高優先度: track-transition/add-task/set-override の Makefile 直接呼び出し化 (shell 引数パース除去)

## Phase 3: 中優先度マイグレーション (arg フォワーディング)

- [ ] 中優先度: track-branch-create/switch/activate の command 直接呼び出し化
- [ ] 中優先度: track-pr-* 系の command 直接呼び出し化
- [ ] 中優先度: track-plan-branch を sotp track branch plan サブコマンドとして新設
- [ ] 中優先度: 残りの薄い shell ラッパー (track-local-review, track-resolve 等) の command 直接呼び出し化

## Phase 4: -exec daemon ラッパー統一

- [ ] -exec daemon ラッパー: sotp make exec サブコマンドで WORKER_ID 処理を Rust 統一
- [ ] -exec タスク群の Makefile.toml command 形式への変換

## Phase 5: ドキュメント・CI 更新

- [ ] Makefile.toml の最終整理と script_runner=@shell 残存の監査
- [ ] ドキュメント更新: track/workflow.md, .claude/rules/07-dev-environment.md, DESIGN.md
