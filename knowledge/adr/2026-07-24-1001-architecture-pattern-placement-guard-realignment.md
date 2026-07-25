---
adr_id: "2026-07-24-1001-architecture-pattern-placement-guard-realignment"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d1"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d2"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d3"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d4"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d5"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d6"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d7"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d8"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d9"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D10
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d10"
    candidate_selection: "from:[A,B,C] chose:A"
    status: proposed
  - id: D11
    user_decision_ref: "chat_segment:current-task:2026-07-24:architecture-pattern-placement-hearing-d11"
    candidate_selection: "from:[A,B,C] chose:D"
    status: proposed
---
# DDD・Clean Architectureに整合する型配置と境界依存の再調整

## Context

SoTOHE の TDDD 型設計規則は、レイヤー間の依存方向と役割配置を機械的に検証することで、実装者による境界逸脱を防いできた。一方、一部の規則は意味論より構造条件を優先し、一般的な DDD、ヘキサゴナルアーキテクチャ、Clean Architecture の判断から外れる設計を誘導し得る。

第一に、domain の `ValueObject` 候補を domain concept と認めるために、同一 track catalogue の別 domain entry からの inbound reference を必須としている。この条件では、ユビキタス言語や不変条件を表す型であっても、track の分割範囲やカタログ上の参照関係を理由に usecase 境界値へ押し出され得る。

第二に、`PrimaryAdapter` のメソッドシグネチャから domain／usecase 固有 role を一律に排除する `NoRoleInMethodSignature` 規則が、usecase の `Command`、`Query`、`Response`、boundary DTO まで参照不能にする。Primary Adapter が transport 入力を application boundary model へ変換するという通常の責務に対して制約が広すぎ、ほぼ同じ境界型を delivery 側へ重複させる mapping tax を生み得る。

第三に、ADR baseline の純 DI 化では timestamp provider が Primary Adapter に注入され、driver が時刻を取得して完成済み `Timestamp` を usecase command に渡す構造になった。依存は明示されテスト可能だが、snapshot 実行時刻を取得する判断が application policy であるなら、その所有者は usecase であるべきである。

また、Command／Query の role が存在すること自体を分離理由にすると、副作用、依存、エラー、整合性境界、read/write model に実質的な差がない操作まで別 Interactor／Serviceへ分ける儀式的 CQRS を誘発する。

この問題は個別実装だけでなく、type-designer が参照する convention、role matrix、catalogue linter、レビュー briefing が同じ構造基準を共有していることに起因する。規範と機械 enforcement を同時に再調整しなければ、後続の型設計でも同じ配置と重複が再生産される。

## Decision

### D1: domain concept は意味論を主基準として判定する

domain concept の判定では、ユビキタス言語に属すること、domain invariant を表すこと、複数の application operation を越えて意味が安定していること、persistence・CLI・workflow の都合を除いても存在する概念であることを主基準とする。

型が現在どの層から参照されているか、または一つの track catalogue 内でどの entry から参照されているかは、概念の意味を決定しない。

### D2: domain-internal inbound reference は補助シグナルとする

domain-internal inbound reference は、domain model 内で概念が実際に利用されていることを示す補助シグナルとして保持する。ただし、その欠如だけを理由に domain 配置を拒否してはならない。

inbound reference がない候補についても D1 の意味論基準で分類し、domain concept と判断した場合は適切な domain role でモデル化する。application boundary にのみ意味を持つ値は usecase の `Command`、`Query`、`Response`、または boundary DTO に配置する。

### D3: Primary Adapter は application boundary model を参照できる

Primary Adapter は usecase が公開する `Command`、`Query`、`Response`、boundary DTO をメソッドシグネチャで参照できるものとする。

Primary Adapter は transport 入力の解析・検証・変換、application service の呼び出し、application 出力から transport 表現への変換を担う。この変換に必要な application boundary model の参照は、依存方向違反ではない。

### D4: Primary Adapter の禁止境界を漏出リスクへ限定する

Primary Adapter では、domain の `Entity`／`AggregateRoot` を transport API として直接露出すること、infrastructure 型を公開シグネチャへ露出すること、transport 固有型を application boundary の内側へ漏出させることを引き続き禁止する。

`NoRoleInMethodSignature` は domain／usecase role の一律禁止ではなく、この禁止境界を表現する規則へ縮小する。

### D5: 実行時生成値は判断を所有する層へ配置する

時刻、乱数、ID 採番などの実行時生成値が usecase の実行判断、永続化内容、整合性、または domain event に影響する場合、その取得能力を usecase の Secondary Port として定義し、Interactor が必要な時点で呼び出す。

表示時刻、terminal decoration、transport protocol metadata のように delivery 表現だけへ影響する値は Primary Adapter が取得してよい。

### D6: ADR baseline の snapshot 時刻を usecase が取得する

ADR baseline usecase に `ClockPort` を導入し、snapshot を実行する Interactor が port から時刻を取得する。Primary Adapter は timestamp provider を保持せず、ユーザー入力から snapshot command を構築して application service を呼び出すことに専念する。

infrastructure は system clock を用いる adapter を実装し、composition root はその adapter を Interactor へ配線する。

### D7: CQRS 分離は実質的な非対称性がある場合に適用する

Command と Query を別 Interactor／Application Serviceへ分離するのは、副作用、依存、エラー、整合性境界、read/write model の少なくとも一つが実質的に異なる場合とする。

単に public な read と write が存在することや、カタログに `Command`／`Query` role が用意されていることだけを分離理由にしない。

### D8: 規範と機械 enforcement を同一変更単位で更新する

type-designer の convention、role × layer matrix、`NoRoleInMethodSignature` catalogue lint、関連テスト、type-designer と reviewer の briefing を同一 track で更新する。

文書だけを変更して旧 lint が正当な設計を拒否し続ける状態、または lint だけを緩和して判断基準が失われる状態を許容しない。

### D9: lint は構造的不変条件を、semantic review は意味分類を検証する

機械 lint は依存方向、禁止された型露出、role と layer の明白な不整合など、ソースとカタログから決定的に検証できる構造的不変条件を強制する。

ユビキタス言語への所属、domain invariant の有無、概念の時間的安定性などの意味分類は、type-designer が根拠を記述し、semantic review が ADR・spec・近接 domain model と照合して検証する。語彙名の一致だけで domain concept を機械判定しない。

### D10: 変更対象は即時適合し、既存未変更型は段階監査する

本決定後に追加または変更する型とシグネチャは、新しい規則へ即時適合させる。既存の未変更型は自動的に正当化も全面 grandfathering もせず、専用監査または関連変更の track で段階的に分類・移行する。

一括移行を完了条件にはしないが、既存型を恒久的な適用除外にも指定しない。

### D11: 最初の実装範囲を enforcement と ADR baseline の是正に限定する

最初の実装 track は、D8 が定める enforcement 機構の修正と、D1〜D7を具体的に検証する ADR baseline の型配置・境界型・Clock 配置の修正までを対象とする。

この範囲で規範、機械検証、代表実装を同時に整合させ、後続の設計が新しい判断基準を利用できる状態を完了条件とする。

## Rejected Alternatives

### A. domain-internal inbound reference の必須条件を維持する

track 分割と catalogue の可視範囲という偶然によって domain concept の分類結果が変わる。ユビキタス言語と不変条件より一時的な参照構造を優先するため、採用しない。

### B. Primary Adapter の role 制約を全面撤去する

usecase boundary model の参照を許可するために制約をすべて撤去すると、domain の `Entity`／`AggregateRoot` や infrastructure 型の漏出まで許容する。禁止境界を漏出リスクへ限定して保持する方が適切なため、採用しない。

### C. convention だけを変更し、lint とテストを変更しない

文書上の規範と機械挙動が不一致になり、旧規則が正当な型設計を拒否し続ける。実装者とreviewerの判断も分裂するため、採用しない。

### D. 既存の全 catalogue と実装を一括移行する

変更範囲と回帰原因が過大になり、今回の機構是正と直接関係しない型まで巻き込む。変更対象の即時適合と既存型の段階監査で収束可能なため、採用しない。

### E. 時刻・乱数・IDを常にPrimary Adapterで生成する

application policy を delivery 境界へ漏出させ、usecase単体での決定性とオーケストレーションの凝集性を弱める。値の用途を基準に所有層を決めるため、採用しない。

### F. すべてのread/writeを別Interactor／Serviceへ分離する

実質的な非対称性がない操作まで儀式的CQRSを強制し、型、trait、mapping、test doubleを増加させる。分離の利益がある場合に限定するため、採用しない。

## Consequences

### Positive

- domain model の配置がtrack内参照構造ではなく意味論と不変条件を反映する。
- Primary Adapter がapplication boundary modelを直接利用でき、同型のinput／output wrapperと変換処理を削減できる。
- application policyに属する時刻・乱数・ID採番をusecaseへ回収し、Interactor単体のテスト可能性と責務の凝集性が高まる。
- Command／Query分離の条件が明確になり、儀式的CQRSと不要なservice interfaceの増加を抑制できる。
- convention、lint、テスト、代表実装が同時に整合し、文書と機械挙動の二重規範を避けられる。

### Negative

- domain concept の意味分類をsemantic reviewで確認する負荷が増える。
- 既存型は段階的な監査対象として残り、全リポジトリが直ちに新規則へ揃うわけではない。
- catalogue linter、role matrix、type-designer workflow、review briefing、その周辺テストを同時に更新するコストが発生する。
- 境界型の共有を許可する分、Entity／AggregateRootやinfrastructure型の漏出を対象とした、より精密なlintとreviewが必要になる。

### Neutral

- crate topologyは変更しない。
- 外部観測可能なCLI契約は変更しない。
- 既存の未変更型は自動的に適合済みとも恒久的な例外とも扱わず、段階移行の対象として残る。

## Reassess When

- 同じ型についてtype-designerとsemantic reviewerのdomain／usecase分類が継続的に揺れるとき。
- application boundary modelの参照を許可した後も、同義のdriver DTOとusecase DTOの重複が減らないとき。
- 構造lintが正当なPrimary Adapter境界またはdomain配置を反復して誤検出するとき。

## Related

- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md`
- `knowledge/adr/2026-06-21-1420-cli-layers-tddd-and-role-placement-lint.md`
- `knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md`
- `knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md`
- `knowledge/conventions/type-designer-kind-selection.md`
