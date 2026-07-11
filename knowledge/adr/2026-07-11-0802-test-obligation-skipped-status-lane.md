---
adr_id: 2026-07-11-0802-test-obligation-skipped-status-lane
decisions:
  - id: D1
    user_decision_ref: "chat:session-014Ubif2uPVJit1733uriobU:2026-07-11 ユーザー承認「ADR承認します」— skipped status lane 決定（D1）の明示承認"
    status: proposed
---

# テスト義務ゲートにおける skipped task status レーン

## Context

`2026-07-02-0359-test-obligation-and-fulfillment-gate.md` D15 は、テスト義務の task status 別解釈を `todo` / `in_progress` / `done` の三レーンについて定めている。一方、task completion の終端判定は `skipped` も resolved として扱うため、skipped task は merge の task-completion 前提を満たし得る。したがって、skipped 帰属の未解消義務に猶予を与えると、D15 が `todo` 猶予について定めた「merge 時点で必ず消尽し、本番運用へ漏出しない」という不変条件を満たせない。

D15 の `todo` 猶予は「義務を実装する task が未着手である」ことに根拠を持つ。`skipped` は未着手の継続ではなく、当該 task を実装しないという終端判断であるため、この根拠は移転しない。他方、skipped への帰属が正常に解決できた状態は、未帰属、rules totality 違反、malformed artifact のような構造的破綻でもない。よって、skipped を独立した status レーンとして最終 gate 判定へ位置付ける必要がある。

## Decision

### D1: skipped 帰属の未解消義務には猶予を与えず、独立した blocking レーンとして扱う

task 帰属の strictest-wins 解決結果が `skipped` である義務は、`done` / `in_progress` と同じ blocking class に置く。binding 欠如、verdict 欠如、鮮度失効を含む未解消状態は、`check` が fail-closed で block する。`skipped` は task 自体の終端性を表すだけであり、テスト義務の履行、免除、または verdict 成立を自動的に意味しない。

複数 task への帰属を一意に解決する status 順序は `done > in_progress > skipped > todo` とする。これにより `skipped` は `todo` より厳しい側を必ず選び、todo 猶予へ落ちない。先頭三レーンはいずれも同じ blocking verdict を返すため、その相互順序は帰属と表示を決定論的にするためのものであり、gate の厳しさに差を設けるものではない。

`results` は `skipped` を `todo` / `in_progress` / `done` と並ぶ独立した informational レーンとして集計表示する。親 ADR D14 の契約どおり exit は常に 0 のままとし、pass / fail 判定は `check` だけが担う。

本決定は親 ADR の D6 edge-locality、D9 drift classification、D10 rules totality、D11 fail-closed、D14 results-informational、D16 fingerprint validity を変更せず、いずれの保証も弱めない。

## Rejected Alternatives

### A. skipped を todo と同じ猶予レーンに置く

skipped は task completion 上 resolved であるため、猶予が merge 前に消尽する保証がない。未解消義務を本番運用へ漏出させる経路となり、D15 の不変条件に反するため却下する。

### B. skipped 帰属そのものを構造的破綻として扱う

skipped は有効な task status であり、帰属解決も正常に完了している。未帰属や malformed artifact と同じ構造軸へ畳むと、正常な帰属と構造欠陥の診断を混同するため却下する。

### C. skipped を done 表示へ畳む

両者の gate verdict は同じ blocking だが、「実装完了」と「実装しない終端判断」は運用上異なる。`results` の診断情報を失わないよう、独立レーンを維持する。

## Consequences

### Positive

- skipped task が merge の task-completion 前提を満たしても、未解消のテスト義務は fail-closed のまま残る
- todo 猶予の根拠と構造軸の診断分類を変えずに、四つ目の status を一意に解釈できる
- `results` で skipped 由来の未解消を他の blocking レーンから識別できる

### Negative

- task を skipped にするだけでは、その task に帰属するテスト義務は解消されない。既存の履行・免除手段で解消するか、帰属または上流判断を正す必要がある

## Reassess When

- task completion の終端判定が `skipped` を resolved として扱わなくなった場合
- skipped 専用の猶予について、merge 前に必ず消尽する別の機械的保証が確立された場合

## Related

- `knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md` — 親決定。D15 が task status レーンと todo 猶予、D14 が `results` の informational 性質を定める
