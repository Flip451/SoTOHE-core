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
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-28"
    candidate_selection: "from:[combined-single-command,coverage-check-split-task-contract-domain,coverage-check-split-verify-domain,attribution-liveness-rename] chose:coverage-check-split-task-contract-domain"
    status: accepted
  - id: D6
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-28"
    candidate_selection: "from:[wire-via-bin-sotp-hardcode,wire-via-cargo-make-dependencies,no-wiring-document-only] chose:wire-via-cargo-make-dependencies"
    status: accepted
  - id: D7
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-28"
    candidate_selection: "from:[impl-plan-status,task-contract-extension-with-status,external-state-file,git-blame-derivation] chose:impl-plan-status"
    status: accepted
  - id: D8
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-29"
    candidate_selection: "from:[stdout-string-matching,magic-exit-codes,typed-error-variant-pass-through] chose:typed-error-variant-pass-through"
    status: accepted
  - id: D9
    user_decision_ref: "chat_segment:session_013pQgnVvMR775h8tsD1mya4:2026-06-29"
    candidate_selection: "from:[coverage-external-verify-subcommand,silently-ignore-stale-task-keys,coverage-integrated-ri-check] chose:coverage-integrated-ri-check"
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

- **信号機が判定するのは「構造的な型契約の履行」だけで、body の意味論検証は LLM レビューが担う——これは制約ではなく意図した責任分界点である.** SoT chain ③ の信号機（型カタログ ↔ 実装）は宣言シンボルの存在と shape（signature / field / variant）の一致のみを判定対象とし、body の意味論的正しさは見ない（前 bullet の通り、入力が rustdoc JSON ＝ body を含まない）。「**構造的整合＝機械的信号 / 意味論的整合＝ LLM レビュー**」という層分けはプロジェクトが既に採る分界であり（`2026-05-27-1601-sot-chain-semantic-review-gate` が presence/構造 信号と意味論検証を別レーンに分離する3層モデルを定義。同 ADR の対象は chain①②で、**chain ③ の body 意味論は実装がコードそのものゆえ通常の `/track:review` が担う**）、本 ADR が shift-left するのは分界の **構造側（信号機）だけ**で、意味論側（reviewer）は不変のまま残す。帰結として signature だけ揃った stub は構造的には契約一致＝ 🔵 になり（body の liveness は信号機の管轄外）、その検証は分界の向こう＝ reviewer の責務に残る。この責任分界点が D3（判定は 🔵 のみ）の根拠になる。
- **不足は task→entry の辺だけ.** 信号は layer doc 内で type/method 単位（per-entry）に 🔵🟡🔴 を持つ（`libs/domain/src/tddd/type_signals_doc.rs:30`）一方、計算は per-layer 反復で **per-task の紐付けが無い**（`apps/cli-composition/src/signal.rs:386-398`）。前倒しを per-task で行うのに足りないのは「どのタスクがどの entry を履行するか」のマッピングであり、この辺は gate 対象 scope 内の関連 entry を漏れなく覆う complete relation として扱う。
- **置き場所は専用の新 Phase 3 artifact.** この task→entry マッピングは `task-coverage.json`（責務＝ spec coverage）には混ぜず、独立した Phase 3 artifact として持つ（責務分離。詳細は D1）。writer は impl-planner: 既に spec / カタログを読み task を author しているため、新 artifact の author も自然。

## Decision

### D1: task→型契約 entry のマッピングを専用の新 Phase 3 artifact（`task-contract.json`）に持つ

「どのタスクがどの型契約 entry を履行するか」のマッピングを、Phase 3 の **新規 artifact `task-contract.json`**（作業名）に持つ。**writer は impl-planner（Phase 3）**。型カタログ（Phase 2 / type-designer）にも `task-coverage.json` にも混ぜない。

`task-contract.json` は「pre-review ゲートに見せたい信号を漏れなくタスクに紐付けたか」だけを言う契約。1 件でも未紐付けのエントリが残ると、ゲートはそのエントリの信号 (🟡/🔴) を見ないまま素通りさせるので、結局 merge まで止まらない＝ shift-left の意味がなくなる。

型カタログに載せない理由: (a) task は Phase 3 で初めて生まれる概念であり、Phase 2 成果物に task→entry を載せると Phase 3 概念への**後方依存**（SoT Chain 逆流・順序違反）になる。(b) 型カタログは肥大化しがちで、task 知識を背負わせると type-designer の責務が膨張する。

`task-coverage.json` に混ぜない理由 (責務分離): `task-coverage.json` の責務は**仕様被覆** (Phase1↔Phase3 の完全性: 全仕様要素が ≥1 タスクで覆われているか)。本マッピングの責務は**型契約の履行帰属** (Phase2↔Phase3: 各タスクが履行するカタログエントリを pre-review ゲートへ供給する)。両者は (i) 参照する上流 SSoT が違い (`spec.json` vs `<layer>-types.json`)、(ii) 供給先ゲートが違い (被覆完全性 vs pre-review 履行性)、(iii) 不変条件が違う (仕様要素への全射 vs 型契約エントリの帰属完備性 + 参照整合性)。同一ファイルに同居させると「変更理由」とゲート評価器が二重化する (artifact 単位の SRP 違反)。両者は別々の単一責務 artifact とし、いずれも impl-planner が著作する (1 writer が複数の単一責務 artifact を持つのはレイヤー別カタログの type-designer と同じ)。

### D2: pre-review の blocking 入場ゲートを binary check として新設する

reviewer 起動の前に、「`task-contract.json` で**現在のタスクと完了済タスクに帰属するエントリの `impl_catalog` 信号が全て 🔵 か**」を判定する binary check (`bin/sotp task-contract check`) を置く。🔵 でないエントリがあれば review に入れない (fail-closed・前倒し)。これは既存の freshness prepend (再計算) とは別の、**入場条件としての blocking 判定**である。SoT Chain (🔵🟡🔴) ではなく binary check として実装する (`knowledge/conventions/workflow-ceremony-minimization.md` の「SoT Chain 信号 + binary check」枠組みに乗る)。これにより、merge まで先送りされていた契約履行確認の最初の hard gate が reviewer 入場前・per-task に移る。

なお attribution 完全性 (`task-contract.json` が型カタログの全エントリをタスクに帰属させているか) は別責務として `bin/sotp task-contract coverage` で扱う (D5 参照)。

### D3: 「履行を試みている」の判定は 🔵 のみとする（body は検証しない）

ゲートの合否基準は「`task-contract.json` が型カタログの全エントリをいずれかのタスクに帰属させており (attribution 完全性)、かつ現在のタスクと完了済タスクに帰属するエントリの信号が全て 🔵 (生存性判定)」だけとする。test-pass (テスト通過) や stub-scan (スタブ走査) による本体認識型 (body-aware) の生存性判定は**採らない**。

これは意図的な責務分離である。ゲートは**型契約履行のカバレッジ**（構造的整合 — 宣言した API surface が型カタログと shape 一致しているか）のみを保証し、body の意味論的妥当性（実装が意図通りか、`todo!()` で済まされていないか、等）はゲートの保証範囲外として LLM レビューに残置する。Context 節で確立した「構造 = 機械的信号 / 意味論 = LLM レビュー」の責任分界点を、本 ADR が導入するゲートも踏襲している。

### D4: 信号値は既存 impl_catalog chain を再利用し、新 chain も型カタログ拡張も行わない

ゲートは、`task-contract.json` のタスク→エントリ完全帰属関係と、既存 `impl_catalog` のエントリ毎の信号値を**エントリをキーに突き合わせて**読むだけ (具体的な判定基準は D3 参照)。新 chain (`task_conformance` 等) を切らず、型カタログにも何も足さない。

理由: 新 chain はゲート構成 (ゲート行列 / 厳格度 / view) の表面積を増やす。タスク毎の判定に必要なのは既存信号と Phase 3 帰属マッピングの JOIN で足り、binary check で実装できる。検証エンジン (`SignalEvaluatorV2`) は丸ごと再利用する。

唯一必要な追加検証は**帰属完備性 + 参照整合性** (型カタログの全エントリが漏れなくタスクに帰属され、帰属されたエントリがカタログに実在すること)。これは `task-coverage.json` が今 spec に対して負っている被覆 / drift と同種で、新しいリスククラスではない (`/track:plan` の back-and-forth + binary gate の既存機構で吸収)。

### D5: attribution 完全性 (coverage) と生存性 (check) を別 subcommand に分割する

`task-contract.json` に対する検証は (a) 型カタログの全エントリが漏れなくタスクに帰属されているか (**attribution 完全性** / drift 検出) と (b) 現在のタスクと完了済タスクに帰属するエントリの `impl_catalog` 信号が全て 🔵 か (**生存性** / progression gate) の 2 責務を持つ。両者は失敗時の責任者 (planner が attribution を author する vs implementer が impl を 🔵 化する) と修正経路 (`task-contract.json` 再 author vs impl 修正) が異なり、混在させると fixer の判断分岐コストが恒久化する。

そこで 2 subcommand に分割する:

- `bin/sotp task-contract coverage` — attribution 完全性のみ。`cargo make ci` の検証 chain に統合し、commit ごとに drift を検出する。
- `bin/sotp task-contract check` — 生存性のみ。pre-review blocking gate として `cargo make track-local-review` chain に置く (D2)。

両者は **`task-contract` ドメイン配下** に置く。一般 verifier ファミリー (`verify-*`) は既に肥大化しており、domain ownership を希釈するため `verify-task-contract-coverage` のような形には逃さない。

### D6: 生存性判定 task に完全性判定 task を cargo-make `dependencies` で連結する

D5 で 2 subcommand に分割した `task-contract coverage` (完全性) と `task-contract check` (生存性) は、コマンドとしては独立だが**実行順は固定**: 生存性判定は完全性判定が通った状態でのみ意味を持つ (orphan エントリが存在すると、その signal が 🟡/🔴 でもゲート入力から漏れ、生存性判定は「自分が知っているエントリだけ全部 🔵」という誤った安心を返してしまう)。

そこで cargo make 上で 2 task を定義し、**生存性判定 task の `dependencies` に完全性判定 task を組み込む**:

- `cargo make task-contract-coverage` — `bin/sotp task-contract coverage` を呼ぶ
- `cargo make task-contract-check` — `bin/sotp task-contract check` を呼び、`dependencies = ["task-contract-coverage"]` を宣言

これにより、`cargo make task-contract-check` を呼ぶだけで完全性判定 → 生存性判定の順で必ず走り、両者が同期する。`bin/sotp` バイナリ内部で連結する案 (バイナリ内 hardcode) は採らない — 配線は cargo make 層に局所化し、各 `bin/sotp` subcommand は単一責務のままに保つ (`knowledge/conventions/` の cargo-make 配線方針)。

`cargo make track-local-review` と `cargo make track-local-review-fix` の両方の `dependencies` に `task-contract-check` を追加する。これにより per-review-round で完全性 + 生存性の両判定が自動で発火する。

### D7: 「現在のタスク」「完了済タスク」の識別は `impl-plan.json` のタスク状態を参照する

D3 P1 で定義した生存性判定 (「現在のタスクと完了済タスクに帰属するエントリの信号が全て 🔵」) は、識別ソースを必要とする — どのタスクが「現在 (in_progress)」で、どのタスクが「完了済 (done)」で、どのタスクが「未着手 (todo)」か。これは Phase 3 で impl-planner が author する `impl-plan.json` がタスク状態 (status: todo / in_progress / done) として既に保持している情報なので、ゲートはここを SSoT として読む。

これにより:

- **done タスク**に帰属するエントリ → signal 🔵 を要求
- **in_progress タスク** (= 現在のタスク) に帰属するエントリ → signal 🔵 を要求 (生存性判定が走るのは review 前なので、当該タスクの impl はそこまでで完了している前提)
- **todo タスク** (= 未来のタスク) に帰属するエントリ → 🟡 のみ許容 (まだ実装していないので shape mismatch 等は当然発生する)。🔴 は task status に関わらず常に blocker

実装上は、`bin/sotp task-contract check` の usecase 層 (`PreReviewGateInteractor`) に **impl-plan を読む secondary port** (`ImplPlanReaderPort` 等) を追加し、`task-contract.json` から得た task → entry 帰属を impl-plan.json の status フィルタで絞り込む。

`task-contract.json` 自身に status を載せる代替案は採らない — 状態 (動的・進行に応じて変化) と帰属 (静的・planner が一度 author) を 1 ファイルに混在させると SRP 違反になる。impl-plan が SSoT として既に状態を持っているので、そこを単に参照する。

### D8: Claude provider 経由の review-fix dispatch は usecase boundary の typed error として cli_composition から cli_driver へ pass-through する

D2/D6 で新しい task-contract gate の配線対象に含める既存の `bin/sotp review fix-local` (review-fix-lead 用 wrapper) は、`agent-profiles.json` の `review-fix-lead.provider` を見て codex/claude を内部で切り替える。**Claude provider 経由のとき**、CLI は subagent を直接 spawn できず、stdout に `SUBAGENT_DISPATCH_REQUIRED` sentinel + JSON payload を出力し exit code 64 で終了することで、orchestrator (Claude Code) に subagent 起動を delegation する仕組みを採る。

しかしこの dispatch path が `ReviewServiceImpl::run_fix_local` (cli_composition 層で usecase 層 `ReviewService` boundary を実装する shim) を経由したとき、`ReviewServiceImpl` が exit 64 を **「failed」と remap し stdout を REVIEW_FIX_STATUS: failed に書き換えてしまう** 不具合が発覚した。orchestrator は sentinel を受け取れず、Claude provider routing が壊れていた。

そこで以下のとおり修正する:

- usecase 層 `RunReviewFixError` enum に新 variant `SubagentDispatchRequired(String)` を追加。tuple field は exit 64 + sentinel + JSON payload を1つの opaque string として carry する。
- `ReviewServiceImpl::run_fix_local` (cli_composition 層 shim) は composition root から exit 64 + SUBAGENT_DISPATCH_REQUIRED prefix を検出したとき、`Err(RunReviewFixError::SubagentDispatchRequired(payload))` を return する。
- cli_driver 層 `review_run_fix_local` は `Err(RunReviewFixError::SubagentDispatchRequired(payload))` 受領時、`CommandOutcome { stdout: Some(payload), stderr: None, exit_code: 64 }` を生成して呼び元に pass-through する。これにより stdout の sentinel + JSON payload + exit 64 が orchestrator に届く。

代替案 (却下):

- **magic exit codes 全層 propagation**: cli_driver が exit code 64 を string match して特殊扱いする案。usecase 層が exit code を直接扱うのは layering 違反。
- **stdout 文字列 sniffing**: stdout から `SUBAGENT_DISPATCH_REQUIRED` prefix を全層で string match する案。typed error より fragile で、テストもしづらい。

`SubagentDispatchRequired` variant は dispatch contract を表す usecase 層の意味論であり、結果として cli_driver 層 dispatch arm の挙動が決まる。本 ADR の主軸 (`task-contract` gate) とは layer も responsibility も異なる隣接決定であり、review-fix wiring の Claude path における structural 不具合の修正を記録する。

### D9: task-contract.json の task キー ↔ impl-plan.json task IDs の referential integrity 検証

`task-contract.json` の `entries` map のキー (task ID, 例: `T001`, `T015`) は、`impl-plan.json` の `tasks[].id` 集合に **必ず含まれる** 必要がある。`coverage` ゲート (`bin/sotp task-contract coverage`) は attribution 完全性の検証時にこの referential integrity も併せて検証する。

具体的には:

1. `task-contract.json` の各 task キーが `impl-plan.json` の `tasks[].id` 集合に含まれているかを確認する。
2. 含まれない task キーが存在した場合、その task キー配下の全 entry を `CoverageViolation::InvalidTaskRef { task_id, entry_keys }` として報告し、`coverage` ゲートを fail-closed させる。
3. これにより、`impl-plan.json` から task が rename / 削除されたあと `task-contract.json` を更新し忘れて stale 参照が残った状態でも、coverage が silently pass するのを防ぐ。

ゲートの合否基準への追加: 既存の「scope 内の関連 entry が漏れなく attribution されている」+「D7 の task status に基づく生存性判定 (done / in_progress は 🔵 必須、todo は 🟡 許容、🔴 は常に blocker)」に加えて、本 D9 で「全 task キーが impl-plan.json に存在する」を invariant として追加する。

D1 が定めた attribution map の完全性概念を、本 D9 が referential integrity の側面で厳格化する。D3 の "完全性" 部分の補強であり、D7 で確立した impl-plan を SSoT とする方針と整合する。

## Rejected Alternatives

- **A. 「履行」判定に test-pass / stub-scan（body-aware liveness）を含める**: 却下。body は rustdoc JSON に載らず現エンジンでは原理的に見られないため、別データ源（syn 走査 / nextest 名前フィルタ）と新検査系の新設・保守が要る。すり抜ける stub は reviewer（body を読む）と commit/merge gate の test が安く捕まえるので、その狭い残コストのために検査系を抱えるのは ROI が悪い。
- **B. task→entry を型カタログ（Phase 2）に載せる**: 却下。type-designer の責務が膨張し、Phase 2 成果物が Phase 3 概念（task）へ後方依存する（SoT Chain 逆流・設計順序違反）。
- **C. task→entry を既存 `task-coverage.json` に同居させる**: 却下。spec coverage（完全性）と型契約 attribution（conformance ゲート入力）という別責務が混在し、別 SSoT（spec vs カタログ）・別ゲート・別 invariant を1ファイル / 1 evaluator が抱える（artifact 単位の SRP 違反）。ファイル増を避ける安価さより責務分離を優先する。
- **D. 新 chain（task_conformance）を切る**: 却下。gate matrix / strictness / views の表面積が増える。per-task は既存 `impl_catalog` 信号 + Phase 3 マッピングの JOIN で足り、binary check で実装可能。
- **E. shift-left せず現状維持（pre-review prepend は freshness のみ、blocking は commit=interim / merge=strict）**: 却下。契約一致の hard gate が merge までかからず、違反が track 最終盤まで遅延する。早期・per-task の検出という本 ADR の目的を満たさない。
- **F. reviewer に prose の「ソースコード理解キャッシュ」を渡して前倒しの代替とする**: 却下。review は adversarial であり、lossy な散文サマリは「キャッシュを信じて見落とす」穴を作る。安全なのは契約 SSoT（型カタログ）と信号状態を渡すことであり、本 ADR の D2 はその安全形に限定する。
- **AA. task キー RI 検証を coverage 外の別 verify subcommand に切り出す**: 却下。D1/D3 の attribution 完全性は coverage の責務として既に確立。RI check も attribution 完全性の自然な拡張なので同一 subcommand 内に置くのが SRP として正しい。verify-* に新規 subcommand を切ると surface を不必要に増やす (`knowledge/conventions/workflow-ceremony-minimization.md` の精神に反する)。
- **AB. stale task キー配下の entry を silently 無視する**: 却下。silently 無視すると stale entry が catalogue 全集合の attribution カバレッジ判定に含まれず、catalogue にあるはずの entry が attribution されていない bug を覆い隠す。fail-closed が安全。

## Consequences

### Positive

- 契約違反（宣言シンボル欠落 / shape ずれ）が **authoring 直後・per-task で表面化**し、merge 直前での発覚と track 最終盤の手戻りを避けられる（shift-left の主目的）。
- reviewer が「カタログと形すら合わないコード」に消費されるのを入場前に防げる。
- 既存 `impl_catalog` 信号を再利用し、型カタログ / type-designer は不変、新 chain も不要。変更面は (D1) 新 Phase 3 artifact `task-contract.json` と (D2) binary check の新設に限られる（検証エンジンは再利用）。
- task→entry マッピングは副次的に review の split-key（cohesion 境界での分割）としても再利用できる。
- ゲートは deterministic / binary で、SoT-chain-binary-check の枠組みに乗る。
- attribution map の参照整合性が機械検証されるようになり、impl-plan からタスクが消えた後の attribution drift が早期検知される。
- coverage ゲートの「pass = catalogue 全 entry が active task に紐付き、active task は全て impl-plan に存在する」という統一的な不変条件が成立する。

### Negative

- 🔵 は body を見ないため、正しい signature の stub（`todo!()`）はゲートを通る（liveness 未保証）。下流（body-aware reviewer / commit・merge gate test）が backstop する前提に依存する。
- 新 Phase 3 artifact `task-contract.json` 1つ分の surface が増える（schema / codec / gate 配線、および attribution completeness / referential integrity の維持＝ entry の rename / 削除で orphan / drift し得る）。ただし drift は既存の task↔spec と同種で新しいリスククラスではない。
- impl-planner の責務がわずかに増える（新 artifact `task-contract.json` の author を追加）。
- ゲートが blocking なので、🔵 になっていない段階の探索的 / 部分実装に対して review を妨げ得る（interim 的運用や per-task scope での緩和は将来検討）。
- 既存の task-contract.json を impl-plan.json と照合する必要があり、stale 残留があれば修正コストが発生する。
- `CoverageVerifyService` 実装に `impl-plan.json` の task IDs 取得 step が増える (既存 `ImplPlanReaderPort` の拡張または method 追加)。

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
