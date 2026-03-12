# Verification: Track Branch Enforcement Guard

## Scope Verified

- [ ] `TrackBranch` value object が `track/<slug>` 形式をバリデートする
- [ ] `TrackMetadata` に `branch` フィールドが存在し、codec で round-trip する
- [ ] `sotp track transition` が間違ったブランチで拒否する
- [ ] `verify_track_branch()` が branch mismatch を検出する
- [ ] `transition_task()` が間違ったブランチで拒否する
- [ ] `commit_from_file()` が間違ったブランチで拒否する
- [ ] cargo make tasks が track context を正しく渡す
- [ ] テスト時のガードスキップが動作する
- [ ] `cargo make ci` が全チェック通過する

## Manual Verification Steps

- [ ] `track/<id>` ブランチで `cargo make track-transition` を実行 → 成功
- [ ] `main` ブランチで同トラックの `cargo make track-transition` を実行 → 拒否
- [ ] `track/<id>` ブランチで `cargo make track-commit-message` を実行 → 成功
- [ ] `main` ブランチで同トラックの `cargo make track-commit-message` を実行 → 拒否

## Result / Open Issues

(TBD after implementation)

## verified_at

(TBD)
