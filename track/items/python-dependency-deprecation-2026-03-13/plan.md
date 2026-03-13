<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Python 依存脱却計画

Python 依存を security-critical path から段階的に除去し、track workflow と CI の必須経路を Rust バイナリ中心へ寄せる。
/track:plan を含む workflow core を Rust subcommand へ集約し、.venv 未構築でも壊れない運用を目標にする。
verify / PR / auxiliary utility は必須経路と補助経路に分け、移行順と Definition of Done を固定する。

## Phase 0-1: Inventory and security-critical hooks

- [x] 確定版 migration map を基に、Python エントリポイントを security-critical / workflow core / verification / auxiliary に分類し、各エントリポイントの移行先を固定する 2d5da09ff310a854d6b0462e949f0a53146234b4
- [x] Phase 1 として security-critical hooks（block-direct-git-ops, file-lock-acquire, file-lock-release）を Python launcher から Rust direct invocation へ切り替える差分を実装し、settings.json・guardrail検証・selftest を更新する 2467e9f2aa1c6b1f087ab3f28a3de9855fa8fb22

## Phase 2-4: Workflow core and wrapper migration

- [x] /track:plan 相当の workflow から Python 依存を除去するため、track_state_machine.py / track_schema.py / track_markdown.py が担う metadata validation・rendered view generation・sync-views を Rust subcommand へ集約する設計と差分を作る 57decaf7e62f182dbf1e7d1fb0129a15cb479c5f
- [x] git workflow wrapper（git_ops.py, branch_switch.py, pr_merge.py）を Rust CLI に統合する移行計画を固め、commit/note/branch/pr 系の主要 wrapper を Python 非依存にする

## Phase 5-6: Verification boundary and rollout

- [x] CLI 層へ流出した git workflow policy を usecase / infrastructure へ戻す前段スパイクとして、stage path policy・commit cleanup orchestration・branch guard の責務を分離し、`apps/cli/src/commands/git.rs` を薄い adapter に寄せる設計と差分を固める
- [x] CLI 層へ流出した PR workflow policy を usecase / infrastructure へ戻す前段スパイクとして、PR check evaluation・pending/failed 判定・wait-and-merge polling policy を `apps/cli/src/commands/pr.rs` から切り出す設計と差分を固める
- [x] pr_review.py と verify_* / check_layers / verify_orchestra_guardrails の役割を再分類し、Rust 化するもの・データ源泉見直し後に整理するもの・optional utility に残すものを明文化すると同時に、git/gh/repo-root/metadata 読み取りの adapter 境界を固定する
- [ ] .venv 非依存で hook fail-closed、track workflow、CI の必須経路が成立することを Definition of Done とし、M1-M4 の検証手順と rollout 順を固定する
