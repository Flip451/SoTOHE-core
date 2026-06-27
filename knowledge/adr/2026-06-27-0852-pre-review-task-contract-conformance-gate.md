---
adr_id: 2026-06-27-0852-pre-review-task-contract-conformance-gate
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-26"
    candidate_selection: "from:[type-catalogue-phase2,task-coverage-colocate,new-phase3-artifact] chose:new-phase3-artifact"
    status: accepted
  - id: D2
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-26"
    candidate_selection: "from:[status-quo-no-pre-review-gate,commit-gate-only,pre-review-blocking-binary-check] chose:pre-review-blocking-binary-check"
    status: accepted
  - id: D3
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-26"
    candidate_selection: "from:[blue-only,blue-plus-test-pass,blue-plus-stub-scan] chose:blue-only"
    status: accepted
  - id: D4
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-26"
    candidate_selection: "from:[extend-impl_catalog-chain,new-task_conformance-chain,binary-check-reuse-signals] chose:binary-check-reuse-signals"
    status: accepted
  - id: D5
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-26"
    status: accepted
---
# タスク単位の契約履行 pre-review ゲート — Phase 3 attribution artifact と impl_catalog 信号の binary 再利用

## Context

型カタログ（型契約）に対してコードが実際に履行しているか——構造的に契約と一致しているか——の **blocking な確認が、現状では merge 段階まで先送りされている**。本 ADR はこの確認を **shift-left**（reviewer 入場前・per-task へ前倒し）したい。

実コードで現状の機構を確認した結果：

- **契約一致を測る信号は chain ③ `impl_catalog`**（型カタログ ↔ rustdoc API の構造一致。`libs/domain/src/chain.rs:13`）。評価器 `SignalEvaluatorV2` は現行コード側を `rustdoc_types::Crate`＝ rustdoc JSON として受け取り、shape（field 型 / variant 形 / 関数シグネチャ / generics）を比較する（`libs/infrastructure/src/tddd/signal_evaluator_v2/mod.rs:53`, `.../signal_evaluator_v2/structural_eq.rs:22-24`）。
- **この信号が hard に block する最初の地点は merge-gate（strict）.** commit-gate は **interim**（Yellow は warn のみで通過可）、merge-gate は **strict**（Yellow で block）（`libs/infrastructure/src/verify/signal_gates_config.rs` 既定, `libs/usecase/src/merge_gate.rs:479`）。reviewer 起動前に前置される `calc-impl-catalog && … && review local` は signal hash / view を最新化する **freshness 再計算**であって、🔵 を入場条件として assert しない（`libs/infrastructure/src/review_v2/review_fix_runner/prompt.rs:57-67`）。しかも workflow 上 review は commit の前（`.claude/rules/10-guardrails.md`: reviewer が zero findings になるまで commit しない）。
- **帰結（先送りの実害）**: 型契約を構造的に履行していないコード——宣言シンボルの欠落、shape ずれ——が、track の **最終盤（merge 直前）まで hard には止まらない**。authoring 直後に per-task で気づけるはずの構造的な契約違反が、最も手戻りコストの高い段階まで遅延する。これが shift-left したい対象である。

shift-left の前提と不足ピース：

- **信号機が判定するのは「構造的な型契約の履行」だけで、body の意味論検証は LLM レビューが担う——これは制約ではなく意図した責任分界点である.** SoT chain ③ の信号機（型カタログ ↔ 実装）は宣言シンボルの存在と shape（signature / field / variant）の一致のみを判定対象とし、body の意味論的正しさは見ない（前 bullet の通り、入力が rustdoc JSON ＝ body を含まない）。「**構造的整合＝機械的信号 / 意味論的整合＝ LLM レビュー**」という層分けはプロジェクトが既に採る分界であり（`2026-05-27-1601-sot-chain-semantic-review-gate` が presence/構造 信号と意味論検証を別レーンに分離する3層モデルを定義。同 ADR の対象は chain①②で、**chain ③ の body 意味論は実装がコードそのものゆえ通常の `/track:review` が担う**）、本 ADR が shift-left するのは分界の **構造側（信号機）だけ**で、意味論側（reviewer）は不変のまま残す。帰結として signature だけ揃った stub は構造的には契約一致＝ 🔵 になり（body の liveness は信号機の管轄外）、その検証は分界の向こう＝ reviewer の責務に残る。この責任分界点が D3（判定は 🔵 のみ）と D5（briefing は「shape 一致・body 未検証」と明記）の根拠になる。
- **不足は task→entry の辺だけ.** 信号は layer doc 内で type/method 単位（per-entry）に 🔵🟡🔴 を持つ（`libs/domain/src/tddd/type_signals_doc.rs:30`）一方、計算は per-layer 反復で **per-task の紐付けが無い**（`apps/cli-composition/src/signal.rs:386-398`）。前倒しを per-task で行うのに足りないのは「どのタスクがどの entry を履行するか」のマッピングであり、この辺は gate 対象 scope 内の関連 entry を漏れなく覆う complete relation として扱う。
- **置き場所は専用の新 Phase 3 artifact.** この task→entry マッピングは `task-coverage.json`（責務＝ spec coverage）には混ぜず、独立した Phase 3 artifact として持つ（責務分離。詳細は D1）。writer は impl-planner: 既に spec / カタログを読み task を author しているため、新 artifact の author も自然。

## Decision

### D1: task→型契約 entry のマッピングを専用の新 Phase 3 artifact（`task-contract.json`）に持つ

「どのタスクがどの型契約 entry を履行するか」のマッピングを、Phase 3 の **新規 artifact `task-contract.json`**（作業名）に持つ。**writer は impl-planner（Phase 3）**。型カタログ（Phase 2 / type-designer）にも `task-coverage.json` にも混ぜない。

`task-contract.json` は一方向の任意メモではなく、gate 対象 scope に対する **complete attribution relation** として扱う。invariant は (a) scope 内でこの review / task 群が履行責務を持つ catalogue entry が少なくとも1 task に attribution されること、(b) attribution された entry がカタログに実在し当該 scope 内であること、(c) scope 内の entry に orphan（どの task にも attribution されない関連 entry）が残らないこと。orphan entry を許すと、その entry の `impl_catalog` が 🟡/🔴 でも pre-review gate の入力から漏れ、merge-gate まで先送りされるため、shift-left の保証が崩れる。

型カタログに載せない理由: (a) task は Phase 3 で初めて生まれる概念であり、Phase 2 成果物に task→entry を載せると Phase 3 概念への**後方依存**（SoT Chain 逆流・順序違反）になる。(b) 型カタログは肥大化しがちで、task 知識を背負わせると type-designer の責務が膨張する。

`task-coverage.json` に混ぜない理由（責務分離）: `task-coverage.json` の責務は **spec coverage**（Phase1↔Phase3 の完全性: 全 spec 要素が ≥1 タスクで覆われているか）。本マッピングの責務は **型契約の履行 attribution**（Phase2↔Phase3: 各タスクが履行する catalogue entry を pre-review ゲートへ供給する）。両者は (i) 参照する上流 SSoT が違い（spec.json vs `<layer>-types.json`）、(ii) 供給先ゲートが違い（coverage 完全性 vs pre-review conformance）、(iii) invariant が違う（spec 要素への全射 vs 型契約 entry の attribution completeness + referential integrity）。同一ファイルに同居させると「変更理由」と gate evaluator が二重化する（artifact 単位の SRP 違反）。両者は別々の単一責務 artifact とし、ともに impl-planner が author する（1 writer が複数の単一責務 artifact を持つのは type-designer の per-layer カタログと同じ）。

### D2: pre-review の blocking 入場ゲートを binary check として新設する

reviewer 起動の前に、「**gate 対象 scope の `task-contract.json` が complete であり、そこに含まれる全関連 entry の `impl_catalog` 信号が 🔵 か**」を判定する binary check を置く。complete でない、または 🔵 でない entry があれば review に入れない（fail-closed・前倒し）。これは既存の freshness prepend（再計算）とは別の、**入場条件としての blocking 判定**である。SoT Chain（🔵🟡🔴）ではなく binary check として実装する（`knowledge/conventions/workflow-ceremony-minimization.md` の「SoT Chain 信号 + binary check」枠組みに乗る）。これにより、merge まで先送りされていた契約一致確認の最初の hard gate が reviewer 入場前・per-task に移る。

通過時には verified-conformance サマリを生成し、reviewer briefing に流す（D5 の文言規律に従う）。

### D3: 「履行を試みている」の判定は 🔵 のみとする（body は検証しない）

ゲートの合否基準は「scope 内の関連 entry が `task-contract.json` で完全に attribution され、その全 entry の信号が 🔵」だけとする。test-pass や stub-scan による body-aware な liveness 判定は**採らない**。

これは意図的なスコープ確定であり、ゲートの保証は「**宣言した API surface が存在し shape が一致**」までで、「body が `todo!()` でない」ことは保証しない。本命の取りこぼし（構造不一致 / シンボル欠落 / shape ずれ）は 🔵 が確実に弾く。すり抜ける「正しい signature の stub」という狭いケースは下流（body を読む reviewer が trivial finding として即却下、加えて commit-gate / merge-gate の test）が安く backstop する。

### D4: 信号値は既存 impl_catalog chain を再利用し、新 chain も型カタログ拡張も行わない

ゲートは、`task-contract.json` の complete task→entry relation と、既存 `impl_catalog` の per-entry 信号値を **entry をキーに突き合わせて**読むだけ（scope 内の関連 entry が漏れなく attribution され、その全 entry の信号が 🔵 か）。新 chain（`task_conformance` 等）を切らず、型カタログにも何も足さない。

理由: 新 chain は gate matrix / strictness / views の表面積を増やす。per-task 判定に必要なのは既存信号 + Phase 3 マッピングの JOIN で足り、binary check で実装できる。検証エンジン（`SignalEvaluatorV2`）は丸ごと再利用する。

唯一必要な追加検証は **attribution completeness + referential integrity**（scope 内の関連 entry が orphan なく task に attribution され、attribution された entry がカタログに実在し、当該 scope 内であること）。ただしこれは `task-coverage.json` が今 spec に対して負っている coverage / drift と同種で、新しいリスククラスではない（`/track:plan` の back-and-forth + binary gate の既存機構で吸収）。

### D5: reviewer briefing の verified-conformance 行は「shape 一致（body 未検証）」と正確に書く

ゲート通過を reviewer に伝える際の文言は「**declare した API surface が型契約と shape 一致（body は未検証 — stub / liveness は reviewer が確認せよ）**」とする。「契約 satisfied」「実装済み」のように書いてはならない。

理由: lossy な要約を下流が over-trust すると、stub の精査が落ちて D3 で受容したすり抜けが「捕まらないすり抜け」に悪化する。ゲートの保証範囲を文言で正確に伝えることで、安価さ（D3）と健全さ（reviewer が liveness を担保）を両立する。

## Rejected Alternatives

- **A. 「履行」判定に test-pass / stub-scan（body-aware liveness）を含める**: 却下。body は rustdoc JSON に載らず現エンジンでは原理的に見られないため、別データ源（syn 走査 / nextest 名前フィルタ）と新検査系の新設・保守が要る。すり抜ける stub は reviewer（body を読む）と commit/merge gate の test が安く捕まえるので、その狭い残コストのために検査系を抱えるのは ROI が悪い。
- **B. task→entry を型カタログ（Phase 2）に載せる**: 却下。type-designer の責務が膨張し、Phase 2 成果物が Phase 3 概念（task）へ後方依存する（SoT Chain 逆流・設計順序違反）。
- **C. task→entry を既存 `task-coverage.json` に同居させる**: 却下。spec coverage（完全性）と型契約 attribution（conformance ゲート入力）という別責務が混在し、別 SSoT（spec vs カタログ）・別ゲート・別 invariant を1ファイル / 1 evaluator が抱える（artifact 単位の SRP 違反）。ファイル増を避ける安価さより責務分離を優先する。
- **D. 新 chain（task_conformance）を切る**: 却下。gate matrix / strictness / views の表面積が増える。per-task は既存 `impl_catalog` 信号 + Phase 3 マッピングの JOIN で足り、binary check で実装可能。
- **E. shift-left せず現状維持（pre-review prepend は freshness のみ、blocking は commit=interim / merge=strict）**: 却下。契約一致の hard gate が merge までかからず、違反が track 最終盤まで遅延する。早期・per-task の検出という本 ADR の目的を満たさない。
- **F. reviewer に prose の「ソースコード理解キャッシュ」を渡して前倒しの代替とする**: 却下。review は adversarial であり、lossy な散文サマリは「キャッシュを信じて見落とす」穴を作る。安全なのは契約 SSoT（型カタログ）と信号状態を渡すことであり、本 ADR の (D2)+(D5) はその安全形に限定する。

## Consequences

### Positive

- 契約違反（宣言シンボル欠落 / shape ずれ）が **authoring 直後・per-task で表面化**し、merge 直前での発覚と track 最終盤の手戻りを避けられる（shift-left の主目的）。
- reviewer が「カタログと形すら合わないコード」に消費されるのを入場前に防げる。
- 既存 `impl_catalog` 信号を再利用し、型カタログ / type-designer は不変、新 chain も不要。変更面は (D1) 新 Phase 3 artifact `task-contract.json` と (D2) binary check の新設に限られる（検証エンジンは再利用）。
- task→entry マッピングは副次的に review の split-key（cohesion 境界での分割）としても再利用できる。
- ゲートは deterministic / binary で、SoT-chain-binary-check の枠組みに乗る。
- verified-conformance を briefing に正確に渡すことで、reviewer の予算を behavior / 非局所検査に集中できる。

### Negative

- 🔵 は body を見ないため、正しい signature の stub（`todo!()`）はゲートを通る（liveness 未保証）。下流（body-aware reviewer / commit・merge gate test）が backstop する前提に依存する。
- 新 Phase 3 artifact `task-contract.json` 1つ分の surface が増える（schema / codec / gate 配線、および attribution completeness / referential integrity の維持＝ entry の rename / 削除で orphan / drift し得る）。ただし drift は既存の task↔spec と同種で新しいリスククラスではない。
- impl-planner の責務がわずかに増える（新 artifact `task-contract.json` の author を追加）。
- ゲートが blocking なので、🔵 になっていない段階の探索的 / 部分実装に対して review を妨げ得る（interim 的運用や per-task scope での緩和は将来検討）。

### Neutral

- feature バッチ消化（`2026-06-22-1327-feature-batch-default-inversion`）とは関心が異なり直交する: 前者は review round の経済、本 ADR は契約検証の**位置**（shift-left）。併用時は、バッチ実装の各タスクが reviewer 入場時点で構造的に契約一致であることを保証できる。
- DRY ゲートとは直交する。

## Reassess When

- 「正しい signature の stub」のすり抜けが実測で頻発し、後段（reviewer / test）での発覚が無視できなくなる（body-aware liveness 判定の再検討）。
- `task-contract.json` の task→entry 維持コストが高い / drift が頻発する（symbol-join による導出自動化の検討）。
- 1タスクが多数 entry を抱え、per-task gating の粒度が粗すぎると判明する。
- `impl_catalog` 信号の粒度 / 意味論が変わり、per-entry 前提が崩れる。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md` — 構造的 signal と意味論検証（LLM レビュー）を別レーンに分離する3層モデル（D1）。本 ADR の責任分界点の典拠。ただし同 ADR の対象は chain①②（spec↔ADR / catalogue↔spec）で、chain ③（catalogue↔impl）は対象外——本 ADR はその chain ③ の構造側を shift-left する
- `knowledge/adr/2026-06-22-1327-feature-batch-default-inversion.md` — track 実装フェーズの関連 ADR（review round の経済；本 ADR の契約検証 shift-left とは関心が直交）
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR ライフサイクルと配置規約
- `knowledge/conventions/workflow-ceremony-minimization.md` — SoT Chain 信号 + binary check の枠組み
- `.claude/rules/10-guardrails.md` — review は commit の前という順序規約（shift-left の現状基準点）
- `.harness/config/agent-profiles.json` — impl-planner / reviewer capability
- 検証根拠（実コード）: `libs/infrastructure/src/tddd/signal_evaluator_v2/mod.rs`, `.../signal_evaluator_v2/structural_eq.rs`, `libs/domain/src/chain.rs`, `libs/infrastructure/src/review_v2/review_fix_runner/prompt.rs`, `libs/infrastructure/src/verify/signal_gates_config.rs`, `libs/usecase/src/merge_gate.rs`, `apps/cli-composition/src/signal.rs`, `libs/domain/src/tddd/type_signals_doc.rs`
