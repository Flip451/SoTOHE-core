---
adr_id: "2026-07-30-1022-batch-plan-declaration-domain-unsettled-tasks"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-07-31 merge 段階の採用裁定「すべてのdelta adrの決定を承認します。」(PR #228 の delta ADR 一括採用)"
    candidate_selection: "from:[unsettled-only-declaration-domain,forbid-settled-declaration,retroactive-full-declaration,exclude-settled-contributors-from-ceiling,relax-absence-fail-closed] chose:unsettled-only-declaration-domain"
    status: accepted
  - id: D2
    user_decision_ref: "chat:2026-07-31 merge 段階の採用裁定「すべてのdelta adrの決定を承認します。」(PR #228 の delta ADR 一括採用)"
    candidate_selection: "from:[unplanned-task-restricted-to-unsettled,advisory-unplanned-task,keep-all-task-coverage] chose:unplanned-task-restricted-to-unsettled"
    status: accepted
  - id: D3
    user_decision_ref: "chat:2026-07-31 merge 段階の採用裁定「すべてのdelta adrの決定を承認します。」(PR #228 の delta ADR 一括採用)"
    candidate_selection: "from:[judge-every-entry-into-in-progress,declaration-presence-as-reopen-precondition] chose:judge-every-entry-into-in-progress"
    status: accepted
---
# `batch-plan.json` の宣言対象を未 settle タスクに限定する

## Context

`batch-plan.json` の cross-file 構造検査は「`impl-plan.json` の全タスクがちょうど 1 つの batch に
属す」ことを要求し、実装は planned task の全件を走査して batch に属さないタスクを違反として報告する。
一方、todo → in_progress 遷移の admission は `batch-plan.json` の不在を error とする
（graceful skip なし）。

この二つは、ceiling 機構の適用がタスク列の途中で始まる場合に両立しない。すでに settle した
タスクを持つ track が新しいタスクの実装を開始しようとすると、admission は `batch-plan.json` の
存在を要求し、構造検査はそこに settle 済みタスクの宣言まで要求する。

settle 済みタスクを遡って宣言する経路は正直には通らない。過去の commit は ceiling 機構なしに
編成されており、共有 scope へ複数タスクが上限超で寄与している束を含む（このとき単独寄与者免除は
成立しない）。通すには過去の差分に分解不能理由を後付けするか ceiling を上げるしかなく、いずれも
既存成果物を歴史記録として再検証しないという遡及なしの原則に反する
（`2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D1）。

そもそも settle 済みタスクの見積り宣言には予測としての意味がない。その差分は実測され、review も
収束し、commit によって後続 review の対象から外れている。ceiling が守るのは「まだ存在しない、
または止めれば累積を防げる差分」である。

結果として現状の適用域では、機構の導入自体が「宣言できないが宣言を要求される」状態を作り、
残作業へ進む正規経路が存在しない。同じ状態は、機構を含む版へ実装途中で upgrade した consumer
track にも生じる。

一方、宣言を未 settle タスクに限ると「未 settle かつ未宣言」のタスクが初めて成立し得る。タスク
状態遷移は done → in_progress の reopen を正規経路として公開しており、この遷移は admission 判定を
経ない — 判定は todo から始まるタスクにのみ働くと定められている。

## Decision

### D1: 宣言対象は未 settle タスク — settle 済みタスクの宣言は任意

`batch-plan.json` が宣言を要求されるのは未 settle のタスク（todo / in_progress）に限る。
settle 済みのタスクは宣言を要求されない。settle の判定は遷移時の admission が用いる既存の定義を
そのまま使う（commit 記録を伴う done、または skipped）— commit 記録のない done は未 settle であり、
宣言対象に含まれる。新しい状態概念は導入しない。

宣言を禁止せず**任意**とする理由は既存 plan の連続性である。タスクは実装の進行に伴い、batch 宣言を
書き換えることなく settle する。現在 batch の解決（宣言順で最先行の、未 commit の member を残す
batch）は、settle 済み member が宣言に残っていることを前提に成立している。宣言を禁止すると commit の
たびに Phase 3 の終端成果物を書き換える必要が生じ、この解決と 1 file = 1 writer の規律に反する。

任意は免除ではない。宣言された settle 済みタスクは従来どおり見積り Σ と ceiling 照合の対象である。
遡って宣言する経路が緩和されるのではなく、**宣言しないという選択が正規化される**。

### D2: 未宣言違反は未 settle タスクにのみ適用し、実在検査は全宣言に適用する

- 「planned task がどの batch にも属さない」違反は、未 settle タスクについてのみ報告する。
- 「宣言された task id が実在のタスクを指す」検査は、settle 済みタスクへの言及を含むすべての宣言に
  従来どおり適用する。存在しない id の宣言は依然として違反である。
- 「タスクの依存先が同一または先行の batch にある」依存順序検査は、依存元と依存先の双方が宣言
  されている辺についてのみ適用する。宣言されていない端は D1 により settle 済みタスクに限られ、
  その batch を解決する手段がない。検査対象外の辺は違反ではなく、無検査で通す。

適用域を絞っても admission が依拠する性質は失われない。membership 検査が読むのは遷移候補
（必ず未 settle）の所属 batch であり、候補が batch に属することは絞った後の未宣言検査が保証し
続ける。settle 済みタスクの所属を admission が読むのは reopen の判定（D3）のみで、そこでは
未宣言 = 現在 batch の member でない として拒否側に働く。依存辺の順序検査についても、除外される
辺の端は settle 済み、すなわち既に delivered であって「後続 batch」になり得ないため、保証の喪失は
ない。

### D3: admission 判定の対象を in_progress へ入るすべての遷移とする

admission 判定（membership 検査 + 実測ガード）が働く遷移集合を、todo → in_progress に加えて
done → in_progress（reopen）へ広げる。判定式は変更しない — 広げるのは適用対象の遷移だけである。

- reopen の候補にも、現在 batch の member であることと見積りの申告を要求する。満たさなければ
  遷移を拒否する。
- D1 により宣言されていない settle 済みタスクは member ではないため、その reopen は拒否される。
  再開には impl-planner が当該タスクを現在 batch の member として宣言し直すことが必要である
  （宣言の更新が先、遷移が後）。
- 宣言されている settle 済みタスクの reopen は、その所属 batch が現在 batch である（未 commit の
  member を残す）場合に限り通る。すでに閉じた batch の member であれば現在 batch ではなく、同じく
  宣言の更新を要する。
- skipped からの再開は ResetToTodo → Start を経るため、保護は Start の admission が担い、遷移集合の
  追加は要らない。ResetToTodo が未宣言のまま todo タスクを作り得ることは D1 と矛盾しない — D1 の
  宣言要求は状態機械上の表現不可能性ではなく、二つの判定点で強制される義務である。未宣言の todo
  タスクは Start の admission が見積りを引けず error となって着手できず（見積りの照合は membership
  より前に行われるため、batch 帰属を問う前に fail-closed になる）、Phase 3 gate は未 settle かつ
  未宣言である間そのタスクを未宣言違反として報告する。再入した作業が未宣言の todo として現れるのは
  通常の計画状態であり、救済は新規の未 settle タスクと同じ impl-planner の宣言更新である。その間に
  実装が進行することも、いずれかの gate を無言で通過することもない。

必要性は D1 の帰結である。宣言が任意になったことで「未 settle かつ未宣言」の状態が初めて成立し、
reopen が無判定のままなら、そのタスクは batch 帰属も見積りもないまま in_progress となり、D1 が
要求する「未 settle タスクは宣言される」という不変条件がその瞬間に破れる。判定を遷移集合の一部に
だけ置くことは、admission が「どの workflow / capability から到達しても迂回できない」という
`2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D3 の性質にも反する — reopen は CLI が
公開する正規経路である。reopen 後の差分は現在 batch の review に積まれるため、実測ガードが守る対象も
todo からの着手と同一である。

### 既存決定との関係

本 ADR D2 は `2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D2 の cross-file 構造検査の
**適用域**と、`2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md` D2 の依存順序
検査の**適用域**を refine する。後者は「宣言されたすべての依存辺」という域を、依存元と依存先の
双方が宣言されている辺へ絞るものである。本 ADR D3 は 1521 D3 の**判定対象の遷移集合**を refine する
（reopen を加える additive な拡張）。

変更しないもの: 1521 D1 の `batch-plan.json` schema、同 D2 の per-scope Σ 照合と単独寄与者免除、
同 D3 の判定式そのもの（`batch-plan.json` 不在 = error を含む）、0358 D1 の依存宣言 schema と
file-internal 不変条件。

## Rejected Alternatives

- **settle 済みタスクの宣言を禁止する**: 宣言集合が未 settle 集合と完全一致し表現が一意になるが、
  consumption 中に member が settle するたび宣言の書き換えが必要になり、settle 済み member が宣言に
  残る前提で成り立つ現在 batch の解決と衝突する。Phase 3 の終端成果物を実装中に書き換える運用は
  1 file = 1 writer の規律にも反する。
- **全タスクを遡って宣言する（適用域を変えない）**: 追加宣言のみで済むが、正直な宣言が ceiling
  照合を通らない。通すには過去の差分への分解不能理由の後付けか ceiling の引き上げが必要で、
  遡及なしの原則に反する。
- **settle 済み寄与者を ceiling 照合から除外する**: 遡及宣言を通せるようになるが、Σ 照合に
  「いつ commit されたか」という時点依存を持ち込み、同一ファイルに対する gate の結果が実装の進行で
  変わる。適用域は宣言の要求側で絞るほうが照合式を純粋に保てる。
- **`batch-plan.json` 不在時の admission を graceful skip にする**: 導入時の行き詰まりは解けるが、
  遷移が無判定で通る経路を作り、機構の目的そのものを壊す。不在 = error は不変とする。
- **未宣言を違反ではなく警告に留める**: 適用域を考えずに済むが、未 settle タスクの batch 帰属という
  admission が依拠する性質まで強制されなくなり、強制力の梯子を一段下げる。
- **reopen は「宣言に存在しない」ことのみを拒否理由とする（宣言済みタスクの reopen は無判定）**:
  D1 の不変条件は守れるが、宣言済み settle 済みタスクの reopen が ceiling 照合を経ずに現在 batch へ
  差分を積む経路として残る。拒否理由も membership とは別の分類を新設する必要があり、判定式が二本に
  分かれる。適用対象の遷移集合を広げるだけなら、不変条件と累積ガードの双方が既存の判定式で閉じる。

## Consequences

- Good: 機構の導入 track と実装途中で upgrade した consumer track の双方が、残作業のみを宣言する
  `batch-plan.json` を起票することで admission を正規に通過できる。不在 = error は一切緩めない。
- Good: 検査が要求する宣言と、遡及なしの原則が一致する。歴史を宣言できないまま要求される状態が消える。
- Neutral: 新規 track の Phase 3 では全タスクが todo であるため実効挙動は不変で、計画時点では
  引き続き全タスクの宣言が必須である。
- Neutral: 宣言された settle 済みタスクの扱いは変わらない（Σ 照合・免除・実在検査はそのまま）。
- Neutral: admission の guard 条件が「候補が todo である」から「in_progress へ入る遷移である」へ
  広がる（実装レーンの一回限りの変更）。todo からの遷移についての判定結果は不変である。
- Bad: settle 済みタスクの batch 帰属が任意になるため、過去の batch 編成が成果物として残らない
  track が生じ、実測との突合を後から辿れる範囲が狭まる。
- Bad: 閉じた batch の member や未宣言タスクの reopen は、先に Phase 3 成果物の宣言更新を要する。
  軽微な差し戻しでも計画への一往復が生じる。

## Reassess When

- 未 settle 集合の解決が遷移経路以外（並列実装など）で分岐し、宣言対象の定義が一意でなくなった場合。
- 過去の batch 編成の記録を次回計画の入力として使う運用が生まれた場合（宣言の任意性の見直し）。
- reopen が常態化し、宣言更新の往復が計画作業を圧迫する場合（reopen の判定条件の見直し）。

## Related

- `knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D2 / D3 — refines 対象
  （D2: 検査項目の適用域 / D3: 判定対象の遷移集合）。同 D1 は不変。
- `knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md` D2 —
  refines 対象（依存順序検査の適用域）。同 D1 は不変。
- `knowledge/adr/2026-07-30-0951-batch-plan-scope-name-config-validation.md` — 同じ検査集合に項を
  追加した先行 delta。本 ADR は項を追加せず、既存項の適用域を絞る。
- `knowledge/conventions/enforce-by-mechanism.md` — 不在 = error を緩めず適用域で解く判断の原則。
