---
adr_id: "2026-07-28-1521-scope-diff-ceiling-admission-enforcement"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 / chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 Phase 0 裁定境界の承認（収束文面の承認 — Context の分割不能性の記述と D3 実測ガード bullet への review diff base 明確化を含む）"
    candidate_selection: "from:[separate-batch-plan-artifact,impl-plan-json-embedded-schema-2,task-estimates-only-no-batch-declaration] chose:separate-batch-plan-artifact"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 / chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 Phase 0 裁定境界の承認（収束文面の承認 — Context の分割不能性の記述と D3 実測ガード bullet への review diff base 明確化を含む）"
    candidate_selection: "from:[binary-justification-field,waiver-verifier-llm-judgment] chose:binary-justification-field"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 / chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 Phase 0 裁定境界の承認（収束文面の承認 — Context の分割不能性の記述と D3 実測ガード bullet への review diff base 明確化を含む）"
    candidate_selection: "from:[declared-batch-membership,emergent-batch-no-declaration] chose:declared-batch-membership"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-28 / chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 Phase 0 裁定境界の承認（収束文面の承認 — Context の分割不能性の記述と D3 実測ガード bullet への review diff base 明確化を含む）"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 / chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 Phase 0 裁定境界の承認（収束文面の承認 — Context の分割不能性の記述と D3 実測ガード bullet への review diff base 明確化を含む）"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 / chat_segment:session-012hW6t9KKjqZBWQeUVBLE6Z:2026-07-29 Phase 0 裁定境界の承認（収束文面の承認 — Context の分割不能性の記述と D3 実測ガード bullet への review diff base 明確化を含む）"
    candidate_selection: "from:[planning-domain-only,admission-headroom-reserve,post-impl-advisory-logging] chose:planning-domain-only"
    status: proposed
---
# per-scope diff ceiling を実装開始前の admission で機構強制する

## Context

review 収束コストは差分サイズに対して超線形（O(N) 理解 × O(N) findings ≒ O(N²)）に増える。
per-scope diff ceiling はこの review 収束時間を守るための計画値だが、現状の強制力は次の状態にある。

- 設定 SSoT（`.harness/config/review-scope.json` の `default_diff_ceiling_lines` /
  per-group `diff_ceiling_lines`）とアクセサ
  `ReviewScopeConfig::diff_ceiling_for_scope` は実装済みだが、**production の呼び出し元が
  存在しない**。値を消費するのは workflow 正本の散文を読む LLM のみである。
- `.harness/capabilities/impl-planner.md` は上限を literal（`<500`）でハードコードし、
  scope 単位でなく per-task commit 単位で記述し、見積りの産出義務がなく、
  test obligation 由来の試験行数を視野に入れていない。
- `.harness/workflows/track/full-cycle.md` の batch 計画（Step 0b）と actual-diff guard
  （Step 1b）はいずれも advisory で、超過は log して通す。検出が review 時点に偏り、
  その時点では実装済みのため是正手段が「実装やり直し」か「収束困難な review を飲む」の
  二択になる。
- task は自然な実装単位、batch はそれを束ねた commit / review の単位であり、ceiling が
  制約すべきは review が読む batch の per-scope 累積 diff である。しかし batch を宣言する
  場所が存在しない — batch 編成は full-cycle Step 0b の greedy アルゴリズム（散文）を
  実行時に LLM が頭算するだけで、成果物にならず review もされない。
- 過去の merge 済み track の review 記録では、reviewer が 2.6〜3.9 倍の scope 超過を
  split candidate として正しく指摘している。検出精度ではなく検出位置が問題である。

`knowledge/conventions/enforce-by-mechanism.md` の梯子（型 > CI gate > hook > lint > docs）に
照らすと、ceiling は現在最下段の docs にしか存在しない。

一方で、累積済みの scope diff を事後に task / responsibility / hunk 単位へ帰属させて
独立 review に分割する機構は存在しない（review の hash / verdict は、その時点の review diff
base 起点で累積した scope 全体を一単位とする）。ただし review diff base は commit 時に更新され
（guarded commit gate の終端で HEAD へ進む）、commit 済みの差分は後続 review の対象から外れる
（この更新が失われたときの縮退は D3 に記す）。
分割不能なのは「一つの review diff base の内側で積み上がった差分」であって、commit で区切られた
差分列ではない。したがって強制が意味を持つのは「diff がまだ存在しない、または止めれば累積を
防げる」地点、すなわち**実装開始前**に限られる。

なお ceiling 遵守のためにタスク数を減らす目的で Phase 2 の型設計（catalogue の entry 粒度）を
歪めてはならない。obligation 量は Phase 3 のタスク分解の入力であって Phase 2 の設計制約では
なく、正しい設計が多くの obligation を生むなら答えはタスクを増やすことである（user 裁定
2026-07-26）。上限を伝えてよい先は impl-planner と実装系 workflow に限る。

## Decision

### D1: `batch-plan.json` — 見積り・分解不能理由・batch 宣言を第 4 の終端計画成果物として必須化

Phase 3 の計画成果物に、impl-planner が書く第 4 の artifact
`track/items/<id>/batch-plan.json` を追加する。task は自然な実装単位、batch はそれを
束ねた commit / review の単位であり、ceiling の制約が載るのは batch である。

Phase 3 の authoring は依存順に進む: task 分解（`impl-plan.json`）→ spec カバレッジ
（`task-coverage.json`）→ 契約帰属（`task-contract.json`）→ 見積り → batch 編成。
見積りと batch は全成果物の終端導出物であるため、先頭の `impl-plan.json` に埋め込まず
終端の専用 artifact に置く — **書き順 = 依存順**となり、各ファイルは一回書きで済み、
必須フィールド欠落による schema-invalid な中間状態が発生しない。**`impl-plan.json` の
schema は変更しない（schema 1 のまま）。**

`batch-plan.json` の内容:

- **task 見積り**: 各タスクは触れる review scope ごとに `production_lines` と
  `test_lines` を分けて申告する。`test_lines` は test obligation 由来の試験コードを含む
  合算である。タスクごとの obligation 件数は `obligations.json`（`target_entry` が
  catalogue entry を指す）と `task-contract.json`（task → entry 帰属）の join で機械的に
  導出できるため、impl-planner の判断は 1 件あたり行数の乗数に限定される。
- **batch 宣言（`batches[]`）**: 順序付きの batch 列を宣言する。各 batch は member task id
  の列のみを宣言し、すべてのタスクはちょうど 1 つの batch に属する。**batch は行数を
  宣言しない** — batch の per-scope 合計は member task 見積りの Σ として機械導出する
  （二重宣言の禁止）。
- **分解不能理由**: 見積りが当該 scope の resolved ceiling を超えるタスクは、非空の
  分解不能理由フィールドを必須とし、かつ **その超過 scope について batch 内唯一の寄与者で
  あること**（同じ batch に、当該 scope へ見積りを申告する他タスクを置けない）を構造制約と
  する。ceiling は scope 別の制約であるため、**他 scope のみに触れるタスクとの同居は
  妨げない**。通常タスクでは当該フィールドは空（null）。

見積りの欠落、batch への無所属 / 重複所属など file 内で表現できる不整合は codec が
fail-closed で拒否する。task id が `impl-plan.json` の tasks と一致することなどの
cross-file 整合は D2 の構造検査が担う（`task-coverage.json` の前例と同型）。
graceful skip は設けない。

遡及なし: 既存 track 成果物は歴史記録として再検証しない。新 gate は導入後の active
track にのみ適用し、active track での `batch-plan.json` 欠落は error である。
書き手は impl-planner（1 file = 1 writer）で、admission を判定する CLI は読むだけである。

```jsonc
// <!-- illustrative, non-canonical -->
{
  "task_estimates": [
    {
      "task_id": "T003",
      "scope_estimates": [
        { "scope": "infrastructure", "production_lines": 220, "test_lines": 180 }
      ],
      "oversize_justification": null
    }
  ],
  "batches": [
    { "id": "B1", "task_ids": ["T001", "T002"] },
    { "id": "B2", "task_ids": ["T003"] }
  ]
}
```

### D2: Phase 3 gate — batch 単位の ceiling 照合を fail-closed、妥当性は impl-plan review のレーン

Phase 3 の終端 gate として、`batch-plan.json` の宣言された各 batch について member task
見積りの per-scope Σ（`production_lines + test_lines`）を `diff_ceiling_for_scope` の
解決値と照合する binary check を追加する。

- ある batch の per-scope Σ が ceiling を超える → fail-closed（Phase 3 を完了させない）。
  免除は「当該 scope の寄与者が理由付き超過タスク 1 件のみ」の場合に限る — 超過は
  scope 別に判定し、同じ batch でも他 scope の Σ は通常どおり検査する。救済は batch 編成
  またはタスクの割り直しで、未実装のため是正コストは最小。
- cross-file の構造検査もあわせて行う: `impl-plan.json` の全タスクがちょうど 1 つの
  batch に属す / 見積り・batch member の task id が実在のタスクを指す / タスクの依存先が
  同一または先行の batch にある / 超過タスクがその超過 scope の batch 内唯一の寄与者で
  ある。
- 機構としてはここまでを検査して通し、**分解不能理由の妥当性と batch 編成の妥当性の判定は
  impl-plan scope の review に委ねる**。gate は構造適合のみを検査し、本文の意味判定は
  reviewer のレーンという既存分業（pre-review task-contract gate と同型）に従う。
- impl-plan review の指示書には、分解不能理由の妥当性検査、batch 編成の妥当性検査、
  独立に検証可能な振る舞い単位での実装前分割提案を明示的に含める。

### D3: `track transition` の todo → in_progress 遷移に admission 判定を内蔵する

タスク状態遷移の唯一の経路である `bin/sotp track transition` の todo → in_progress 遷移に、
membership 検査と実測ガードの 2 段の admission 判定を組み込む。

- **(i) membership 検査**: 候補タスクが現在 batch（`batch-plan.json` の宣言順で最先行の、
  未 commit の member を残す batch）の member であること。後続 batch のタスクは現在 batch の commit 成立まで
  遷移を拒否する — 宣言 batch の**合併**を構造的に禁止する。
- **(ii) 実測ガード**: 候補タスクが見積りを申告する scope ごとに、先行寄与
  （HEAD 起点の実測 per-scope diff + 現在 in_progress の member の申告見積りの和）と、
  それに候補タスクの申告見積りを加えた合計を求め、resolved ceiling と比較する。
  **先行寄与が 0 の scope では候補は 1 件目の寄与者であり、見積りの大小によらず admit
  する** — ブロックするのは累積のみである。先行寄与が 0 でない scope で合計が ceiling を
  超えるなら遷移を拒否し、現在 batch をそのタスク境界で早期に閉じる
  （DFP → Review → Commit）。候補が積み増さない scope は照合対象外であり、その scope の
  既超過は遷移を妨げない。commit gate は commit 成立後に review の diff base をその commit へ
  進めるため、admission の実測 baseline と review の対象範囲は同じ地点で 0 に戻る。残る member
  は後続の batch として続行し、その review は先行 batch の commit 済み差分を含まない。ただし
  diff base の解決は per-track の commit 記録（`.commit_hash`）に依存し、それが欠落・破損・
  HEAD の非祖先である場合は base branch へ縮退する — このとき先行 batch の commit 済み差分は
  review 対象へ戻る。縮退の向きは review 対象が広がる安全側であり、本決定が課す遷移判定
  （何を admit / 拒否するか）はこの縮退の影響を受けない。
  **runtime に許されるのは宣言 batch の分割のみで、合併は許されない**
  （分割は review 単位を小さくする安全側の逸脱）。書きかけ分と見積りの二重計上は
  許容する — 誤差は batch が小さくなる安全側に倒れる。
- 各 scope の 1 件目の寄与が常に admit されるため deadlock は原理的に発生せず、理由付き
  超過タスクも（宣言上その超過 scope の唯一の寄与者であるため）この経路で通常どおり
  進行する。
- 判定は算術と membership 照合のみで行い、gate 経路に LLM 判定を挟まない。見積り欠落
  タスクの遷移は error とする（graceful skip なし）。
- full-cycle の Step 0b（runtime の greedy batch 編成）は削除され、宣言された `batches[]`
  の宣言順消化に置き換わる。

タスク状態遷移が単一経路であるため、この判定はどの workflow / capability から到達しても
迂回できない。散文の読み落としによる素通りを構造的に塞ぐ。

### D4: implementer の着手前 precondition — in_progress 未遷移なら orchestrator へ返す

implementer 正本に、コードを書き始める前の precondition を追加する: 担当タスクが
`impl-plan.json`（SSoT。rendered view の `plan.md` ではない）上で `in_progress` に
なっていることを確認し、満たされていなければ実装せず orchestrator へ返して遷移を促す。

- これは実装開始前のチェックであり手戻りを生まない。
- orchestrator が遷移を経ずに実装を dispatch する経路（D3 の迂回）を implementer 側から
  塞ぎ、遷移 = admission 判定を事実上必須化する。

### D5: 行数定義の CLI 持ち上げ

scope 行数の数え方の正本を workflow 散文から CLI 実装へ移す。定義は full-cycle Step 1b の
既存定義をそのまま採用する:

- scope の行数 = additions + deletions（batch-base 起点の committed / staged / unstaged を
  合算）。
- untracked ファイルは全行数を additions として計上する。
- scope 分類は既存の review scope 分類機構（`review files --scope` 系）を再利用し、
  再実装しない。

正本散文は数え方を再記述せず CLI を参照する。なお per-scope diff ceiling と
`module_limits.max_lines`（単一ファイル行数）は別指標である — 同一 crate 内のファイル分割は
scope diff を 1 行も減らさない。ceiling 超過への有効な応答はタスク境界の分割のみである。

### D6: ドメイン境界 — ceiling は計画と admission にのみ存在し、実装開始以後には登場しない

ceiling は目安・計画目標であり、その概念が存在してよいのは Phase 3（D1 / D2）と
admission（D3）のドメインに限る。実装開始以後のドメインには行数制限の概念を持ち込まない。

- full-cycle Step 1b（actual-diff guard: 実装後〜review 前の実測照合と advisory log）は
  削除する。実測が読まれるのは D3 の admission baseline としてのみ。
- review 入口・commit 時に ceiling 照合を行わない。review fix / DFP による差分増加は
  正当な挙動の帰結であり、「超過」として扱わない。手戻りは許容しない。
- admission に余裕率（headroom reserve）の概念も設けない。

## Rejected Alternatives

- **prompt-only の soft 抑制（70% 目標を複数正本へ散文複製）**: magic constant が散文に
  重複して config に追随せず、正本を読まない経路に迂回され、enforce-by-mechanism の
  最下段に留まる。予防の狙い（実装前に閉じる）は D1〜D3 が機構で吸収する。
- **commit 時の実測 fail-closed gate**: guarded commit に到達した時点で review は収束済みで
  あり、守る対象（review 収束時間）のコストは支払い済み。発火しても収束済み review を
  無駄にするだけで、逃げ道が「ceiling を上げる」に傾く。review 入口で止めても累積 diff は
  分割不能で deadlock になる。
- **実装中の implementer escalation（超過検知で中断して返す）**: implementer は catalogue
  契約が要求する実装と obligation を削れず、「残りを書かない」選択肢がない。半実装の
  working tree は CI / review / commit のいずれにも進めず、返した先の是正は実装やり直し
  （手戻り）しかない。
- **遷移時の waiver-verifier（LLM 判定）による超過例外の検証**: gate 経路に LLM 判定を
  挟むと遅く不確実になり、判定者を新設する必要がある。binary な理由フィールド検査
  （D1 / D2）+ reviewer レーンでの妥当性判定で足り、機械 gate を騙る形の waiver 濫用も
  構造上成立しない。
- **admission headroom（余裕率の config キー）**: review fix 増分に備えて ceiling 手前で
  閉じる案だが、実装開始以後に ceiling 概念を持ち込む前提自体が誤り（D6）。rfl による
  差分増加は正当であり、予約枠で「守る」対象ではない。
- **見積り・batch 宣言を `impl-plan.json` へ埋め込む（schema 2 化）**: 見積りと batch は
  Phase 3 authoring の終端導出物（tasks → contract → 見積り → batch）であり、先頭の
  artifact に必須フィールドとして埋め込むと書き順が反転する。二度書きは fail-closed な
  required schema の下で schema-invalid な中間状態を作り（特に obligation 件数導出を CLI
  化すると `task-contract.json` がディスク上に先に必要）、batch の再編成のたびに安定した
  task SoT を書き換えることになる。さらに `impl-plan.json` の破壊的 schema 移行が必要に
  なる。成果物新設の伝播コスト（一回限り）より、これらの構造的欠陥のほうが重い。
- **batch にも予想行数を宣言する**: task 見積りとの二重宣言になり必ず desync する。batch は
  membership のみを宣言し、per-scope 合計は member task 見積りの Σ として機械導出する。
- **batch を宣言せず emergent なまま扱う（実測と task 状態のみで判定する）**: ceiling の
  制約対象である review 単位が機械実体にならず、「現在の batch」を CLI が解決できない。
  batch 編成も runtime の greedy 頭算（無レビュー）に残る。宣言化により編成が Phase 3 の
  review 対象へ入る。
- **batch-first 既定の反転（1 task ずつを既定に戻す）**: feature batch 既定の目的
  （review / CI / commit の round 分断解消）は、admission の機械化と両立する。ceiling の
  内側で batch を積む判断が機械になれば、既定の反転は不要。

## Consequences

- Good: `diff_ceiling_for_scope` に初めて production の呼び出し元ができ、ceiling が
  データから gate になる。
- Good: fail-closed がすべて実装開始前（Phase 3 と遷移時）に置かれ、是正コストが最小で
  手戻りが発生しない。
- Good: batch 編成が runtime の無レビュー頭算から Phase 3 の計画成果物へ移り、
  impl-plan scope review の検査対象に入る。
- Good: 各 scope の 1 件目の寄与の常時通過（D3）により deadlock が原理的に起きず、
  分解不能な大タスクも超過 scope の単独寄与者として通常進行できる。
- Good: タスク状態遷移が単一経路のため、admission は散文を読まない経路からも迂回できない。
- Good: gate 経路に LLM 判定がなく、判定が決定的・高速・安価。
- Bad: 見積りは自己申告であり、scope 1 件目の寄与タスクの見積り大外れは機構では塞げない。
  これは受容する残余リスクとし、admission の拒否記録と実測 baseline を次回計画の入力に
  返す。
- Bad: impl-planner の作業コストが増える — 見積りに加えて batch 編成も担う（緩和:
  obligation 件数は機械導出、判断は乗数と束ね方に限定される）。
- Bad: 成果物ファイルの新設に伴い、workflow 正本・adapter・peer capability 契約
  （writes-forbidden 列を含む）への一回限りの伝播が必要になる。一方 `impl-plan.json` の
  schema 移行は発生せず、既存成果物との互換は保たれる。
- Neutral: full-cycle Step 0b（greedy batch 編成）は宣言された `batches[]` の宣言順消化に
  置き換わり、Step 1b は削除される。impl-planner 正本の literal 上限値は config 参照に
  置き換わる。

## Reassess When

- impl-planner の申告見積りと実測の乖離が常態化した場合（見積り方式・ceiling 値・
  乗数指針の見直し）。
- 並列実装運用で admission の実測 baseline が干渉し、判定の意味が失われる場合
  （baseline の定義または判定式の再設計）。
- 分解不能理由付きの超過タスクが頻発する場合（ceiling 値または分解指針の見直し）。

## Related

- `knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md` — feature batch 既定の
  前提。本 ADR はこれを反転せず、batch admission を機械化して両立させる。
- `knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md` —
  「gate は構造適合のみ、意味判定は reviewer のレーン」という分業の前例（D2 が踏襲）。
- `knowledge/adr/2026-06-06-1609-enforce-module-size-limit-splitting.md` — module-size は
  別指標（D5 の注記）。`verify module-size` は設定値×実測の fail-closed gate の実装前例。
- `knowledge/conventions/enforce-by-mechanism.md` — 本 ADR の強制力設計の原則。
