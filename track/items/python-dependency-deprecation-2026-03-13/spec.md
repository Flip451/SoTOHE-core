# Spec: Python 依存脱却計画

## Goal

track workflow と CI の必須経路から Python 依存を外し、`.venv` 未構築でも
security-critical hook、状態遷移、主要 wrapper、必須検証が動作する構成にする。

## Scope

- security-critical hooks の Rust direct invocation 化
- `/track:plan` を含む workflow core の Rust 化計画
- git workflow wrapper と PR workflow wrapper の Rust CLI 統合計画
- verify script 群の Rust 化または責務再配置
- `.venv` / `python3` を optional utility へ後退させる rollout 設計

## Non-Goals

- 今トラックで全 Python script を一度に削除すること
- advisory hook や docs utility を即時にすべて Rust 化すること
- `takt` の後継システムを新設すること

## Constraints

- `takt` は廃止前提であり、後継 orchestrator を前提にしない
- 既存の fail-closed / warn+exit0 契約は security-critical hook で維持する
- 既存コマンドの外形は可能な限り保ちつつ、内部実装を Rust CLI へ寄せる
- `metadata.json` を SSoT とする前提は維持する
- 移行中も `cargo make ci` 相当の必須ゲートが壊れないようにする

## Acceptance Criteria

- [ ] Python エントリポイントの migration map が固定され、各 script/hook の移行方針が明文化されている
- [ ] security-critical hooks 3本について、`.claude/settings.json`・`sotp`・検証テストを含む差分計画がある
- [ ] `/track:plan` が依存する `track_state_machine.py` / `track_schema.py` / `track_markdown.py` を Rust へ集約する計画がある
- [ ] git workflow wrapper と PR workflow wrapper の Rust 移行対象と順序が明文化されている
- [ ] verify / CI gate の Python 依存について、Rust 化・再配置・optional utility の分類がある
- [ ] `.venv` 未構築でも M1 の必須経路が成立する Definition of Done が定義されている
- [ ] M1-M4 のマイルストーンと rollout 順が定義されている
