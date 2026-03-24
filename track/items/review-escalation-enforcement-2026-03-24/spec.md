<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 3, yellow: 14, red: 0 }
---

# Review escalation enforcement — planning-only bypass guard + record-round wiring

## Goal

review state が NotStarted のまま code ファイルを含むコミットが通るすり抜けを構造的に防止する。
check-approved の planning-only fast-path に staged diff スコープガードを追加し、
/track:review を record-round に配線して review 結果を domain に永続化する。

## Scope

### In Scope
- check-approved CLI に staged files スコープ判定を追加 (planning-only allowlist vs code files) [source: discussion — 2026-03-24 review escalation 発見]
- check_approved usecase に planning_only フラグを追加し fast-path を条件付きに制限 [source: discussion — 2026-03-24 review escalation 発見]
- /track:review skill を sotp review record-round に配線 (各グループの fast/final 結果を永続化) [source: discussion — 2026-03-24 review escalation 発見]
- sotp review status CLI コマンド: per-group の Fast/Final 状態を表示 [source: discussion — 2026-03-24 review escalation 発見]
- 統合テスト: code diff + NotStarted review → check-approved 拒否を検証 [source: discussion — 2026-03-24 review escalation 発見]

### Out of Scope
- record-round 自体の domain ロジック変更 (RoundType::Fast/Final は既存) [source: inference — domain layer already complete]
- ReviewGroupState の変更 (FastOnly/BothRounds は既存) [source: inference — domain layer already complete]
- concern_streaks escalation の変更 [source: inference — separate concern]

## Constraints
- domain 層は I/O を含まない (hexagonal purity) [source: convention — hexagonal-architecture.md]
- 既存の planning-only コミット (track/items/, registry.md 等のみ) は引き続き通過させる [source: discussion — backward compatibility]
- check-approved の既存呼び出し元 (track-commit-message) を壊さない [source: discussion — backward compatibility]
- staged files 取得は CLI 層の責務 (usecase は planning_only フラグを受け取るだけ) [source: convention — hexagonal-architecture.md]

## Domain States

| State | Description |
|-------|-------------|
| ReviewStatus | NotStarted, Invalidated, FastPassed, Approved — 既存型、変更なし |
| ReviewGroupState | NoRounds, FastOnly, FinalOnly, BothRounds — 既存型、変更なし |

## Acceptance Criteria
- [ ] code ファイルを含む staged diff + NotStarted review → check-approved が拒否する [source: discussion — 2026-03-24 review escalation 発見]
- [ ] planning-only ファイルのみの staged diff + NotStarted review → check-approved が通過する [source: discussion — backward compatibility]
- [ ] /track:review skill が各グループの reviewer 結果を record-round で永続化する [source: discussion — 2026-03-24 review escalation 発見]
- [ ] sotp review status が per-group の Fast/Final 状態を表示する [source: discussion — 2026-03-24 review escalation 発見]
- [ ] cargo make ci が全テスト通過する [source: convention — hexagonal-architecture.md]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md

## Signal Summary

### Stage 1: Spec Signals
🔵 3  🟡 14  🔴 0

