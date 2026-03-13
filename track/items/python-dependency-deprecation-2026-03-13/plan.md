<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Python 依存脱却計画

Python 依存を security-critical path から段階的に除去し、track workflow と CI の必須経路を Rust バイナリ中心へ寄せる。
/track:plan を含む workflow core を Rust subcommand へ集約し、.venv 未構築でも壊れない運用を目標にする。
verify / PR / auxiliary utility は必須経路と補助経路に分け、移行順と Definition of Done を固定する。

## Phase 0-1: Inventory and security-critical hooks

- [ ] 確定版 migration map を基に、Python エントリポイントを security-critical / workflow core / verification / auxiliary に分類し、各エントリポイントの移行先を固定する
- [ ] Phase 1 として security-critical hooks（block-direct-git-ops, file-lock-acquire, file-lock-release）を Python launcher から Rust direct invocation へ切り替える差分を実装し、settings.json・guardrail検証・selftest を更新する

## Phase 2-4: Workflow core and wrapper migration

- [ ] /track:plan 相当の workflow から Python 依存を除去するため、track_state_machine.py / track_schema.py / track_markdown.py が担う metadata validation・rendered view generation・sync-views を Rust subcommand へ集約する設計と差分を作る
- [ ] git workflow wrapper（git_ops.py, branch_switch.py, pr_merge.py）を Rust CLI に統合する移行計画を固め、commit/note/branch/pr 系の主要 wrapper を Python 非依存にする

## Phase 5-6: Verification boundary and rollout

- [ ] pr_review.py と verify_* / check_layers / verify_orchestra_guardrails の役割を再分類し、Rust 化するもの・データ源泉見直し後に整理するもの・optional utility に残すものを明文化する
- [ ] .venv 非依存で hook fail-closed、track workflow、CI の必須経路が成立することを Definition of Done とし、M1-M4 の検証手順と rollout 順を固定する
