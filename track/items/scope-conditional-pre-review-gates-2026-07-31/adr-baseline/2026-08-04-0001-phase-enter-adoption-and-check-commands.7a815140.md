---
adr_id: 2026-08-04-0001-phase-enter-adoption-and-check-commands
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-13:本文確認・関係リスト再構成の上で ADR 全体を承認"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-08-13:本文確認・関係リスト再構成の上で ADR 全体を承認"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-08-13:本文確認・関係リスト再構成の上で ADR 全体を承認"
    status: proposed
---
# phase enter を canonical writer entry とし収束 check command を公開する

## Context

phase command config と phase engine が writer 前の ordered pre-entry command を実行できても、canonical な
`/track:*` workflow が writer capability を直接 dispatch すれば、その engine と収束条件を迂回できる。
また、直上流の収束を構成する final `zero_findings` review と chain-scoped semantic verification には、
phase engine が literal argv として実行できる zero/non-zero の公開 check contract が不足している。

operator-owned config に収束条件を宣言するだけでなく、canonical workflow の入口と各条件の exit-code contract を
揃えなければ、宣言された順序は任意の補助経路に留まり、writer 起動の前提条件を機械的に保証できない。

## Decision

### D1: canonical phase workflow は `phase enter` だけを writer entry に使う

config に宣言された `spec-design`、`type-design`、`impl-plan` の canonical `/track:*` workflow は、対応する writer を
起動するとき `bin/sotp phase enter <phase-id>` だけを入口に使う。複合 workflow も同じ入口を使う。
workflow SSoT や provider adapter から `bin/sotp capability exec` で writer を直接起動すると、宣言された
pre-entry command を通らずに進めるため、この直接起動は行わない。

phase engine は pre-entry command がすべて zero の場合に限り、configured writer を一度起動する。writer 自身は
引き続き `capability exec` を使用できる。provider routing、briefing、権限、in-host / subprocess の契約は変えない。
`capability exec` は phase engine 内部の dispatch と phase 外の capability invocation に使う汎用経路のままである。

関係:

- `2026-04-22-0829-plan-command-structural-refinements.md` D1 を一部 supersede: writer を Agent tool または直接処理で
  起動する外部構造だけを `phase enter` への委譲に置き換える。writer が対応 SSoT を単独で書き、内部 pipeline を
  完結し、結果を `/track:plan` state machine へ返す責務は維持する。
- `2026-07-12-0510-capability-exec-unified-dispatch.md` D8 を一部 supersede: canonical phase workflow から phase writer を
  `capability exec` で直接 dispatch する部分だけを置き換える。orphan planner 経路の撤去と、`capability exec` を汎用
  capability dispatch として維持する決定は変えない。
- `2026-08-02-0806-operator-owned-phase-command-config.md` D2 を refine: 宣言順の pre-entry、first non-zero stop、
  全条件通過後の writer 一回起動という engine contract を canonical workflow に適用する。

### D2: review と semantic verification に exit-code-checkable な公開 check を設ける

収束を終了コードで判定できるように、公開 `bin/sotp` check command を二つ設ける。

- review convergence check: 対象 scope の current final verdict が `zero_findings` の場合だけ exit zero とする。
  verdict の欠如、final 未完了、`findings_remain`、current artifact と対応しない verdict、入力不正は non-zero とする。
- chain-scoped semantic verification check: 選択した chain が current convergence rule を満たす場合だけ exit zero とする。
  未解消、pending、必要な判定根拠の欠如、入力不正は non-zero とする。他 chain の状態を選択した chain の結果として
  読み替えない。

表示用の `ref-verify results` は常に exit zero のままであり、gate には使わない。

関係:

- `2026-07-22-0400-sot-reentry-sequencing.md` D2 を concretize: review 要素と意味論検証要素を phase engine が判定できる
  command contract にする。参照信号・意味論検証・review という収束定義は変えない。
- `2026-07-22-0817-deferred-upstream-semantic-verification.md` D1 を maintain: 選択した chain だけを判定する。
- `2026-06-26-0842-ref-verify-results-command.md` D2 を maintain し、semantic verification check から分離:
  informational かつ常時 exit zero の `ref-verify results` を gate として再解釈しない。

### D3: shipped default config は direct-upstream convergence matrix を順序付きで宣言する

配布する `.harness/config/phase-commands.json` は、各 declared phase の pre-entry commands を次の順で宣言する。
適用されない要素は省く。

1. direct-upstream signal gate
2. 適用される chain-scoped semantic verification check
3. upstream scope の current final `zero_findings` review check

- `spec-design`: `adr_user` signal gate、ADR scope の current final `zero_findings` review check。
- `type-design`: `spec_adr` signal gate、Chain ① semantic verification check、spec scope の current final
  `zero_findings` review check。
- `impl-plan`: `catalog_spec` signal gate、Chain ② semantic verification check、types scope の current final
  `zero_findings` review check。

default config はテンプレート利用者が所有し、有効な公開 command とその順序を編集できる。新しい phase を追加する
ときも、その direct upstream に対する同じ順序を default declaration に適用する。

関係:

- `2026-07-22-0546-adr-convergence-ref-verify-scope-exemption.md` D1 を maintain: `spec-design` に semantic verification を
  置かない。
- `2026-07-22-0817-deferred-upstream-semantic-verification.md` D1 を maintain: Chain ① / ② の限定を守り、別 chain の状態を
  混入させない。
- `2026-07-22-0400-sot-reentry-sequencing.md` D6 を一部 supersede: 収束規律を prompt level のみに置き、machine
  enforcement を範囲外とする部分だけを、declared phase の entry に限って置き換える。同 ADR D1 の routing と
  sequencing の責務分離、D2 の収束定義、D3 の direct-upstream matrix、D4 の即時突き返し、D5 の impl-plan status
  transition 例外は維持する。既存の commit / merge gate と CI の責務も変えない。
- `2026-08-02-0806-operator-owned-phase-command-config.md` D1 の literal argv と no-rewrite 規則、および D2 の ordered
  execution contract を maintain する。

## Rejected Alternatives

### A. workflow の direct writer dispatch と `phase enter` を併存させる

direct dispatch が収束 check を迂回でき、canonical entry が二つになる。config の matrix を必須条件ではなく
任意の補助経路に戻すため却下する。

### B. informational な results command の出力を caller が読んで成否を決める

exit zero が収束を意味せず、出力解釈を workflow や adapter ごとに再実装することになる。
engine が fail-closed に扱える専用 check contract を設けるため却下する。

### C. convergence matrix を Rust にハードコードする

operator-owned literal argv declaration と engine の command-semantics 非解釈を破り、設定と実装に二つの
matrix を作るため却下する。

## Consequences

### Positive

- canonical phase entry が一つになり、writer 起動前の direct-upstream convergence を迂回できない。
- review と semantic verification を literal argv の zero/non-zero contract として順序付けられる。
- shipped default が direct-upstream matrix を明示しつつ、operator の config 所有権を維持できる。

### Negative

- canonical phase workflow は writer を直接起動できず、phase engine の config validation と process execution に依存する。
- review と semantic verification に新しい check command surface と fail-closed 診断が必要になる。
- default config を変更した operator は、direct-upstream convergence を満たす有効な command と順序を所有する。

## Reassess When

- phase engine 以外に同じ収束 matrix を迂回不能に強制できる canonical entry が導入されたとき。
- review または semantic verification の verdict model が変わり、zero/non-zero の binary contract で表せなくなったとき。
- SoT Chain または phase の direct-upstream 関係が変更されたとき。

## Related

- `knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md` D1 — direct writer invocation を
  D1 が `phase enter` へ置き換える modification target。
- `knowledge/adr/2026-07-12-0510-capability-exec-unified-dispatch.md` D8 — canonical phase workflow の
  direct `capability exec` だけを D1 が置き換え、planner 撤去と汎用 dispatch は維持する modification target。
- `knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md` D1 / D2 — literal argv ownership、
  no-rewrite、ordered pre-entry、first-failure stop、writer 一回起動を維持して canonical entry と default matrix に適用する対象。
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` D2 / D3 / D6 — D2 が収束要素を check command に具体化し、
  D3 が prompt-only enforcement を declared phase entry に限って supersede する対象。
- `knowledge/adr/2026-07-22-0546-adr-convergence-ref-verify-scope-exemption.md` D1 — `adr_user` convergence に
  semantic verification を要求しない規則を維持する対象。
- `knowledge/adr/2026-07-22-0817-deferred-upstream-semantic-verification.md` D1 — semantic verification を
  selected chain に限定する規則を維持する対象。
- `knowledge/adr/2026-06-26-0842-ref-verify-results-command.md` D2 — informational / always-zero results を
  新しい semantic check command と分離して維持する対象。
- `knowledge/adr/2026-06-30-0425-harness-workflow-ssot-adapters.md` D3 / D8 — workflow logic を SSoT に置き、
  provider adapter を薄く保つ境界を維持する対象。
