<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 15, yellow: 10, red: 0 }
---

# RVW-34: RecordRoundProtocol findings fidelity fix

## Goal

RecordRoundProtocolImpl が findings_remain verdict を review.json に記録する際、reviewer の原データ（message, severity, file, line）を保持する。
現状は findings_to_concerns() で concern slug に変換後 StoredFinding::new(slug, None, None, None) で再構築しているため、全ての付加情報が消失している。
この修正により review.json の findings データが既存4フィールド（message, severity, file, line）について reviewer 出力と同等の忠実度を持つようになる。category フィールドの保持は RVW-38（別トラック）で対応する。

## Scope

### In Scope
- RecordRoundProtocol::execute trait に findings: Vec<StoredFinding> パラメータを追加 [source: knowledge/strategy/TODO.md §RVW-34, knowledge/strategy/rvw-remediation-plan.md §Phase-B T003] [tasks: T001]
- RecordRoundProtocolImpl::execute の lossy 変換コード（review_adapters.rs:458-461）を削除し、渡された findings を直接使用 [source: knowledge/strategy/TODO.md §RVW-34] [tasks: T001]
- usecase 層に ReviewFinding → StoredFinding 変換関数を追加（verdict.rs 付近に配置） [source: inference — Codex planner design decision: usecase is the right boundary between ReviewFinding(usecase) and StoredFinding(domain)] [tasks: T002]
- record_round()（string-based）で既にパース済みの ReviewPayload.findings から Vec<StoredFinding> を構築して protocol に渡す [source: inference — Codex planner design decision: no new RecordRoundInput field needed] [tasks: T002]
- record_round_typed() に findings パラメータを追加 [source: knowledge/strategy/TODO.md §RVW-34] [tasks: T002]
- CLI auto-record path（codex_local.rs）で findings_to_concerns() と変換関数の両方を呼び出し [source: inference — both concerns(escalation) and findings(persistence) must flow through] [tasks: T002]
- StubProtocol / RecordRoundProtocolCallArgs の更新 + round-trip fidelity テスト追加 [source: convention — .claude/rules/05-testing.md] [tasks: T003]

### Out of Scope
- RVW-38: StoredFinding / FindingDocument への category フィールド追加（別トラック） [source: knowledge/strategy/rvw-remediation-plan.md §Phase-B T001/T002]
- RVW-08: ScopeFilteredPayload の削除 [source: knowledge/strategy/rvw-remediation-plan.md §Phase-B T004]
- WF-45: render_review_payload() の category null 出力 [source: knowledge/strategy/rvw-remediation-plan.md §Phase-B T005]
- FindingDocument codec への新フィールド追加（既存4フィールドの保持のみ） [source: inference — no new fields being added to the codec]

## Constraints
- TDD ワークフロー必須（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md]
- 同期のみ（async なし） [source: track/tech-stack.md]
- ライブラリコードでパニック禁止 [source: convention — .claude/rules/04-coding-principles.md]
- レイヤー依存方向の厳守: domain → (なし), usecase → domain, infra → domain+usecase, cli → all [source: architecture-rules.json]
- findings_remain 時は findings と concerns 両方 non-empty を検証、zero_findings 時は両方 empty を検証（fail-closed） [source: inference — Codex planner design decision: both are required for their respective purposes]
- Vec<StoredFinding> をそのまま使用（NonEmpty wrapper は不適切: zero_findings では空リスト、domain の GroupRoundVerdict::findings_remain() が非空を保証） [source: inference — Codex planner design decision: domain already enforces non-empty]

## Domain States

| State | Description |
|-------|-------------|
| ZeroFindings | concerns=empty, findings=empty. No reviewer findings. |
| FindingsRemain | concerns=non-empty, findings=non-empty. Full finding data preserved from reviewer output. |

## Acceptance Criteria
- [ ] RecordRoundProtocol::execute が findings: Vec<StoredFinding> パラメータを持つ [source: knowledge/strategy/TODO.md §RVW-34] [tasks: T001]
- [ ] review_adapters.rs の lossy 変換コード（StoredFinding::new(slug, None, None, None)）が削除されている [source: knowledge/strategy/TODO.md §RVW-34] [tasks: T001]
- [ ] usecase 層に ReviewFinding → StoredFinding 変換関数が存在する [source: inference — Codex planner design decision] [tasks: T002]
- [ ] record_round() が verdict JSON の findings を StoredFinding に変換して protocol に渡す [source: inference — Codex planner design decision] [tasks: T002]
- [ ] findings_remain verdict で message, severity, file, line が review.json に完全に保持される round-trip テストが存在する [source: convention — .claude/rules/05-testing.md] [tasks: T003]
- [ ] findings_remain verdict で findings が空の場合にエラーを返すテストが存在する（fail-closed 不変条件） [source: inference — Codex planner design decision: both findings and concerns required for findings_remain] [tasks: T003]
- [ ] zero_findings verdict で findings が非空の場合にエラーを返すテストが存在する（fail-closed 不変条件） [source: inference — Codex planner design decision: both must be empty for zero_findings] [tasks: T003]
- [ ] cargo make ci が通る [source: convention — track/workflow.md §Quality Gates] [tasks: T001, T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 15  🟡 10  🔴 0

