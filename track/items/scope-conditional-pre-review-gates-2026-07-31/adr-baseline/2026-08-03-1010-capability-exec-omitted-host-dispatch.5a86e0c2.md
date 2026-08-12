---
adr_id: 2026-08-03-1010-capability-exec-omitted-host-dispatch
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-13:本文確認・D3 平文化の上で ADR 全体を承認"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-08-13:本文確認・D3 平文化の上で ADR 全体を承認"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-08-13:本文確認・D3 平文化の上で ADR 全体を承認"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-08-13:本文確認・D3 平文化の上で ADR 全体を承認"
    status: proposed
---
# capability exec の省略 host は profile 解決 subprocess に限定する

## Context

`capability exec` の `--host` は、呼び出し元が実際の runtime host を自己申告し、
呼び出し先 provider との一致時に in-host 委譲を選べるようにする入力として決定されていた。
一方、operator-owned command config が provider 固有値を持たずに同じ command を宣言するには、
`--host` を省略できる意味論が必要である。

省略時に profile から得た provider を呼び出し元の自己申告として扱うと、実際の caller と profile が
異なる場合に同一 host と誤判定し、その caller が実行できない in-host 委譲指示を返しうる。
省略値は runtime host の事実を表さないため、明示値と同じ分岐入力にはできない。

## Decision

### D1: supplied `--host` は caller の runtime-host 自己申告を維持する

`--host` が supplied の場合、その値は caller が自己申告した実際の runtime host として扱う。
`2026-07-12-0510-capability-exec-unified-dispatch.md` D7 の provider-match 規則に従い、
capability 契約を保持できる provider の一致時だけ in-host 委譲の候補にできる。

### D2: omitted `--host` は in-host 分岐を放棄し、常に subprocess dispatch する

`--host` が omitted の場合、caller の runtime host は未申告である。この dispatch は provider の一致判定を行わず、
in-host 委譲を選ばず、常に subprocess dispatch する。実際に起動する provider は既存の capability routing を維持し、
`.harness/config/agent-profiles.json` にある requested capability の `capabilities.<name>.provider` から解決する。
`capabilities.orchestrator.provider` は subprocess target の選択には使わない。

subprocess dispatch は caller の実 host に依存せず成立するため、profile と caller が異なっても
in-host 委譲の誤分類を起こさない。この決定は
`2026-07-12-0510-capability-exec-unified-dispatch.md` D7 のうち、`--host` を mandatory とする部分を
supersede し、omitted case を追加する。D7 の supplied `--host` を caller の runtime-host 自己申告として扱う意味と、
capability 契約を保持できる provider の一致時だけ in-host 委譲を選べる規則は変更しない。

また、この決定は
`2026-07-12-0510-capability-exec-unified-dispatch.md` Rejected Alternative F のうち、
`--host` の省略自体を認めない結論だけを supersede する。profile 値を caller の自己申告とみなして
in-host 分岐に使わないという F の安全上の理由と、supplied 値に対する D7 の意味は維持する。

### D3: テンプレート利用者所有の config は host を持たず、invocation surface が optional host を転送する

`--host` は、「いま自分がどの provider host の中で動いているか」を caller が自己申告する値である。
これは実行時の事実であり、D1 の規則に従って扱う。

一方、phase writer argv などの command config は実行前に書かれる静的な宣言である。config に `--host` を
固定すると、別の host から同じ config を使ったときに、実際とは異なる自己申告になる。その結果、caller が
実行できない in-host 委譲指示が返りうる。

このため、テンプレート利用者所有の command config は provider host を固定せず、`--host` を保持しない。
保存する literal argv は provider-neutral のままにする。phase-engine entry command など、その config を使う
invocation surface が caller-supplied optional host parameter を受け取る。値がある場合だけ、configured writer の
`capability exec` 呼び出しへその値を転送する。転送された値は D1 の runtime-host 自己申告として扱い、D7 の規則に
基づく in-host 委譲の候補になりうる。値がない場合は `--host` を転送せず、D2 の profile 解決による
subprocess dispatch を適用する。

この決定は `2026-08-02-0806-operator-owned-phase-command-config.md` D1 のうち、engine が configured argv を
書き換えないとする部分だけを限定的に modify する。invocation surface は dispatch boundary で、caller-supplied
optional `--host` 一つだけを実行時 argv に追加できる。config に保存・解決された literal argv 自体は変更しない。
それ以外の argv を書き換えたり意味解釈したりしない規則と、config が provider-neutral な literal argv を
宣言する決定も維持する。

実行時の事実は config に固定しない。その事実を知る caller が実行時に自己申告し、機構は申告があった場合だけ
値をそのまま転送する。

### D4: supplied `--host` を profile 値で上書きしない

明示された `--host` と `capabilities.orchestrator.provider` が異なっても、明示値を profile 値へ正規化しない。
supplied 値は D1 の runtime-host 自己申告として D7 の分岐に使う。`--host` が omitted の場合も
`capabilities.orchestrator.provider` から caller host を推定せず、caller host は未申告のまま D2 を適用する。
requested capability の subprocess target 解決は caller host の決定とは独立している。

## Rejected Alternatives

### A. omitted `--host` を profile 値で補い、supplied の場合と同じ分岐を行う

profile は runtime caller の事実ではない。両者が異なるときに同一 host と誤判定し、caller が実行できない
in-host 委譲指示を返しうるため却下する。

### B. operator-owned command config に provider ごとの `--host` を固定する

同一 config の移植性を失わせ、runtime profile が持つ provider 選択と command declaration に同じ値を
二重管理させるため却下する。

## Consequences

### Positive

- provider-neutral な command config と、truthful な runtime-host 自己申告を両立できる。
- caller と profile の不一致を in-host 委譲へ誤分類しない。
- supplied `--host` の既存契約と D7 の provider-match 規則を維持できる。

### Negative

- `--host` を省略した dispatch は、実際には同一 host であっても in-host 委譲を利用できない。
- provider-neutral な command は常に subprocess 起動の分離コストを負う。

## Reassess When

- runtime caller の host identity を自己申告に頼らず、改ざんや profile との混同なく取得できる機構が導入されたとき。
- in-host 委譲と subprocess dispatch が同じ capability 契約を保持し、分岐差をなくせるとき。

## Related

- `knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md` D7 — `--host` を mandatory とする部分を
  D2 が supersede して omitted case を追加する modification target。supplied 値の runtime-host 自己申告と
  provider-match 規則は維持する。
- `knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md` D4 — requested capability の
  `capabilities.<name>.provider` から subprocess target を解決する既存の capability routing を維持する対象。
- `knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md` Rejected Alternative F —
  omitted `--host` を拒否する結論だけを D2 が限定的に supersede する対象。
- `knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md` D1 — configured argv を
  書き換えない部分に、dispatch boundary で caller-supplied optional `--host` だけを実行時 argv へ追加できる
  例外を D3 が設ける modification target。保存・解決された provider-neutral な literal argv は維持する。
- `knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md` D2 —
  provider-neutral な operator-owned writer argv を適用する対象。
- `.harness/config/agent-profiles.json` — requested capability の `capabilities.<name>.provider` を
  subprocess target として解決する SSoT。`capabilities.orchestrator.provider` は target 選択に使わない。
