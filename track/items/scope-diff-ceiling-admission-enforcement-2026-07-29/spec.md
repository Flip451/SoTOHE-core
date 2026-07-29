<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 66, yellow: 0, red: 0 }
---

# per-scope diff ceiling の実装開始前 admission 強制

## Goal

- [GO-01] per-scope diff ceiling を、設定値を読む production の呼び出し元が存在しない散文上の助言値から、宣言された計画値を機械が fail-closed で照合する gate へ引き上げる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3]
- [GO-02] ceiling の fail-closed 判定をすべて実装開始前（Phase 3 の計画終端と todo → in_progress 遷移）に配置し、超過への是正手段を未実装のタスク境界・batch 編成の引き直しに限定して手戻りを発生させない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6]
- [GO-03] ceiling の制約対象である batch（commit / review の単位）を runtime の無レビュー頭算から Phase 3 の宣言成果物へ移し、batch 編成と分解不能理由を impl-plan scope review の検査対象にする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2]

## Scope

### In Scope
- [IN-01] `track/items/<id>/batch-plan.json` を Phase 3 の第 4 の終端計画成果物として新設する。書き手は impl-planner のみ（1 file = 1 writer）で、admission を判定する CLI は読むだけである。Phase 3 の authoring は task 分解 → spec カバレッジ → 契約帰属 → 見積り → batch 編成の依存順に進み、各ファイルを一回書きで完結させる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T007, T008, T013, T016]
- [IN-02] `batch-plan.json` に task 見積りを持たせる。各タスクは触れる review scope ごとに `production_lines` と `test_lines` を分けて申告し、`test_lines` は test obligation 由来の試験コードを含む合算とする。タスクごとの obligation 件数は obligation 成果物と契約帰属成果物の join で機械的に導出できるため、impl-planner の判断は 1 件あたり行数の乗数に限定される。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T001, T007]
- [IN-03] `batch-plan.json` に順序付きの batch 列 `batches[]` を宣言させる。各 batch は member task id の列のみを宣言し、すべてのタスクはちょうど 1 つの batch に属する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T001, T007]
- [IN-04] 見積りが当該 scope の resolved ceiling を超えるタスクは非空の分解不能理由を伴い、通常タスクは分解不能理由を伴わない。両者の区別は `batch-plan.json` 上で機械可読に表現するが、その表現形は本契約では固定しない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T001, T007]
- [IN-05] `batch-plan.json` の codec が、見積りの欠落・batch への無所属・複数 batch への重複所属など file 内で表現できる不整合を fail-closed で拒否する。graceful skip は設けない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T002, T007]
- [IN-06] Phase 3 の終端 gate として、宣言された各 batch について member task 見積りの per-scope Σ（`production_lines + test_lines`）を `diff_ceiling_for_scope` の解決値と照合する binary check を追加する。超過は fail-closed で Phase 3 を完了させず、免除は当該 scope の寄与者が理由付き超過タスク 1 件のみの場合に限る。超過は scope 別に判定し、同じ batch でも他 scope の Σ は通常どおり検査する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2] [tasks: T003, T006, T012]
- [IN-07] 同 gate が cross-file の構造検査もあわせて行う: `impl-plan.json` の全タスクがちょうど 1 つの batch に属す / 見積りと batch member の task id が実在のタスクを指す / 宣言されたすべての依存辺について、依存先タスクの所属 batch が依存元タスクの所属 batch と同一であるか `batches[]` の宣言順でそれより先行する / 超過タスクがその超過 scope について batch 内唯一の寄与者である。依存辺の検査の入力は `impl-plan.json` の依存宣言と `batches[]` のみであり、`batch-plan.json` 側に新しい宣言フィールドを要求しない。依存を宣言していないタスク対は検査の対象外である。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D2] [tasks: T003, T006, T008, T019]
- [IN-08] impl-plan scope の review 指示書に、分解不能理由の妥当性検査、batch 編成の妥当性検査、独立に検証可能な振る舞い単位での実装前分割提案を明示的に含める。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2] [tasks: T014]
- [IN-09] `bin/sotp track transition` の todo → in_progress 遷移に membership 検査を組み込む。候補タスクが現在 batch（`batch-plan.json` の宣言順で最先行の、未 commit の member を残す batch）の member であることを要求し、後続 batch のタスクは現在 batch の commit 成立まで遷移を拒否する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T018]
- [IN-10] 同遷移に実測ガードを組み込む。候補タスクが見積りを申告する scope ごとに、先行寄与（HEAD 起点の実測 per-scope diff + 現在 in_progress の member の申告見積りの和）に候補タスクの申告見積りを加えた合計を resolved ceiling と比較する。先行寄与が 0 の scope では見積りの大小によらず admit し、先行寄与が 0 でない scope で合計が ceiling を超えるなら遷移を拒否して現在 batch をそのタスク境界で早期に閉じる。候補が積み増さない scope は照合対象外であり、その scope の既超過は遷移を妨げない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T009, T018]
- [IN-11] admission 判定を算術と membership 照合のみで行い、見積り欠落タスクの遷移を error とする（graceful skip なし）。runtime に許すのは宣言 batch の分割のみで、合併は許さない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T018]
- [IN-12] full-cycle の runtime greedy batch 編成ステップを削除し、宣言された `batches[]` の宣言順消化に置き換える。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T017]
- [IN-13] implementer 正本に着手前 precondition を追加する。担当タスクが `impl-plan.json`（SSoT。rendered view の `plan.md` ではない）上で `in_progress` になっていることを確認し、満たされていなければ実装せず orchestrator へ返して遷移を促す。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D4] [tasks: T015]
- [IN-14] scope 行数の数え方の正本を workflow 散文から CLI 実装へ移す。定義は既存の実測ガードのものをそのまま採用する: scope の行数 = additions + deletions（base 起点の committed / staged / unstaged を合算）、untracked ファイルは全行数を additions として計上、scope 分類は既存の review scope 分類機構を再利用する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D5] [tasks: T009]
- [IN-15] 正本散文から行数の数え方の再記述を除去し、CLI を参照する形に改める。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D5] [tasks: T017]
- [IN-16] full-cycle の実装後〜review 前の actual-diff guard（実測照合と advisory log）を削除する。実測が読まれるのは admission の baseline としてのみとする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T017]
- [IN-17] impl-planner 正本の literal な上限値を config 参照へ置き換え、per-task commit 単位ではなく scope 単位の記述に改め、見積りの産出義務と test obligation 由来の試験行数を扱いに含める。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2] [tasks: T013]
- [IN-18] 成果物新設に伴う一回限りの伝播を同一変更集合で行う: workflow 正本、command / skill adapter、peer capability 契約（writes-forbidden 列を含む）を `batch-plan.json` の新設に合わせて更新する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T016]
- [IN-19] `impl-plan.json` を schema 2 へ拡張し、task entry に省略可能な `depends_on`（同一ファイル内の task id の列）を追加する。意味は「そのタスクの実装は列挙されたタスクの完了を前提とする」であり、省略と空列はいずれも未宣言と読む。読み取りは schema 1 / 2 の双方を受理して schema 1 は未宣言として読み、書き出しは schema 2 とする。書き手は impl-planner のみで、依存宣言はタスク分解と同じ一回の書き込みで確定する。 [adr: knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D1] [tasks: T019, T020]
- [IN-20] `impl-plan.json` の codec に、依存宣言に関する file-internal 不変条件 3 件を fail-closed で追加する: `depends_on` の各 id が同一ファイルの `tasks[]` に実在する（参照の実在）/ 宣言された依存関係が閉路を含まない（自己参照を含む、非循環）/ plan order が宣言された依存グラフの線形拡大である。3 件目は宣言と実行順の食い違いを構造的に排除し、前提未完了のタスクを先に配る計画を表現不能にする。 [adr: knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D1] [tasks: T019, T020]

### Out of Scope
- [OUT-01] 見積りと batch 宣言を `impl-plan.json` へ埋め込むことは対象外とする。これらは Phase 3 authoring の終端導出物であり `batch-plan.json` に置く。`impl-plan.json` の schema 拡張はタスク間の依存宣言に限る。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D1] [tasks: T019, T020]
- [OUT-02] 既存の track 成果物および archive 済み track への遡及適用と再検証は対象外とする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1]
- [OUT-03] commit 時の実測 fail-closed ceiling gate、および review 入口での ceiling 照合の追加は対象外とする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T016]
- [OUT-04] admission の余裕率（headroom reserve）概念と、そのための config キーの導入は対象外とする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6]
- [OUT-05] 実装中の implementer による超過検知・中断・escalation の導入は対象外とする。implementer 側に追加するのは着手前の precondition のみである。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D4, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T015]
- [OUT-06] 遷移時の waiver-verifier（LLM 判定による超過例外の検証）と、そのための判定者の新設は対象外とする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3]
- [OUT-07] 分解不能理由の妥当性と batch 編成の妥当性を機構が判定することは対象外とする。依存宣言の網羅性 — 実際の実装前提がすべて `depends_on` に宣言されているか — の機械検査も対象外とし、未宣言のタスク対は batch 順序検査を素通りする。これらはいずれも impl-plan scope review のレーンに残す。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D2] [tasks: T014]
- [OUT-08] review scope 分類機構の再実装は対象外とする。既存機構を再利用する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D5] [tasks: T009]
- [OUT-09] feature batch 既定の反転（1 task ずつの消化を既定に戻すこと）は対象外とする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T017]
- [OUT-10] 見積りの自己申告性を機構で検証すること（scope の 1 件目の寄与タスクの見積り大外れを塞ぐ機構）は対象外とする。受容する残余リスクとして扱い、admission の拒否記録と実測 baseline を次回計画の入力に返すに留める。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3]
- [OUT-11] 累積済みの scope diff を事後に task / responsibility / hunk 単位へ帰属させて独立 review に分割する機構の新設は対象外とする。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T017]

## Constraints
- [CN-01] batch は行数を宣言しない。batch の per-scope 合計は member task 見積りの Σ として機械導出し、二重宣言による desync を作らない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T001, T002]
- [CN-02] 機構は構造適合のみを検査する。分解不能理由の妥当性と batch 編成の妥当性という本文の意味判定は reviewer のレーンに委ね、gate 経路に LLM 判定を挟まない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T003, T014]
- [CN-03] 各 scope の 1 件目の寄与は常に admit される。ブロックするのは累積のみであり、この性質により admission に deadlock は原理的に発生せず、理由付き超過タスクも通常どおり進行できる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004]
- [CN-04] runtime に許される宣言 batch からの逸脱は分割のみで、合併は許されない。書きかけ分と見積りの二重計上は許容し、誤差は batch が小さくなる安全側に倒す。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T018]
- [CN-05] admission の実測 baseline は per-track の commit 記録に依存し、それが欠落・破損・HEAD の非祖先である場合は base branch へ縮退する。縮退は review 対象が広がる安全側であり、本 track が課す遷移判定（何を admit / 拒否するか）はこの縮退の影響を受けない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T009]
- [CN-06] ceiling の概念が存在してよいのは Phase 3 の計画と admission のドメインに限る。実装開始以後のドメインには行数制限の概念を持ち込まず、review fix / DRY fix による差分増加を「超過」として扱わない。手戻りは許容しない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T016, T017]
- [CN-07] ceiling 遵守のためにタスク数を減らす目的で Phase 2 の型設計（catalogue の entry 粒度）を歪めない。obligation 量は Phase 3 のタスク分解の入力であって Phase 2 の設計制約ではなく、上限を伝えてよい先は impl-planner と実装系 workflow に限る。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T013]
- [CN-08] per-scope diff ceiling と単一ファイル行数上限は別指標である。同一 crate 内のファイル分割は scope diff を 1 行も減らさないため、ceiling 超過への有効な応答はタスク境界の分割のみである。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D5] [tasks: T013, T017]
- [CN-09] 遡及なし。新 gate は導入後の active track にのみ適用し、active track での `batch-plan.json` 欠落は error とする。既存 track 成果物は歴史記録として再検証しない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T006, T008]
- [CN-10] admission 判定はタスク状態遷移の単一経路上に置き、どの workflow / capability から到達しても迂回できない状態を保つ。implementer 側の着手前 precondition が、遷移を経ない実装 dispatch という迂回経路を塞ぐ。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D4] [tasks: T015, T018]

## Acceptance Criteria
- [ ] [AC-01] `track/items/<id>/batch-plan.json` が Phase 3 の計画成果物として存在し、その書き手が impl-planner に限定されている（peer capability 契約の writes-forbidden 列に列挙され、admission を判定する CLI は読み取りのみ行う）。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T007, T008, T013, T016]
- [ ] [AC-02] `batch-plan.json` の各タスク見積りが、触れる review scope ごとに `production_lines` と `test_lines` を分けて保持し、`test_lines` が test obligation 由来の試験コードを含む合算として定義されている。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T001, T007]
- [ ] [AC-03] `batches[]` が順序付きの batch 列であり、各 batch が member task id の列のみを宣言して行数を宣言しない。ceiling 超過タスクが非空の分解不能理由を伴い、通常タスクが分解不能理由を伴わないことを、`batch-plan.json` から機械可読に判別できる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T001, T007]
- [ ] [AC-04] 見積りの欠落、batch への無所属、複数 batch への重複所属という file 内不整合のそれぞれについて、codec が fail-closed で拒否し graceful skip しないことを確認できる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T002, T007]
- [ ] [AC-05] 既存成果物との互換が読み取り側で保たれ（schema 1 の `impl-plan.json` が引き続き decode でき、active でない track の registry / views 描画が壊れない）、既存 track 成果物が再検証されない。gate 系の検証対象は active track に限られ、導入後の active track で `batch-plan.json` が欠落している場合は error になる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D1] [tasks: T006, T008, T020]
- [ ] [AC-06] 宣言された batch の per-scope Σ が resolved ceiling を超える `batch-plan.json` に対し、Phase 3 の binary check が fail-closed で ERROR を返す。当該 scope の寄与者が非空の分解不能理由を持つタスク 1 件のみである場合は通す。同じ batch の他 scope の Σ は通常どおり検査される。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2] [tasks: T003, T006, T012]
- [ ] [AC-07] cross-file 構造違反のそれぞれ — どの batch にも属さないタスク、実在しないタスクを指す task id、依存先タスクの所属 batch が依存元タスクの所属 batch より後にある宣言済み依存辺、超過タスクがその超過 scope の唯一の寄与者でない batch — について、同 check が ERROR を返す。依存を宣言していないタスク対の batch 割当は同 check の結果を変えない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D2] [tasks: T003, T006, T008, T019]
- [ ] [AC-08] ceiling が解決されない scope については Σ の照合が行われず、その scope の合計値の大小が Phase 3 gate の結果を変えない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2] [tasks: T001, T003, T009]
- [ ] [AC-09] impl-plan scope の review 指示書に、分解不能理由の妥当性検査、batch 編成の妥当性検査、独立に検証可能な振る舞い単位での実装前分割提案の 3 項目が明示的に含まれている。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2] [tasks: T014]
- [ ] [AC-10] 現在 batch（宣言順で最先行の、未 commit の member を残す batch）に属さないタスクの todo → in_progress 遷移が拒否され、後続 batch のタスクは現在 batch の commit 成立後に遷移できる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T002, T004, T018]
- [ ] [AC-11] 先行寄与が 0 の scope では、候補タスクの申告見積りが resolved ceiling を超えていても todo → in_progress 遷移が admit される。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T018]
- [ ] [AC-12] 先行寄与が 0 でない scope で「先行寄与 + 候補タスクの申告見積り」が resolved ceiling を超える場合に遷移が拒否される。候補タスクが見積りを申告しない scope は照合対象外であり、その scope の既超過は遷移を妨げない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T018]
- [ ] [AC-13] `batch-plan.json` に見積りを持たないタスクの todo → in_progress 遷移が error になる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T004, T018]
- [ ] [AC-14] admission 判定が算術と membership 照合のみで完結し、判定経路に LLM 判定が存在しない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3] [tasks: T003, T004, T018]
- [ ] [AC-15] full-cycle 正本に runtime greedy batch 編成ステップと実装後の actual-diff advisory guard が存在せず、batch 消化が宣言された `batches[]` の宣言順で行われる。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D3, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T017]
- [ ] [AC-16] implementer 正本に、担当タスクが `impl-plan.json` 上で `in_progress` であることの着手前確認と、満たされない場合に実装せず orchestrator へ返して遷移を促す指示が含まれている。確認対象が rendered view ではなく SSoT であることが明示されている。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D4] [tasks: T015]
- [ ] [AC-17] scope 行数の算出が CLI 側に存在し、additions + deletions の合算、untracked ファイルの全行 additions 計上、既存 review scope 分類機構の再利用という定義で動作する。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D5] [tasks: T009]
- [ ] [AC-18] 正本散文に行数の数え方の再記述が残っておらず、CLI を参照する形になっている。impl-planner 正本には literal な上限値が残っておらず、上限が config 参照として scope 単位で記述されている。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D2, knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D5] [tasks: T013, T017]
- [ ] [AC-19] review 入口と commit gate に ceiling 照合が追加されておらず、review fix / DRY fix による差分増加が「超過」として扱われない。admission に余裕率の概念が存在しない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D6] [tasks: T016, T017]
- [ ] [AC-20] `batch-plan.json` の新設が workflow 正本、command / skill adapter、peer capability 契約の writes-forbidden 列へ同一変更集合で伝播しており、`batch-plan.json` を impl-planner 以外が書ける経路が残っていない。 [adr: knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md#D1] [tasks: T016]
- [ ] [AC-21] `impl-plan.json` の task entry が省略可能な `depends_on` を保持し、省略と空列がいずれも未宣言として読まれる。schema 1 の `impl-plan.json` が引き続き decode でき、書き出しが schema 2 になる。 [adr: knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D1] [tasks: T019, T020]
- [ ] [AC-22] 依存宣言の file-internal 不変条件違反のそれぞれ — `depends_on` が同一ファイルの `tasks[]` に存在しない task id を指す、宣言された依存関係が閉路（自己参照を含む）を含む、plan order が宣言された依存グラフの線形拡大でない — について、codec が fail-closed で拒否し graceful skip しないことを確認できる。 [adr: knowledge/adr/2026-07-29-0358-task-dependency-declaration-and-batch-order-check.md#D1] [tasks: T019, T020]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 66  🟡 0  🔴 0

