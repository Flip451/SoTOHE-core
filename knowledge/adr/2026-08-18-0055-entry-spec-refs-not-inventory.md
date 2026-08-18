---
adr_id: "2026-08-18-0055-entry-spec-refs-not-inventory"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:grok-session:2026-08-18:user-strip-entry-spec-refs-inventory-keep-field"
    status: proposed
---
# entry-level spec_refs から総目録の役割を外す

## Context

[`2026-08-13-1720-test-obligation-method-anchor-ownership.md`](2026-08-13-1720-test-obligation-method-anchor-ownership.md) D1 は、entry-level `spec_refs` を全 anchor の総目録、method-level `spec_refs` をその担当分とした。包含（method ⊆ entry）と被覆（method の和 = entry）を構造検証で強制し、単一 method は entry の全 anchor を写す。

当時は、entry だけの anchor が fulfillment から消えるとして被覆を必須にした。その前提は、entry 参照が method 割り当て用の総目録でしかない、という仮定だった。entry `spec_refs` は check totality の cite edge を既に作る。method 義務へ割り当てなくても、entry の grounding は検証から消えない。

総目録としての役割だけを外す。フィールド自体は残す。

## Decision

### D1: entry-level spec_refs は総目録ではない

本 ADR は [`2026-08-13-1720-test-obligation-method-anchor-ownership.md`](2026-08-13-1720-test-obligation-method-anchor-ownership.md) D1（現行の実効 head）を refine / modify する。0340 / 0040 は変更対象ではない。

1720 D1 から次を撤回する:

- entry-level `spec_refs` を全 anchor の総目録とすること
- method-level `spec_refs` をその総目録からの割り当て部分集合とすること
- 包含（method ⊆ entry）
- 被覆（method の和が entry を覆うこと）
- 単一 method が entry の全 anchor を写すこと
- これらに反する catalogue の構造的棄却

残す:

- entry-level `spec_refs` フィールド
- entry-level `spec_refs` は entry 自身の仕様への grounding（義務へはコピーしない）
- method `spec_refs` は method 義務が所有する独立集合（1720 D2）
- 0340 D1 / D2 / D3（独立 `action`、Add / Modify の非空 `spec_refs`、指名 catalogue のみ）
- 0040 D1（親が `reference` / `delete` なら子 `spec_refs` は空）

## Rejected Alternatives

- **entry-level `spec_refs` フィールドごと削除する** — 問題は総目録役割だけである。entry 自身の grounding は残す。
- **1720 が却下した「coverage を検証しない」を前提ごと再採用する** — 当時は entry-only の anchor が fulfillment から消えると見なした。entry `spec_refs` は check totality の cite edge を既に作る。撤回するのは総目録役割とそれに付く包含・被覆・写し・構造棄却であり、entry 参照の可視性を捨てることではない。

## Consequences

- 良: method `spec_refs` を独立した所有集合として書ける。包含・被覆・単一 method の全量写しを強制しない。
- 負: entry と method の `spec_refs` は一致しなくてよい。entry だけの anchor は method 義務の fulfillment 対象にはならない（check totality の cite edge としては残る）。

## Reassess When

- entry-only の anchor が check totality からも消えることが分かったとき。
- 独立集合のままでは履行漏れや二重検証が繰り返し起きるとき。

## Related

- [`2026-08-13-1720-test-obligation-method-anchor-ownership.md`](2026-08-13-1720-test-obligation-method-anchor-ownership.md) D1 — 本 ADR はこれを refine / modify する。対象は現行の実効 head。D2 は変更対象ではない。
- [`2026-08-17-0340-method-declaration-action.md`](2026-08-17-0340-method-declaration-action.md) D1 / D2 / D3 および [`2026-08-18-0040-parent-forbids-method-spec-refs.md`](2026-08-18-0040-parent-forbids-method-spec-refs.md) D1 — 変更対象ではない。
