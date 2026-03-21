# verification: cli-review-module-split

## Scope Verified

- spec.md の acceptance criteria 1-6 をカバー

## Manual Verification Steps

1. `cargo make ci` が全て pass すること
2. 全対象ファイルが 700行以下であること:
   - `apps/cli/src/commands/review/mod.rs` — <200行
   - `apps/cli/src/commands/review/codex_local.rs` — <700行
   - `apps/cli/src/commands/review/adapters.rs` — <700行
3. 既存コマンドが同じ動作をすること（bin/sotp review codex-local / record-round / check-approved / resolve-escalation）

## Result / Open Issues

- (未実施)

## Verified At

- (未実施)
