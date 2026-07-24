---
adr_id: 2026-07-25-0313-architecture-pattern-placement-cli-contract-preservation
decisions:
  - id: D1
    review_finding_ref: "ref-verify:architecture-pattern-placement-guard-realignment-2026-07-24:chain1:CN-02"
    status: proposed
---
# 型配置是正における CLI 契約の維持

## Context

`2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11` は、代表的な enforcement 機構と ADR baseline の型配置・境界型・Clock 配置を是正対象に定めた。一方、外部観測可能な CLI 契約を変更しないという制約は同 ADR の Neutral consequence にのみ記録され、D11 の実装範囲を拘束する Decision anchor にはなっていなかった。

## Decision

### D1: D11 の是正範囲では外部観測可能な CLI 契約を維持する

`2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11` を次の制約で refine する。D11 が選定する代表的な enforcement 機構の変更と ADR baseline の型配置・境界型・Clock 配置の是正は、コマンドおよびサブコマンド、引数とオプションの意味、終了状態、標準出力・標準エラー出力、機械可読出力の構造を含む外部観測可能な CLI 契約を変更してはならない。

## Rejected Alternatives

### A. CLI 契約の維持を consequence の記述に留める

Decision anchor を参照して是正範囲を拘束できず、下流の受入条件に規範的根拠を与えられないため、採用しない。

## Consequences

### Positive

- D11 の是正範囲に対する CLI 互換性を Decision anchor から検証できる。
- 内部の型配置や依存注入を変更しても、利用者と自動化から観測される挙動を維持できる。

### Negative

- 内部構造の是正時に、CLI の代表的な成功経路と失敗経路について互換性を確認する必要がある。

### Neutral

- 新しい CLI 機能や契約変更は導入しない。
- D11 が定める是正対象の範囲は拡張しない。

## Reassess When

- CLI 契約自体を意図的に変更する独立した判断が採択されたとき。
- D11 の是正対象が外部観測可能な挙動の変更なしには成立しないと判明したとき。

## Related

- `knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md`
