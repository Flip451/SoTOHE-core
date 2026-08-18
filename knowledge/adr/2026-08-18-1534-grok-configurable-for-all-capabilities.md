---
adr_id: "2026-08-18-1534-grok-configurable-for-all-capabilities"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:cursor-adr-add-grok-configurable:2026-08-19 D1 typed-pipeline 専用経路に grok 起動契約を足す。対象は ref-verifier-chain1 / chain2 / obligation-fulfillment-verifier / waiver-verifier の4つ。pr-reviewer hosted は対象外 + chat_segment:grok-tui:2026-08-19 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[add-grok-arms, unify-runner] chose:add-grok-arms; from:[delegate-profile, current-gap-only, strict-all] chose:current-gap-only; from:[criterion, enumerate-four] chose:enumerate-four; from:[out-of-scope, require-non-hosted] chose:out-of-scope"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:cursor-adr-add-grok-configurable:2026-08-19 D2 対象4つの fast_provider も grok を割り当て可能にする + chat_segment:grok-tui:2026-08-19 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[final-only, fast-too] chose:fast-too"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:cursor-adr-add-grok-configurable:2026-08-19 D3 起動契約は既存 grok binding の model / effort / resume / sandbox 写像を使う。例外表は作らない + chat_segment:grok-tui:2026-08-19 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[reuse-d8, new-contract] chose:reuse-d8"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:cursor-adr-add-grok-configurable:2026-08-19 D4 共有 semantic-verifier runner に grok arm を1本足す。capability ごとの独立 runner は作らない + chat_segment:grok-tui:2026-08-19 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[shared-runner, per-capability] chose:shared-runner"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:cursor-adr-add-grok-configurable:2026-08-19 D5 割り当て可能にするだけ。committed の agent-profiles.json と sample の値は grok に書き換えない + chat_segment:grok-tui:2026-08-19 reviewer の shipped default だけ grok に書き換える。D1 の4経路と sample は grok にしない + chat_segment:grok-tui:2026-08-19 Phase0 境界承認 再収束全文"
    candidate_selection: "from:[assignable-only, set-current, set-sample] chose:assignable-only; from:[keep-reviewer-default, set-reviewer-grok] chose:set-reviewer-grok"
    status: proposed
---
# 欠ける typed-pipeline 専用経路に grok を割り当て可能にする

## Context

[`2026-08-14-1225-grok-provider-binding.md`](2026-08-14-1225-grok-provider-binding.md) は、grok を両 `execution_mode` の provider にし、呼び出せる宇宙を `agent-profiles.json` に委譲した。`orchestrator-output` は `capability exec` の grok arm 1 本で足りる。`typed-pipeline` は専用経路ごとに grok の起動契約を載せる。typed-pipeline を `capability exec` に合流させない。

その専用経路のうち、`reviewer` / `review-fix-lead` / `dry-checker` / `dry-fix-lead` には grok の起動契約がある。一方、次の 4 capability は共有の provider-dispatching process runner（`make_agent_process_runner`。obligation / waiver の semantic-verifier もこれを包む）を使い、分岐は `claude` / `codex` / `gemini` だけである。

- `ref-verifier-chain1`
- `ref-verifier-chain2`
- `obligation-fulfillment-verifier`
- `waiver-verifier`

profile は grok を provider として受理する。しかしこの runner は `provider: grok` および `fast_provider: grok` を `unsupported ref-verifier provider 'grok'` で fail-closed する。profile に書けても実行できない。

「すべての capability に grok を割り当て可能にする」は開集合である。今回埋めるのは、上記の現在欠ける 4 経路である。`pr-reviewer` の hosted（Codex Cloud）経路は CLI 起動契約の外であり、今回の対象ではない。

## Decision

### D1: 欠ける typed-pipeline 専用経路 4 つに grok の起動契約を足す

`agent-profiles.json` の `provider` が grok のとき、次の 4 つの typed-pipeline 専用経路が grok を起動できるようにする。

- `ref-verifier-chain1`
- `ref-verifier-chain2`
- `obligation-fulfillment-verifier`
- `waiver-verifier`

専用経路を `capability exec` に合流させない。`pr-reviewer` の hosted 経路は対象外とし、grok 割り当て不能のまま残す。

開集合の閉じ方は、profile 宇宙全体でも「grok arm が無い runner」という基準でもなく、上記 4 名前への列挙である（開集合検査の保守的な閉じ方）。親 ADR が capability 名を列挙しないのは、呼び出せる宇宙を profile に委譲する話であり、本 ADR は現在の実装ギャップを 4 名前で閉じる。宇宙の委譲は変更しない。

### D2: 対象 4 つの `fast_provider` も grok を割り当て可能にする

D1 の 4 capability について、`fast_provider` に grok を書いたときも fail-closed しない。final だけを対象にして fast を残すと、fast を grok にした時点で実行時に落ち、割り当て可能にならない。

### D3: 起動契約は既存の grok 写像を使い、例外表は作らない

model / effort / resume / sandbox の写像は既存の grok provider binding（D1・D3・D8）を使う。これら 4 経路向けの起動契約や例外表は新設しない。返却は envelope の構造化出力フィールドから取り、テキスト欄は使わない。

### D4: 共有 process runner に grok arm を 1 本足す

D1 の 4 capability は、同じ provider-dispatching process runner を使う。その runner に grok の起動契約を 1 本足す。capability ごとに独立した grok 起動経路は持たない。

### D5: 対象 4 経路と sample は grok にせず、reviewer の shipped default だけ grok にする

D1 の 4 capability について、committed の `agent-profiles.json` と sample profile の `provider` / `fast_provider` 値は grok に書き換えない。その 4 経路の採否は設定者の編集に残す。

例外として、committed の `reviewer` だけは `provider: grok` / `model: grok-4.6` に書き換える。`fast_provider` / `fast_model` は現行のまま（codex / `gpt-5.6-luna`）とする。これは親 ADR D7（shipped default は grok を指さない）の reviewer に限った refinement であり、他 capability の shipped default は変えない。

### Existing decision relationship

本 ADR は [`2026-08-14-1225-grok-provider-binding.md`](2026-08-14-1225-grok-provider-binding.md) D5（grok は typed-pipeline の provider、専用経路に起動契約を載せる）の欠ける 4 経路を埋める refinement である。同文書 D5 の「名前を列挙せず宇宙は profile」・D8（model / effort / resume 写像、例外表なし）は変更しない。同文書 D7（shipped default は grok を指さない）は reviewer に限り本 ADR D5 が refine し、それ以外の shipped default は変えない。[`2026-07-12-0510-capability-exec-unified-dispatch.md`](2026-07-12-0510-capability-exec-unified-dispatch.md) D9（typed-pipeline を `capability exec` に合流させない）は変更しない。[`2026-08-02-0151-multi-provider-capability-routing.md`](2026-08-02-0151-multi-provider-capability-routing.md) D3（既定は外部プロバイダーを指さない）は reviewer 以外について維持する。

## Rejected Alternatives

- **A: 専用経路を `capability exec` に合流させる**: 既存の grok provider binding が typed-pipeline を `capability exec` に合流させないと決めている。固定返却スキーマを機械が消費する経路と、orchestrator が自由形式を消費する経路を混ぜる。
- **B: profile 上の全 capability を今回の対象にする**: 「すべての capability」は開集合であり、今回欠ける typed-pipeline 経路を超える。`orchestrator-output` は既に `capability exec` の grok arm で足りる。
- **C: `pr-reviewer` の hosted 経路も grok 化する**: Codex Cloud 投稿であり、今回の CLI 起動契約の外。既存契約も hosted を検査対象外のままとしている。
- **D: final だけ grok 可能にして fast は対象外にする**: `fast_provider` に grok を書くと実行時に落ち、割り当て可能にならない。
- **E: これら専用経路向けの grok 起動契約を新設する**: 既存写像と drift する。
- **F: capability ごとに独立 runner を持つ**: 今の共有 runner を複製し、保守面が増える。
- **G: committed profile / sample の値を grok に書き換える**: D1 の 4 経路と sample については、今回は割り当て可能性であり既定値の変更ではない。reviewer の shipped default だけは別裁定で grok にする。

## Consequences

- 良: 対象 4 capability は profile で grok / `fast_provider=grok` を書いても fail-closed しない。
- 良: 起動契約は既存 grok 写像のまま。例外表が増えない。
- 良: 共有 runner に arm 1 本で 4 capability が揃う。
- 負: profile 編集だけでは足りず、専用経路の実装が必要。
- 中立: D1 の 4 経路と sample の値は変わらない。その採否は設定者の編集。
- 中立: committed の `reviewer` だけは grok を指す。他 capability の shipped default は変えない。
- 中立: 新しい typed-pipeline 専用経路が増えても、本 ADR の列挙は自動では広がらない。

## Reassess When

- grok arm の無い typed-pipeline 専用経路が新しく増えたとき。
- 4 capability が共有 runner をやめて経路が分かれたとき。
- `pr-reviewer` に hosted ではない CLI 経路ができたとき。
- reviewer 以外の shipped default も grok に指す必要が繰り返し現れたとき。

## Related

- [`2026-08-14-1225-grok-provider-binding.md`](2026-08-14-1225-grok-provider-binding.md) — 本 ADR は D5 の欠ける 4 経路を埋める refinement。D8 は変更しない。D7 は reviewer に限り refine する。
- [`2026-07-12-0510-capability-exec-unified-dispatch.md`](2026-07-12-0510-capability-exec-unified-dispatch.md) — typed-pipeline を `capability exec` に合流させない決定を維持する。
- [`2026-08-02-0151-multi-provider-capability-routing.md`](2026-08-02-0151-multi-provider-capability-routing.md) — 既定は外部プロバイダーを指さない決定を、reviewer 以外について維持する。
