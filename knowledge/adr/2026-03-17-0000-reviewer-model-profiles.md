---
adr_id: 2026-03-17-0000-reviewer-model-profiles
decisions:
  - id: 2026-03-17-0000-reviewer-model-profiles_grandfathered
    status: accepted
    grandfathered: true
---
# Reviewer model_profiles in agent-profiles.json

## Status

Accepted

## Context

Codex CLI レビューアの `--full-auto` フラグをモデルごとにどう制御するか。

## Decision

`agent-profiles.json` の `providers.codex.model_profiles` で per-model behavioral config（`full_auto` 等）を集中管理。CLI がファイルを読み、フラグを自動解決。Fail-closed: unknown model or missing file defaults to `full_auto: true`。

## Rejected Alternatives

- Hardcoded model-name heuristic in Rust code: モデル追加のたびにコード変更が必要
- Explicit CLI `--full-auto` flag: 呼び出し側に知識が必要。設定の一元管理ができない

## Consequences

- Good: モデル追加が JSON 編集のみで完結
- Good: fail-closed デフォルト（unknown model → full_auto: true）
- Bad: agent-profiles.json の管理コスト

## Reassess When

- Codex CLI の API が `full_auto` 相当を自動判定するようになった場合
