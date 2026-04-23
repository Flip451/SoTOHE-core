<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# sotp track branch create: main 上 activation commit bug fix (switch-before-commit)

## Summary

`execute_branch(BranchAction::Create)` と `execute_activate` の code path を切り離し、branch create が activation commit を main 上に生成する regression を構造的に排除する (ADR: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md §D1-D3)
T001: activate.rs 単一ファイル内で実装変更 (D1 path 分離 + D2 BranchMode::Create 退役) + ユニットテスト追加/更新を 1 commit にまとめる (コンパイル可能 + テスト green を各 commit で維持)
T002: cargo make ci 全体 gate を通過させ、回帰がないことを確認して track を完了状態にする
T003: /track:init を 3 step 構成 (branch 作成 + switch → metadata.json 作成 → verify-track-metadata) に整理し、Makefile.toml description 更新。execute_activate / sotp track activate は無変更。PR review で検出された P1 finding (workflow 破綻) に対応する

## Tasks (3/3 resolved)

### S1 — S1 — branch create path 分離 + BranchMode::Create 退役 + テスト (D1/D2/D3)

> execute_branch(BranchAction::Create) を execute_activate(BranchMode::Create) への forward から切り離し、単純な git switch -c track/<id> main のみを実行する独立関数 / コードパスとして実装する
> execute_activate は BranchMode::Switch / BranchMode::Auto のみを受け付けるように制限し、BranchMode::Create を経由した呼び出しが存在しないようにする
> 新 Create path が git commit を一切呼ばないこと、BranchMode::Auto / BranchMode::Switch の既存テストが引き続き pass することを単体テストで確認する
> no panics in library code 原則 (CN-01) および BranchMode::Auto resume flow 維持 (CN-02) を遵守する

- [x] **T001**: apps/cli/src/commands/track/activate.rs: separate execute_branch(BranchAction::Create) from execute_activate into an independent path that runs only `git switch -c track/<id> main` with no commit side-effects; retire BranchMode::Create from execute_activate (Switch/Auto only); update rstest callers that reference BranchMode::Create; add unit tests verifying the new Create path produces no git commit call and that existing Auto/Switch execute_activate tests continue to pass (`34d0214cac2318e32cebda5a3f442893a49fe626`)

### S2 — S2 — CI gate 全体確認 (CN-03 / AC-04)

> cargo make ci (fmt / clippy / nextest / deny / verify-* 一式) を実行して全チェックが pass することを確認する
> T001 実装後の回帰 (clippy lint / フォーマット崩れ / 既存テスト失敗) を検出して報告する

- [x] **T002**: Run `cargo make ci` (fmt / clippy / nextest / deny / verify-* suite) and confirm all checks pass; report any regressions introduced by T001 (`33de49a72c4fed1f5a0e05befeeb952b36d03850`)

### S3 — S3 — /track:init workflow 完成 (IN-04 / AC-05)

> .claude/commands/track/init.md を 3 step 構成 (1. `cargo make track-branch-create` で branch 作成 + switch、2. `metadata.json` を `branch: "track/<track-id>"` で作成、3. `cargo make verify-track-metadata`) に整理する
> Makefile.toml の `track-branch-create` description を branch 作成 + switch のみ (no metadata, no commit) に更新する
> `execute_activate` / `sotp track activate` は無変更。`cargo make ci` で全チェックが pass することを確認する

- [x] **T003**: Rewrite `.claude/commands/track/init.md` to 3 steps (`cargo make track-branch-create` → create metadata.json with `branch: "track/<track-id>"` → `cargo make verify-track-metadata`). Update `Makefile.toml` `[tasks.track-branch-create]` description to 'branch only; no metadata, no commit'. Leave `execute_activate` / `sotp track activate` unchanged. Run `cargo make ci`. (`268ab2f60d52d15d303d987f2ef4cc0d0ae41f00`)
