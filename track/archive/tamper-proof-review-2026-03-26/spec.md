<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 22, yellow: 0, red: 0 }
---

# レビュー verdict 改ざん防止

## Goal

Claude Code（オーケストレーター）がレビュー verdict を改ざんできない構造を実現する。
Rust CLI (sotp) を唯一の信頼されたコンポーネントとし、verdict 記録パスを sotp 内部に限定する。
review.json と review-artifacts/ を Write/Edit ガードで保護し、SHA-256 ダイジェストで事後検証を可能にする。

## Scope

### In Scope
- VerdictProvenance ADT を domain 層に追加し ReviewRoundResult に provenance フィールドを持たせる [source: ADR-2026-03-26-0010 §決定, ADR-2026-03-24-1200] [tasks: T001, T002, T003]
- review.json schema_version 2 — ラウンドに provenance ドキュメントを追加、既存データは LegacyUnverified [source: ADR-2026-03-26-0010 §決定] [tasks: T004]
- review-artifacts/<invocation-id>/ にセッションログ、final-message、attestation を永続化 [source: ADR-2026-03-26-0010 §決定] [tasks: T005, T007]
- AttestedReviewRound 型 + RecordRoundProtocol::execute_attested() ポートを追加し、codex-local --auto-record 内で provenance 生成（session log hash + payload hash + attestation）を統合 [source: ADR-2026-03-26-0010 §決定] [tasks: T006, T007, T008]
- RecordRound CLI サブコマンドを削除し、verdict 記録を codex-local --auto-record 内部に限定 [source: ADR-2026-03-26-0010 §決定, ADR-2026-03-26-0010 §追加決定: record-round 削除の検証] [tasks: T009]
- BlockProtectedReviewStateWrite PreToolUse hook で review.json + review-artifacts/ への Write/Edit をブロック [source: ADR-2026-03-26-0010 §決定] [tasks: T012, T013]
- check-approved に --require-provenance フラグを追加し、provenance 付きラウンドの SHA-256 検証を実行 [source: ADR-2026-03-26-0010 §決定] [tasks: T010, T011, T014, T015]

### Out of Scope
- ReviewReader/ReviewWriter ポート分離（review-port-separation トラックのスコープ） [source: ADR-2026-03-25-2125]
- 既存 review.json のレガシーデータマイグレーション（LegacyUnverified として扱う） [source: ADR-2026-03-26-0010 §決定]
- 暗号署名（HMAC, GPG 等）による reviewer output 検証 [source: ADR-2026-03-26-0010 §決定]

## Constraints
- 新規ロジックは Rust で実装する（Python 不可） [source: convention — .claude/rules/04-coding-principles.md]
- TDD ワークフローに従う（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md]
- ヘキサゴナルアーキテクチャ遵守 — domain 層は純粋、I/O は infrastructure 層のみ [source: convention — project-docs/conventions/hexagonal-architecture.md]
- domain 層で sha2 クレートを直接使用しない（SHA-256 計算は infrastructure 層） [source: convention — project-docs/conventions/hexagonal-architecture.md]
- review.json の後方互換 — provenance なしのラウンドは LegacyUnverified としてデコード [source: ADR-2026-03-26-0010 §決定]

## Domain States

| State | Description |
|-------|-------------|
| VerdictProvenance | LegacyUnverified | TrustedSubprocess — 各レビューラウンドの verdict 出自 |
| ReviewEvidenceStatus | Verified | LegacyUnverified | MissingArtifact | DigestMismatch | VerdictMismatch — check-approved での provenance 検証結果 |

## Acceptance Criteria
- [ ] ReviewError に provenance 関連バリアント (InvalidProvenance, MissingEvidence, EvidenceDigestMismatch) が追加され、provenance 検証失敗時に適切なエラーが返る [source: ADR-2026-03-26-0010 §決定] [tasks: T003]
- [ ] codex-local --auto-record で記録された verdict に TrustedSubprocess provenance が付与され、review.json に永続化される [source: ADR-2026-03-26-0010 §決定] [tasks: T006, T007, T008]
- [ ] sotp review record-round CLI サブコマンドが削除され、直接 verdict 注入が不可能になる [source: ADR-2026-03-26-0010 §決定] [tasks: T009]
- [ ] review-artifacts/<invocation-id>/ にセッションログ、final-message.json、attestation.json が永続化される [source: ADR-2026-03-26-0010 §決定] [tasks: T005, T007]
- [ ] check-approved --require-provenance が artifact の存在確認、digest 整合性、verdict 一致を検証し、いずれかの不一致でブロックする [source: ADR-2026-03-26-0010 §決定] [tasks: T010, T011, T014]
- [ ] cargo make ci がデフォルトで --require-provenance を有効にし、LegacyUnverified ラウンドを拒否する（hard enforcement） [source: ADR-2026-03-26-0010 §決定] [tasks: T015]
- [ ] Write/Edit で review.json および review-artifacts/ への書き込みが PreToolUse hook でブロックされる [source: ADR-2026-03-26-0010 §決定] [tasks: T012, T013]
- [ ] 既存 review.json（provenance なし）が LegacyUnverified として正しくデコードされる [source: ADR-2026-03-26-0010 §決定] [tasks: T004]
- [ ] cargo make ci が通過する [source: convention — .claude/rules/07-dev-environment.md] [tasks: T014]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/security.md
- project-docs/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 22  🟡 0  🔴 0

