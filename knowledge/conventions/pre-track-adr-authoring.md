# Pre-Track ADR Authoring Convention

## Purpose

ADR は track 内成果物ではなく **track 前段階** で author が作成する track 横断資産とする。track のライフサイクル (作成 → 進行 → 完了 → archive) と ADR のライフサイクル (書き換え可だが原則安定、全 track で参照される) を独立に保つため、ADR 生成を `/track:plan` の内部ステップから切り離し、事前確認のみを行う。track 中の意味変更は、入力決定と pipeline 産決定の**二箱分離**で扱う (下記「In-track 意味変更の裁定権」節)。

## Scope

- 適用対象: `knowledge/adr/*.md` の新規作成 / 更新、`/track:plan` の起動条件、adr-editor サブエージェントの invocation、`.claude/commands/**/*.md` および `.claude/skills/**/SKILL.md` 本文からの ADR 参照スタイル。
- 適用外: track 内 research note (`track/items/<id>/research/` 配下)、spec.json や型カタログなど track 内 SSoT 成果物、既に `archive/` に移動した track の ADR 修正。

## Rules

- **配置**: ADR は `knowledge/adr/YYYY-MM-DD-HHMM-slug.md` に配置する。`track/items/` 配下には置かない。track 中に起草する delta 候補 (下記裁定権節) も同じ配置規則に従う。
- **作成タイミング**: ADR ファイルの **初期起草** (knowledge/adr/ への file 作成) は `/track:plan` **起動前** に user + main 対話 (または手動) で完了させる。`/track:plan` は ADR を自動生成しない。**初回 commit はその ADR を最初に必要とする track の Phase 0 (ADR-baseline commit) で行う**のが typical。ADR は track 横断資産であり 1 つの ADR が複数 track にまたがる関係 (ADR ⊇ track) を維持する。
- **起動時の事前確認**: `/track:plan` は起動直後に、参照予定 ADR が `knowledge/adr/` に存在するかを確認する。未整備なら停止し、ADR 整備を促す (厳密モード)。
- **状態フィールドなし**: ADR に `Status` 見出しや `approved` のような状態フィールドは作らない。ファイルが存在して内容が読めることは `/track:plan` の起動前提として十分であるが、Phase 0 の user 承認（下記の裁定境界）を代替しない。
- **書き換え可 (pre-track 文脈に限る)**: track 前段階の user + main 対話 (または手動) で既存 ADR を直接書き換えられるのは、`knowledge/conventions/adr.md` の effective merge target 判定で **pre-merge** の ADR に限る。merge target に既に入った ADR は永続 record であり、意味上の変更は新 ADR で supersede または refinement する。track が当該 ADR を init 刻印した後の書き換えは、下記「In-track 意味変更の裁定権」節に従う (この一文を track 内の包括的な編集許可として読んではならない)。
- **track 内成果物からの参照**: spec.json は ADR を構造化参照 (`AdrRef { file, anchor }`) で cite できる (SoT Chain ① spec → ADR)。**入庫済みの delta draft も cite できるが、入庫前の候補は cite できない** (下記裁定権節)。型カタログ (Phase 2) は spec を `spec_refs[]` で参照し、ADR を直接 cite することは SoT Chain のレイヤースキップになるので禁止 (`type catalogue → ADR` は逆流/スキップ)。impl-plan (Phase 3) も同様に spec / 型カタログ経由で参照し、ADR を直接 cite しない。逆方向 (ADR から track 内成果物への参照) は SoT Chain 逆流なので禁止。
- **Phase 1+ の ADR 側修正 (delta 候補)**: Phase 1+ の探索的精緻化ループで下流 signal が 🔴 になって ADR 側の意味変更が必要になった場合、入力箱 ADR を in-place 編集せず、adr-editor が delta 候補を起草する (下記裁定権節の Phase 1+ 手順)。下流 spec の 🔴 は入庫済み delta draft を cite して解消する。
- **終端処理**: track 終端で入力箱 ADR に不意の乖離 (byte 照合の不一致) があれば守護者トリアージへ送る。入庫済み delta draft は 🟡 のまま merge 段階の user 裁定へ進む。乖離があることだけを理由に同期の accept / revert を user に求めない。
- **main による直接編集の禁止**: track 内での ADR 修正も含めて、main orchestrator が `knowledge/adr/*.md` を直接 Edit してはならない。adr-editor サブエージェントを経由する (1 ファイル = 1 writer 原則)。
- **skill / command からの特定 ADR 参照禁止**: `.claude/commands/**/*.md` および `.claude/skills/**/SKILL.md` の本文では、特定 ADR を file path / 日付付きスラグ (`knowledge/adr/YYYY-MM-DD-HHMM-slug.md`) で直接 cite しない。commands / skills は運用ドキュメントとして自己完結させ、ADR の背景知識を前提とせず動作・条件だけで読者が理解できる粒度で記述する。ADR の decision を workflow に反映する際は、その workflow 文章が ADR を指さなくても意味が通じるように書き直す。設計背景を残したい場合は、変更を持ち込んだ track の commit message / 該当 ADR 側 (Consequences / Related) に記述する。
  - OK: 「`knowledge/adr/` 配下に事前 ADR を配置する」「ADR を参照する spec.json がある場合は…」のような generic / pattern description。
  - NG: 「per ADR `YYYY-MM-DD-HHMM-slug.md` §Dn」のように特定 ADR を日付付きスラグで cite。

## In-track 意味変更の裁定権

ADR の意味は user に属する。track が ADR を扱う全期間について、意味変更の正規経路と裁定点を、入力決定と pipeline 産決定の**二箱分離**で固定する。

用語:

- **入力箱**: track が init 刻印した ADR の集合。Phase 0 裁定境界後は意味を固定する。
- **delta 候補**: Phase 1+ で adr-editor が起草した track-born draft ADR のうち、入庫判定を通過していないもの。下流成果物は cite できない。
- **delta 箱**: 入庫判定を通過した track-born draft ADR の集合。chain ⓪ が 🟡 と評価し、strict merge gate が user 裁定まで merge を止める。
- **Phase 0 裁定境界**: user が収束文面を承認し、境界刻印と ADR-baseline commit が完了した時点。ここから track 終了まで入力箱の意味を固定する。

役割は 4 者で分離する:

| 役割 | 担い手 | 責務 |
|---|---|---|
| 書き手 | adr-editor | 編集の適用・delta 候補の起草改稿・user 裁定の実装編集 (すべて working tree のみ) |
| 守護者 | adr-diagnoser | Phase 0 の編集ごとの決定保存判定 / Phase 1+ の delta 入庫三択判定と非意味的修正の分類 / user 裁定の実装編集の conformance 再監査 / 不意の乖離のトリアージ。決定を壊す提案には**保全代案または修正不要理由の提示義務**を負う (裸の差し止めで終わらない) |
| 配達人 | orchestrator | dispatch・記録・運搬・verdict 準拠の routing のみ。意味の裁定・分類を行わない |
| 裁定者 | user | Phase 0 境界の承認と merge 段階の採用・棄却・監査 |

### Phase 0 — 裁定境界まで

1. **init 刻印**: track init が持ち込み時点の ADR を ledger に init 種別で刻印する。「track に何を持ち込んだか」の記録であり、user が見る diff の基準点。
2. **in-place 収束ループ**: baseline review の findings への修正は、**意味変更を含めて** adr-editor が working tree に適用する。**適用直後に adr-diagnoser が適用済み編集 (concrete diff) を監査する** — 決定を保存する編集のみ採用し、決定を壊すと判定された変更は編集前の文面へ戻す。その際守護者は保全代案または修正不要理由を提示し、orchestrator はそれをレビュアーへそのまま還流する。レビュアーが所見を維持して対立が解消しない場合だけ、orchestrator は裁定せずその対立を手順 3 の user 裁定へ載せる。**ループ中は ledger に一切書き込まない** — 中間刻印は禁止。init 記録との乖離は「レビュー中の draft」を意味する正常状態である。新決定が必要な場合も delta 候補は作らない — user 同席の hearing で決定内容を裁定し、adr-editor がその hearing 内容を実装する (既存 pre-merge 入力 ADR への追記、または post-merge 入力 ADR の場合は hearing の決定を記録する新 ADR ファイルの起草。grounds は hearing を指す `user_decision_ref`)。新 ADR ファイルは orchestrator が init 刻印して入力箱に加え、fresh review の再収束が境界 commit 前にそれを覆う。
3. **zero_findings / adjudication-ready 到達 → user 裁定**: 全 required scope が zero_findings に至ったら、またはすべての **required** 非 ADR scope が zero_findings（approved / not-required は阻害しない）で ADR scope に残る **すべて** の finding が (a) 守護者の差し止めた決定対立かつ保全代案 / 修正不要理由を記録している、または (b) lifecycle を問わず新決定が必要で user-present hearing を必要とする、場合は review workflow を **adjudication-ready** として停止し、user に (a) init 刻印との diff、(b) 守護者が差し止めた提案・hearing-required 提案とその根拠を提示して裁定を仰ぐ。adjudication-ready は commit 許可ではなく、この user 裁定だけの到達点である。この提示は user が実際に読める形で行う — diff の内容 (変更 hunk の原文、または hunk 単位の忠実な要約) を裁定依頼と同じ chat 本文に明示する。tool 出力・ファイル path 参照・添付だけによる提示は裁定依頼の前提を満たさない。finding 単位で user を割り込ませない — user がレビューするのは収束後の全体である。承認後、adr-editor が承認 `user_decision_ref` を対象 decision に適用し、adr-diagnoser が承認 ref を含む文面を再監査し、fresh review で current hash を zero_findings に再収束させる。再収束中に user 裁定済み文面の意味変更が必要な finding が出た場合は自己解決せず user 裁定へ戻す。
4. **境界を閉じる**: 収束文面が init 文面から変わっている場合、orchestrator は **review-refinement 刻印** (review による精緻化の user 承認記録) を一度だけ行う。reason には「review が何を精緻化したか」の自己完結説明と守護者判定要旨のみを記す — 承認 ref は front-matter、hash は ledger field に置き、reason へ重複させない。収束文面が init と同一なら追加刻印は行わない。その後 ADR-baseline commit を成立させる — byte 照合を通過した commit の成立で裁定境界が閉じる。**経過措置**: review-refinement kind の実装まで、既存の escalation kind に reason 冒頭で review-refinement 記録である旨を明記して代用する。これが新規 escalation 刻印の唯一の例外である。

   ここでの「一度だけ」は track 全体で一回ではなく、**各 input-box source ごと**に
   その source 自身の init 記録から収束文面が変わった場合に一回を意味する。hearing
   起草 ADR を含む全 input-box source を走査し、一 source の刻印で別 source の byte
   照合を代替してはならない。
5. Phase 0 で pipeline を止めて user に届けてよいのは、上記裁定と「先へ進めない設計欠陥」のみ。

新 ADR を hearing で起草した場合、adr-diagnoser が hearing 内容への忠実性を
`hearing-conformant` と再監査した後に限り、orchestrator は init 刻印して入力箱に加える。
`deviating` は編集を復元して user に戻し、刻印してはならない。

### Phase 1 以降 — 自律区間 (境界の外側)

- **入力箱の freeze**: 入力箱 ADR への意味的な in-place 編集を禁止する。精緻化を含む意味変更と新決定は、対象 ADR の pre-merge / post-merge を問わず、adr-editor が **delta 候補**として `knowledge/adr/` 配下に起草する。候補には非 user 系根拠 (`review_finding_ref` 等) を記録する。
- **非意味的修正 lane**: 入力箱 ADR への誤字・参照 path 等の修正提案は、adr-editor がまず working tree に適用し、直後に adr-diagnoser が concrete diff を分類する — orchestrator は分類を自己裁定しない。非意味的 verdict のみ修正を保持して kind: non-semantic-fix で刻印する。意味的・不確実 verdict は編集前文面へ復元し、その内容を delta 候補として起草し直す。
- **delta 入庫判定 (三択)**: 候補の起草・改稿ごとに adr-diagnoser が判定する。(a) 記録済み決定の実効内容を変えない → 入庫。(b) 決定保存的な解決 — 決定を保つ代替文面 / 下流 (spec 等) での解決 / 発生元 input 自体が ADR 変更を要しないとする理由付き返却 — が存在する → その解決を添えて起点へ差し戻す (入庫・刻印なし)。(c) いずれの保存的解決も存在せず決定修正が不可欠 → 修正対象を明示した決定修正提案として入庫し、user の非同期裁定を待つ。判定に迷う場合は差し戻す (fail-closed)。既存決定を変更する候補は supersedes / refines の関係と対象を起草時に宣言し、対象は relation chain の現 head (最新の採用済み修正、なければ元の決定) に限る。**入庫は判定済み文面に束縛される** — 一 byte でも改稿すれば失効し、再判定なしに再入庫できない。
- **入庫済み draft**: 下流成果物は user の裁定を待つ間も cite して作業を進められる。draft が正式な決定になるのは user の明示的採用時のみで、採用まで strict merge gate が merge を止める。入庫前の候補への cite は禁止。
- **不意の変更** (経路不明の乖離): byte 照合 (下記機構整合) で検出し、adr-diagnoser がトリアージする。意味を変えない差分のみ non-semantic-fix で再刻印し、それ以外は restore する。判断に迷えば逸脱側に倒す (fail-closed)。

### merge 段階 — 採用・棄却・監査

- **採用**: user の明示的採用のみが draft を正式決定化する。orchestrator は adr-editor に根拠の `user_decision_ref` への昇格を指示する (改稿につき入庫は失効し、再判定・再入庫を経る)。その後 adr-diagnoser が採択差分を conformance 再監査する — 基準は「採択内容の忠実な実装か」であり決定保存判定ではない。adoption-conformant の場合のみ orchestrator が kind: new-adr で刻印する。reason には起点来歴 (local review round / 外部 PR review round / spec→ADR の 🔴 signal / diagnose routing の別) と入庫判定・再監査の要旨を記す。supersede / refine の関係は採用された delta 側にのみ記録し、入力箱 ADR の decision status・`superseded_by` は書き換えない。採用時に relation を新設・変更しない (変更は改稿 = 再入庫)。決定の現在の内容は、元の決定に採用済みの修正を採用順に重ねて読むことで得る (supersedes は chain prefix の全置換、refines は delta 優先の合成)。
- **棄却**: 昇格・刻印は行わない。adr-editor が draft を削除するか user 指示どおり改稿し、adr-diagnoser が rejection-conformance を再監査する。削除が conformant なら確定、改稿が conformant なら候補として再判定へ。いずれの分岐でも、draft を cite または導出した下流成果物を SoT Chain 順に merge 前へ再作業する — 削除時は Chain ① の 🔴 を gate が機械的に強制し、改稿時は再検証完了まで merge を承認しない。
- **terminal 監査**: merge 前に、全 protected source (ledger 記録を持つすべての source) について、保護開始記録 (primary は init / 後続保護は cite / 採用済み delta は new-adr) から terminal までの diff と provenance を user に提示する。各 non-semantic-fix 記録には直前記録との隣接 diff も提示する — terminal diff が空でも中間の誤刻印はここに現れる。記録の意味裁定は user に属する (守護者は事前注記できる)。誤分類と裁定された記録は corrective 復元で回復する: adr-editor が直前の有効記録の文面へ復元し、守護者が byte 一致を確認した後、reason なしの non-semantic-fix で再刻印する。誤記録は履歴と provenance に残り隠蔽されない。復元後になお意味の解決が必要なら、該当手順 (Phase 0 相当は user 同席、Phase 1+ は delta lane) へ戻す。

### 機構との整合

本節が規範の正であり、機構は以下の形で追随する:

- `adr-baseline check-review` (review 入口) — init 記録の存在確認と台帳健全性 (保存された複製の実在と記録 hash との一致) のみ。現在の ADR 文面との byte 照合は行わない。
- byte 照合 — commit gate と track-aware CI (`cargo make ci-track` と PR CI の同等 check) でのみ発火する。比較基準は当該 source の ledger 最新記録であり、kind を問わない (init / cite / new-adr / non-semantic-fix / review-refinement、および旧 workflow の historical escalation)。
- escalation kind — 新規刻印用途から引退する。唯一の例外は Phase 0 境界刻印の経過措置であり、review-refinement kind の実装後に失効する。escalation の意味 (下流失敗の遡上) は new-adr 刻印 reason 内の起点来歴として保持される。
- 目標仕様と経過措置 — review-refinement kind、admission marker (`admission_class`)、`supersedes` / `refines` の front-matter field は文書上の目標仕様であり、Rust 実装 (enum・schema・validator) は後続 track が行う。実装までは: 境界刻印は escalation kind で代用し、入庫判定の verdict は dispatch / レビュー記録で追跡し、relation の宣言は draft 本文 (Decision / Related) で行う — front-matter へ未実装 field を書いてはならない (parser が未知 field を拒否する)。

## Examples

- Good: user が `knowledge/adr/<date>-<slug>.md` を作成 → `/track:plan "feature X"` を invoke → `/track:plan` が ADR 存在を確認して Phase 0 (init) に進む。
- Good: Phase 0 の baseline review が意味変更を含む修正で zero_findings に収束 → user が init 刻印との diff を裁定して承認 → 承認 ref 適用・再監査・fresh review 再収束 → review-refinement 刻印 (経過措置: escalation kind 代用) → ADR-baseline commit で境界を閉じる。
- Good: Phase 1 (spec) で signal 🔴 が ADR 側の意味変更を要求 → adr-editor が delta 候補を起草 → 入庫判定 (c) で決定修正提案として入庫 → spec が入庫済み draft を cite して 🔴 解消 → 🟡 のまま merge 段階で user が採用/棄却。
- Good: 入庫判定が (b) — 決定を保つ代替文面が存在 → 候補を削除して代案を起点へ返す → 起点が代案で解決。
- Bad: `/track:plan "feature X"` を実行、内部で ADR を自動生成 (spec に合わせて decision を後付けする rationalization の温床になる)。
- Bad: main が `Edit` tool で `knowledge/adr/xxx.md` を直接書き換える (1 ファイル 1 writer 原則違反)。
- Bad: Phase 0 の review ループ中、編集のたびに刻印を打つ (承認前の中間刻印は、user 裁定の diff 基準を自己洗浄する行為)。
- Bad: 裁定境界後に入力箱 ADR を in-place で意味編集する (freeze 違反。delta 候補として起草すべきもの)。
- Bad: 入庫前の候補を spec が cite する (未判定の決定に下流を接続する行為)。
- Bad: 入庫判定や意味/非意味の分類を orchestrator が自己裁定する (配達人は意味を裁定しない)。
- Bad: user の採用なしに draft の根拠を `user_decision_ref` へ昇格・刻印する (裁定点の無断通過)。

## Exceptions

- **既存 ADR 流用**: 本機能の設計判断が既存 ADR でカバーされているなら新規作成は不要。
- **緩和モードなし**: 「ADR 未整備でも stub で進行を許す」モードは意図的にサポートしない。必要性が実証された時点で別 ADR で検討する。

## Review Checklist

- [ ] track 内成果物 (`track/items/<id>/` 配下) に ADR を配置していないか
- [ ] ADR ファイルに `Status` / `approved` 等の状態フィールドを追加していないか
- [ ] `/track:plan` 起動前に ADR が `knowledge/adr/` に存在するか (厳密モード条件を満たすか)
- [ ] track 内の ADR 修正で main が直接編集せず adr-editor を経由しているか
- [ ] Phase 0 の baseline review ループ中に ledger へ書き込んでいないか (中間刻印禁止)
- [ ] 裁定境界後に入力箱 ADR を in-place で意味編集していないか (意味変更は delta 候補へ)
- [ ] delta 候補の入庫判定 (三択) と、意味/非意味の分類を adr-diagnoser に委ねているか (orchestrator の自己裁定なし)
- [ ] 入庫前の候補を下流成果物が cite していないか
- [ ] user 裁定 (境界承認・採用・棄却) を経ずに刻印・昇格・commit へ進んでいないか
- [ ] ADR からの参照が track 内成果物を指していないか (SoT Chain 逆流禁止)
- [ ] 未実装の front-matter field (`admission_class` / `supersedes` / `refines`) を書き込んでいないか (目標仕様 — 実装は後続 track)
- [ ] `.claude/commands/**/*.md` / `.claude/skills/**/SKILL.md` の本文が特定 ADR を日付付きスラグで cite していないか (commands / skills は自己完結)

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/adr.md](./adr.md) — ADR 運用の基本ルール
- [knowledge/conventions/workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 事後レビュー方式 / 事前承認限定の原則
