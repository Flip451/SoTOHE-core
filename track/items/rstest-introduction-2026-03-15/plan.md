<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# rstest 導入 — 全 crate パラメータ化テスト移行

rstest クレートを workspace 全体に導入し、繰り返しパターンの多いテストをパラメータ化テストに変換する。
全 4 crate (domain, usecase, infrastructure, cli) を対象とし、最大効果の guard/policy.rs (120 tests) から着手する。

## 技術決定・依存追加

tech-stack.md 更新と Cargo.toml への rstest 追加。実装タスクの前提条件。

- [x] track/tech-stack.md に rstest を追記し、ルート Cargo.toml の [workspace.dependencies] に rstest を追加。各 crate の Cargo.toml に dev-dependencies として追加。cargo make ci-rust で確認。

## domain crate テスト変換

最大効果の guard/policy.rs を中心に、domain crate のテストをパラメータ化。

- [x] libs/domain/src/guard/policy.rs の blocked テスト群 (~85) と allowed テスト群 (~20) を rstest #[case] でパラメータ化。ヘルパー関数テストも変換。
- [x] track_phase.rs (resolve_phase/resolve_phase_from_record マトリクス)、lib.rs の繰り返しパターンを rstest 化。guard/parser.rs も該当があれば変換。

## usecase crate テスト変換

track_resolution, hook, worktree_guard 等のパラメータ化。

- [x] track_resolution.rs の reject_branchless 系、hook.rs の resolve_lock_mode、worktree_guard.rs の parse_dirty_worktree_paths をパラメータ化。

## infrastructure + cli crate テスト変換

infrastructure と cli crate の該当テストをパラメータ化。

- [x] infrastructure (track/codec.rs, track/render.rs 等) と cli (commands/review.rs 等) の繰り返しテストパターンをパラメータ化。

## CI 検証

- [x] cargo make ci が通ることを確認。テスト数が変わらない (パラメータ展開で同等) ことを検証。
