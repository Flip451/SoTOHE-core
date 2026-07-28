<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 25, yellow: 0, red: 0 }
---

# Architecture Pattern Placement Guard Realignment

## Goal

- [GO-01] 型の配置、Primary Adapter 境界、実行時 application policy、および CQRS の適用判断を DDD・Clean Architecture に整合する意味論中心の規則へ再調整し、正当な設計を旧来の構造条件が拒否しない状態にする。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D1, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D3, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D5, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D7]
- [GO-02] 規範、型配置マトリクス、catalogue lint、テスト、type-designer guidance、reviewer briefing、および ADR baseline の代表実装を同じ変更単位で整合させ、構造的不変条件は機械的に、意味分類は根拠付き review で検証できる状態にする。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D8, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11]

## Scope

### In Scope
- [IN-01] domain concept の配置を、ユビキタス言語、不変条件、複数 operation を越えた意味の安定性、および delivery・persistence 都合からの独立性を主基準として判定する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D1] [tasks: T002, T003]
- [IN-02] same-track domain-internal inbound reference を domain model での利用を示す補助シグナルとして維持する一方、その不在だけで domain 配置を拒否せず、application boundary にのみ意味を持つ値は usecase boundary model として扱う。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D2] [tasks: T002, T003]
- [IN-03] Primary Adapter が usecase の Command、Query、Response、および boundary DTO を公開シグネチャで参照して transport 入出力との変換を担えるようにし、domain Entity・AggregateRoot の直接露出、公開シグネチャへの infrastructure 型露出、application boundary 内への transport 固有型漏出は引き続き禁止する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D3, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D4] [tasks: T002, T003, T006, T007]
- [IN-04] usecase の実行判断、永続化内容、整合性、または domain event に影響する時刻・乱数・ID 採番などの実行時値を、その取得能力を表す usecase Secondary Port と Interactor の必要時呼出しを通じて扱う。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D5] [tasks: T004, T005, T006, T007]
- [IN-05] ADR baseline の snapshot 時刻取得を、usecase の ClockPort を Interactor が呼び出す形へ是正し、Primary Adapter から timestamp provider を除去し、infrastructure clock adapter と composition-only wiring で実行経路を成立させる。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D6] [tasks: T004, T005, T006, T007]
- [IN-06] Command と Query を別 Interactor または Application Service に分離するのは、副作用、依存、エラー、整合性境界、read/write model の少なくとも一つに実質的な非対称性がある場合に限定する。実質的な非対称性とは、当該操作についてその次元の observable action/policy、必要な collaborator、起こり得る error、整合性境界、または read/write model が異なり、分離を選ぶ根拠となる差をいう。分離ごとに、該当する次元、操作固有の差、および分離根拠を review 可能な記録として残す。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D7] [tasks: T002, T004]
- [IN-07] type-designer convention、role × layer matrix、catalogue-lint configuration と distributed preset、関連テスト、type-designer guidance、および reviewer briefing を同期して更新し、lint は決定的な構造的不変条件を、semantic review は記録済み根拠による domain 分類を検証する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D8, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9] [tasks: T001, T002, T003, T007]

### Out of Scope
- [OUT-01] support workflow の変更は本 track の対象外とする。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11] [tasks: T002, T007]
- [OUT-02] crate topology の変更は本 track の対象外とする。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11] [tasks: T004, T005, T006, T007]
- [OUT-03] 既存の未変更型または未変更シグネチャを一括移行することは本 track の完了条件に含めず、後続の専用監査または関連変更 track で段階的に分類・移行する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D10] [tasks: T002, T003]

## Constraints
- [CN-01] 今回追加または変更する型とシグネチャは新規則へ即時適合させる一方、未変更の既存型を自動的に適合済みまたは恒久的な例外として扱わない。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D10] [tasks: T001, T003, T004, T005, T006, T007]
- [CN-02] representative enforcement と ADR baseline の是正は、crate topology および外部観測可能な CLI 契約を変更しない。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11, knowledge/adr/2026-07-25-0313-architecture-pattern-placement-cli-contract-preservation.md#D1] [tasks: T006, T007]
- [CN-03] 機械 lint は依存方向、禁止型露出、role と layer の明白な不整合など決定的に判定できる構造的不変条件だけを強制し、語彙名の一致だけで domain concept を機械分類しない。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9] [tasks: T002, T003]
- [CN-04] semantic domain classification は type-designer が根拠を記録し、semantic review が ADR、spec、および近接 domain model と照合して検証可能でなければならない。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] domain candidate は same-track inbound reference の有無だけでは拒否されず、意味論上 domain concept と判断される場合は適切な domain role へ配置でき、分類根拠を review 可能な形で記録する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D1, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D2, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9] [tasks: T002, T003]
- [ ] [AC-02] Primary Adapter の適合例は usecase Command、Query、Response、boundary DTO を参照して transport 変換と application service 呼出しを行え、Entity・AggregateRoot・infrastructure 型の公開露出と application boundary 内への transport leakage は lint または review で拒否される。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D3, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D4, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9] [tasks: T002, T003, T006, T007]
- [ ] [AC-03] ADR baseline snapshot の時刻は Interactor が ClockPort から取得し、Primary Adapter は timestamp provider を保持せず、system clock adapter は infrastructure にあり、composition root だけがそれらを配線する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D5, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D6] [tasks: T004, T005, T006, T007]
- [ ] [AC-04] CQRS の Command と Query は、操作の副作用、依存、エラー、整合性境界、または read/write model の少なくとも一つについて observable action/policy、必要な collaborator、起こり得る error、整合性境界、または read/write model に操作固有の差があり、その差が分離根拠になる場合だけで分離される。各分離は該当次元、具体的な操作差、および分離根拠を review 可能な記録で示し、review はその記録を ADR D7 と照合する。単に read と write が存在することまたは role が利用可能であることだけでは分離されない。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D7] [tasks: T002, T004]
- [ ] [AC-05] 更新済みの convention、role matrix、lint config・preset、テスト、type-designer guidance、および reviewer briefing は同じ配置・境界判断を表現し、文書だけまたは lint だけが新規則へ進む不整合を残さない。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D8] [tasks: T001, T002, T003, T007]
- [ ] [AC-06] catalogue lint とそのテストは、新しい許可境界を通過させつつ、依存方向、禁止型露出、role-layer 不整合などの構造違反を検出し、semantic domain classification の最終判断を review に残す。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D8, knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D9] [tasks: T003, T007]
- [ ] [AC-07] representative enforcement changes と ADR baseline の型配置・boundary・Clock correction は新規則に即時適合する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D10] [tasks: T001, T003, T004, T005, T006, T007]
- [ ] [AC-08] representative enforcement changes と ADR baseline の型配置・boundary・Clock correction は、crate topology を変更せずに完了する。 [adr: knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md#D11] [tasks: T004, T005, T006, T007]
- [ ] [AC-09] representative enforcement changes と ADR baseline の型配置・boundary・Clock correction の間、外部観測可能な CLI 契約を維持する。 [adr: knowledge/adr/2026-07-25-0313-architecture-pattern-placement-cli-contract-preservation.md#D1] [tasks: T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/type-designer-kind-selection.md#R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)
- knowledge/conventions/catalogue-schema-reference.md#Catalogue Lint Rule Kinds (reference)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/track-lifecycle.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 25  🟡 0  🔴 0

