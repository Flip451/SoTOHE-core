---
adr_id: "2026-08-14-1049-base-merge-commit-record-advance"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:grok-session-01a064b2-df42-7062-bd70-f55aa2a078a4:2026-09-03 Phase 0 boundary approval of the converged D1 text (base merge cleanup includes commit-record advance)"
    status: proposed
---
# base merge は track の commit record を前進させる

## Context

base merge の後始末は baseline 置換と derived views 再生成を行うが、track の commit record は前進させない。commit record はレビュー範囲の diff base として解決に使われるため、merge 後も範囲は古い base から計算され、取り込んだ base 所有の変更が当該 track のレビュー対象として計上される。レビュアーは track が書いていない差分を読み、無関係な指摘が生じる。

なお完了印としての同期記録は既存決定で廃止済みだが、それは読取消費者を持たない記録の話である。commit record はレビュー範囲と信号評価という消費者を持つ別の状態であり、前進させる必要がある。

## Decision

### D1: base merge の後始末に commit record の前進を含める

clean merge と conflict 回復の完了時、いずれも merge 後の HEAD を track の commit record として記録する。記録は既存の commit record 更新経路を用い、既存の後始末段と同じ完了条件で扱う。記録に失敗した場合は fail-closed とし、後始末の部分完了を成功として報告しない。

### Existing decision relationship

本 ADR の D1 は `2026-08-02-0715-base-merge-cleanup-state.md` D3（後始末段階の固定）を **refines** する。同 D3 が定める Baseline → Views の順序と失敗報告の規律は変更せず、記録段を加える。

## Rejected Alternatives

- **diff base の解決側で merge commit を考慮する**: 解決の分岐が増え、消費者ごとに base 意味論がずれる。状態である commit record を正しく保つほうが単純。
- **operator が手動で記録する運用**: 忘れると誤ったレビュー範囲を生み、原因の特定も難しい。後始末の一部として機械化すべき。

## Consequences

- 良: merge 後のレビュー範囲が当該 track の差分だけになり、取り込んだ base 所有の変更に対する無関係な指摘が消える。
- 良: 同じ base を用いる信号評価も、merge で取り込んだ内容を当該 track の変更として扱わなくなる。
- 中立: commit record の更新はコミットを作らない（記録の書き換えのみ）。
- 負: 後始末の完了条件が 1 段増える。

## Reassess When

- diff base の解決規則が変わったとき、または commit record の消費者が変わったとき。
