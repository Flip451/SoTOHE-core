---
adr_id: 2026-06-10-0413-dfp-rfp-loop-cost-reduction
decisions:
  - id: D1
    candidate_selection: "from:[unconditional-dfl-loop, external-check-first-gate, efficient-dfl-loop-conditional-cirust] chose:efficient-dfl-loop-conditional-cirust"
    user_decision_ref: "chat:2026-06-14:dfp-rfp-loop-cost-reduction-approved"
    status: proposed
  - id: D2
    candidate_selection: "from:[prompt-driven-fixpoint, mechanized-resolver] chose:mechanized-resolver"
    user_decision_ref: "chat:2026-06-14:dfp-rfp-loop-cost-reduction-approved"
    status: proposed
  - id: D3
    candidate_selection: "from:[sequential-judge, batched-multi-pair-prompt, bounded-parallel-fanout] chose:bounded-parallel-fanout"
    user_decision_ref: "chat:2026-06-14:dfp-rfp-loop-cost-reduction-approved"
    status: proposed
  - id: D4
    candidate_selection: "from:[flat-flagship-judge, flat-lightweight-judge, uncalibrated-two-tier-escalate-fails-only, calibrated-two-tier-with-known-bad-probes] chose:calibrated-two-tier-with-known-bad-probes"
    user_decision_ref: "chat:2026-06-14:dfp-rfp-loop-cost-reduction-approved"
    status: proposed
  - id: D5
    candidate_selection: "from:[self-contained-research-gate, pure-read-gate-coverage-manifest] chose:pure-read-gate-coverage-manifest"
    user_decision_ref: "chat:2026-06-14:dfp-rfp-loop-cost-reduction-approved"
    status: proposed
---
# DFP⇄RFP 往復コストの削減

## Context

track ワークフローのタスクごとのループは、実装 → DFP（DRY fix phase: DRY gate 通過まで検出と修正を回すフェーズ。修正役は dry-fix-lead = dfl）→ RFP（review fix phase: 全 review scope が zero_findings になるまでレビューと修正を回すフェーズ。修正役は review-fix-lead = rfl）→ 両ゲートが同時に green になる fixpoint → commit、で構成される。この 2 フェーズ構成と back-edge（RFP の編集が新たな重複を生んだら DFP へ戻る）は `2026-06-02-0716-dry-checker.md` D2、blocking gate は同 D7、修正役の分離は同 D12、DFP の全コードベース単一スコープは同 D13 で決定済みである。

運用してみると、この DFP と RFP の往復がスループット上のボトルネックになっている。直近の運用実測（track `sot-chain-semantic-review-gate`）に基づく構造的なコスト要因は次の 4 つ:

1. **dfl の DFP ループが、修正ゼロでも修正ループ一式を無条件に走らせる。** DFP 入口は常に dfl を起動し、dfl は違反が 1 件もない場合でも `sotp dry write` → `cargo make ci-rust` → 再 `sotp dry write` → `sotp dry check-approved` を直列実行する。back-edge（RFP 後の DFP 再入）でも同じコストを払う。実測では DFP 4 周のうち 2 周が違反ゼロで、この無駄なループ一式（dfl の推論 約 35k トークン + `ci-rust` + 二重の `dry write`）が違反ゼロ周回でそのまま走った。コスト源は dfl の起動そのものではなく、修正ゼロでも走るループの中身（とりわけ `cargo make ci-rust`）である（D1）。
2. **DFP⇄RFP の収束判定が orchestrator の prompt 運用に委ねられている。** 「RFP が編集していないのに DFP を再起動する」「逆に再入が必要なのに省く」という両方向の判定ぶれが往復回数を増やす。実測では RFP 編集 → dry stale → DFP → dfl 編集 → review stale → RFP という連鎖の収束に 3 サイクルを要し、毎回 orchestrator が check-approved の結果を手動で解釈した。さらに commit gate は現在 review / ref-verify / dry の 3 ゲートの AND（コミットゲートのチェーンが `review check-approved` → `ref-verify check-approved` → `dry check-approved` を直列評価する）であり、prompt 運用で追跡すべきゲート状態は増えている。
3. **判定の agent 呼び出しが直列。** `sotp dry write` は未検証ペア 1 件ごとに判定 agent（1 ペア = 1 回の read-only サブプロセス起動と完了待ち）を同期的に呼び出すため、wall-clock が判定回数に比例して伸びる。判定はペアごとに独立しており、直列に待つ必然性はない。
4. **判定が常時最重量構成で走る。** dry-checker の判定モデルはフラッグシップで固定され（`agent-profiles.json` の dry-checker capability は `fast_model` 無しの単一 `model` 指定。現行 gpt-5.5）、reasoning effort も最大値が adapter にハードコードされている。判定対象は embedding 類似度の閾値で機械フィルタ済みのペア（入力は 2 fragment + 指示のみ）で、実測でも判定の多くは「impl ヘッダの繰り返しは通常の Rust 構造」級の自明な非違反却下だったにもかかわらず、全ペアに一律で最重量構成を適用している。同型の判定ゲートである reviewer / ref-verifier が軽量（fast）→ 重量（final）の 2 段構成を既に持つのと非対称でもある。

このほかに、偽陽性ペアの再燃（内容の微変更でペア識別子が変わり、判定済みの偽陽性ペアが再判定に回る）と、重複の発生源対策（設計・実装フェーズへの DRY 検証のシフトレフト）も観測されたコスト要因だが、**本 ADR では扱わない**。これらは別草案「DRY 検証のシフトレフト」に分離して保留し、本 ADR の各決定の導入後に DFP への violation 流入（検出数 / 本物率 など。severity は現テレメトリに無いため Reassess When 参照）を実測してから採否を決める。

なお、インデックス構築コストについては、ファイル単位の増分維持（変更ファイルのみ再埋め込みする永続インデックス）が既に決定済みである（`2026-06-04-1042-dry-checker-operability-and-batch-index.md` D6/D7）。本 ADR は索引構築の最適化を再決定しない。その永続増分インデックスを前提として、(a) ループの再入・収束の機械化（ムダ起動と往復回数の削減）、(b) 判定実行の並列化・軽量化、(c) ゲート評価自体の軽量化（`dry check-approved` の純読み取り化、D5）を決める。(c) はゲートの**評価コスト**を下げる決定であり、1042 が扱う索引 BUILD（埋め込み・永続化・増分維持）の最適化とは別軸である — D5 は索引の作り方を変えず、ゲートが索引検索に依存すること自体をやめる。

なお、本 ADR の効果計測は直近に導入されたテレメトリ（`track/items/<id>/logs/telemetry.jsonl` に `ReviewRound` / `GateEval` / `ExternalSubprocess` 等を追記。スキーマは `libs/infrastructure/src/telemetry/mod.rs` の `TelemetryEvent`）に依存する。テレメトリが記録するのは wall-clock（`duration_ms`）と構造カウント（`findings_count` / round 数 / subprocess 数）であり、**トークン数・モデル単価・severity は記録しない**。したがって本 ADR の成功指標と Reassess-When トリガーは、トークン建てではなくテレメトリが実測できる量で定義する（各決定の「成功指標」および Reassess When を参照）。判定の verdict 分布（not-a-violation / accepted / violation）は per-track の dry-check.json から別途取得する。

## Decision

### D1: dfl の DFP ループを効率化する — 修正を適用したときだけ ci-rust と再 dry write を走らせる

dfl の DFP ループ（`.claude/agents/dry-fix-lead.md`）は現在 `sotp dry write` → 修正適用 → `cargo make ci-rust` → 再 `sotp dry write` → `sotp dry check-approved` の 5 ステップを**無条件で直列実行**しており、violation が 1 件も無く修正を 1 つも適用しなくても `cargo make ci-rust`（フル Rust CI）と 2 度目の `dry write` が走る（Context の要因 1）。これを、**実際に修正を適用したときだけ ci-rust と再 dry write を走らせる**ループに変える:

1. `sotp dry check-approved`（D5 で安価）。Approved → `completed`（無変更 back-edge 再入を素通り。`dry write` も走らない）。
2. Blocked → `sotp dry write` で未判定ペアを判定（既判定は cache-hit で no-op）。
3. judge 後に violation ゼロ → `sotp dry check-approved`（Approved）→ `completed`。**`cargo make ci-rust` と再 `dry write` はスキップ**する（検証すべき修正が無い）。
4. violation あり → 修正適用 → `cargo make ci-rust` → `sotp dry write` → `sotp dry check-approved` → loop（4 へ）または completed / blocked。

`cargo make ci-rust` は「適用した修正の検証」であり、修正ゼロなら検証対象が無く走らせる意味が無い。2 度目の `dry write` も修正後の再記録なので、修正が無ければ冗長。両者を step 4（修正適用後）の中だけに置くのが本決定の核心。

dfl の**起動そのものは高くない** — 高かったのは、起動後に修正ゼロでも払っていた `cargo make ci-rust`（cargo make 呼び出しでテレメトリにも出ない隠れコスト、フル CI で分単位）と冗長な 2 度目の `dry write`。ループを効率化すれば、違反ゼロの dfl 起動は「安価な check-approved（+ 必要なら判定 1 回の `dry write`）→ completed」で終わり、**dfl 起動を外部で gate する必要が消える**（Rejected Alternatives I）。orchestrator / D2 のルータは常に DFP ループを呼び、ループ自身が「直すものが無ければ安く no-op する」。

役割分担: 無変更 back-edge 再入での dfl 起動オーバーヘッド自体は **D2**（fixpoint ルータ）が安価な `dry check-approved` を読んで「dry green なら DFP をスキップ」することで回避する（dfl を呼ばない）。**D1** は dfl が呼ばれた後の中身を安くする — D2 が呼ぶ前に振り分け、D1 が呼ばれた後を効率化する。

理由: 違反ゼロのケース（実装・RFP の編集が重複を生まなかった）が定常状態として最も多く、実測でも DFP 周回の半数〜2/3 がこれに該当した（Context の要因 1）。そこで `cargo make ci-rust`（フル CI、隠れた分単位コスト）を毎回払うのが最大のムダ。`sotp dry write`（判定）は violation の有無を知るために要るが、ci-rust と再 write は「修正があった後」だけで必要十分。gate の fail-closed 性（`2026-06-02-0716-dry-checker.md` D7）は変わらない（check-approved が Blocked のときは必ず judge され、未解決 violation は通過できない）。実装は dfl 定義（`.md`）に分岐を 1 つ足すだけで、Rust 変更は不要。

**成功指標（テレメトリ実測）**: (i) **違反ゼロ dfl run の wall-clock 崩落**: violation ゼロで終わる dfl run の wall-clock span が、`cargo make ci-rust`（フル CI、分単位）を含まない floor（`dry write` ≤ 1 回 + check のみ）に収束する。ci-rust は cargo make 呼び出しでテレメトリに出ないため、dfl run の総 wall-clock（dfl の dry ReviewRound 群とその終端までの span）で代理測定する — ci-rust スキップで span が大きく縮む。(ii) **機会量**: ゼロ違反率 = `count(dry ReviewRound, findings_count=0) / count(all dry ReviewRound)`（ベースライン: テレメトリ実測トラックで 2/3、Context 引用の `sot-chain-semantic-review-gate-2026-06-07` で約半数）が対象母集団のサイズを示す。(iii) 直接計測したい場合は、no-fix completed を識別する `dry check-approved` の `GateEval`（gate_name=`dry`、verdict Approved→`ok` / Blocked→`error`）か、ci-rust 実行点を示す `GateEval` を 1 つ足す（既存 `emit_gate_eval`、テレメトリ schema 変更は不要）。

### D2: DFP⇄RFP の fixpoint 判定を機械化する

「全ゲートが同時に green」という commit 前提条件（`2026-06-02-0716-dry-checker.md` D2 の fixpoint）の判定を、orchestrator の prompt 運用から単一の決定的コマンドに移す。コマンドは DRY gate の状態（`sotp dry check-approved` 相当）、review の per-scope 状態（hash 失効・verdict）、および ref-verify gate の状態（`sotp ref-verify check-approved` 相当）を公開 API 経由で合成し、「次に必要なフェーズ（DFP / RFP の対象 scope / ref-verify 再実行 / なし = commit 可）」を出力する。commit gate は現在 review / ref-verify / dry の 3 ゲートの AND であるため（Context の要因 2）、fixpoint 判定はこの 3 ゲートすべてを覆う。orchestrator はこの出力に従うだけで、収束判定ロジックを持たない。

ゲート間の疎結合（同 D1）は維持する: このコマンドは各ゲートの公開インターフェース（gate 判定の exit code / 読み出し API）だけを合成し、dry-check.json / review.json / ref-verify cache の内部構造には依存しない。

**D1 との責務境界**: D5 により全ゲートの check-approved が安価な純読み取り（staleness + all-resolved）になるため、本コマンドは 3 ゲートの check-approved を読み合成するだけでよく、dfl ループ（D1）の先頭 check-approved が同じ `dry check-approved` を読むことと**二重実行になっても問題にならない**（安価な読みは何度実行してもよい）。重い処理（類似検索・判定・review・ref-verify run）は各 resolver（`dry write` / rfl / `ref-verify run`）に集約され、check-approved 側には無い。したがって責務は素直に分かれる — **本コマンド = 安価な読み取りの合成ルータ、各フェーズ = 自分のゲートの単一 resolver**。D5 を採らない場合に必要となる「同一ゲートの二重評価を避ける単一所有者の調停」は、D5 により不要になる（Rejected Alternatives G）。

理由: 収束判定の材料（各 gate の状態・scope hash の失効）はすべて機械可読であり、prompt 運用に残す必然性がない。実測では収束に 3 サイクルを要し、毎回 orchestrator が check-approved の結果を手動解釈していた（Context の要因 2）。決定的に解決すれば、再入の過不足という判定ぶれが構造的に消える。

### D3: 判定の fan-out を上限付き並列で実行する

`sotp dry write` の判定実行を、ペアごとの直列呼び出しから上限付きの並列 fan-out に変える。処理を 2 相に分ける:

1. **照会相**: 全 diff fragment の類似照会を先に行い、ペア識別子で重複排除した未検証ペアの一覧を確定する（現在は照会 → 判定 → 記録がペアごとに交互に走り、run 内の検証済み集合の逐次更新が同一ペアの二重判定を防いでいる。これを事前の重複排除に置き換える）。
2. **判定相**: 確定した一覧に対し、設定可能な上限付き並列度で判定 agent を同時に呼び出す。判定の単位は 1 ペア 1 呼び出しのまま変えない（Rejected Alternatives D）。永続化は完了順ではなくペア識別子順で append し、dry-check.json の記録順を決定的に保つ。判定エラーは収集して報告し、1 件のエラーで残りの判定結果を破棄しない。

並列度の上限は **`.harness/config/dry-check.json` から注入する**（既存の `threshold` を持つこの設定に `max_parallelism: Option<usize>` を追加。未指定なら nonzero の既定値）。これは ref-verify が `max_parallelism` を `.harness/config/ref-verify.json` から読む実装（`RefVerifyConfigDto` + `unwrap_or_else(default)` + usecase 層の検証付き newtype）の踏襲である。モデル選択（provider / model）は agent-profiles.json、並列度などの運用チューニングは per-gate の dry-check.json、という ref-verify と同じ分離にする。provider の rate limit / アカウント同時実行制限に合わせて運用側が調整する（無制限の並列は行わない）。

理由: 判定はペアごとに独立した read-only の agent 呼び出しであり、直列に待つ必然性がない（Context の要因 3）。判定ポートは既にスレッド間共有可能な契約（Send + Sync）であり、同期ベースラインのまま実現できるため実装コストは低い。D1（no-fix run での ci-rust スキップ）・D4（判定 1 回あたりの単価）と直交して、未検証ペアが残る run の wall-clock を並列度に応じて削る。

### D4: dry-checker の判定を較正付き 2 段構成にし、reasoning effort のハードコードを設定に昇格する

dry-checker の verdict 判定を、ref-verifier ゲートと同型の**較正付き 2 段構成**（既知の誤り例の注入と検出率較正。`2026-05-27-1601-sot-chain-semantic-review-gate.md`）にする:

- **fast tier（1 次判定）**: 軽量モデルで全ペアを判定する。モデルは `agent-profiles.json` の dry-checker capability に `fast_model` を追加して解決する。このとき**既知違反プローブ**（真の DRY 違反と確定済みのペア集合。過去に violation と判定された実例や合成例から整備し、具体的な整備方法は実装時に確定する）をバッチに混ぜて判定させる。
- **較正判定**: fast tier のプローブ検出率（既知違反を violation と判定できた割合）が閾値以上なら fast tier を信頼し、not-a-violation / accepted をそのまま最終 verdict として確定する。violation と判定不成立（出力不正・タイムアウト等）のみ、重量級モデル（既存の `model`）+ 高 reasoning effort に escalation して再判定する。
- **較正失敗時**: プローブ検出率が閾値未満なら fast tier の判定を信頼せず、fast の非違反判定を含む全ペアを重量級 tier で再判定する。重量級 tier でもプローブ検出率が閾値未満なら、判定系自体の劣化として gate を fail-closed にし、人間へエスカレーションする。
- プローブの verdict は dry-check.json の production 記録には残さない（プローブと production の記録は分離する）。プローブの注入率と検出率閾値は `.harness/config/dry-check.json` で調整可能とする（ref-verify が `known_bad_injection_rate_percent` / `known_bad_detection_threshold_percent` を `.harness/config/ref-verify.json` に置くのと同じ配置。tier 別の `model` / `fast_model` だけ agent-profiles.json）。

あわせて、adapter にハードコードされている reasoning effort（`model_reasoning_effort="high"`）を capability 設定へ昇格し、tier ごとに指定可能にする。

理由: 判定対象は embedding 閾値で機械フィルタ済みの 2 fragment のみで、実測でも判定の多くは自明な非違反の却下だった（Context の要因 4。`sot-chain-semantic-review-gate-2026-06-07` の dry-check.json では not-a-violation 546 / accepted 54 / violation 59 = 非違反 91%）。最重量構成（フラッグシップ + 最大 reasoning effort）の全ペア一律適用は、ペア数 × 思考トークン単価の両方に効く過剰コストになっている。一方、単純な 2 段化は軽量 tier の見逃し（false not-a-violation）がそのまま確定する保険の無い構成になる（Rejected Alternatives F）。既知違反プローブの検出率で fast tier の信頼性を run ごとにその場で較正すれば、「fast の非違反を信頼してよい run」と「重量級で再判定すべき run」を機械的に区別でき、見逃し側にも保険がかかる。誤 violation 側は escalation の重量級確認で防ぐ。較正機構は ref-verify ゲートの known-bad probe 実装（`libs/usecase/src/ref_verify/interactor.rs` のプローブ注入 → 検出率閾値 → 較正失敗時の全ペア重量再判定 → fail-closed escalation の FSM）が本番稼働済みであり、その実装パターンを流用できる。D3（並列化）が wall-clock を削るのと直交し、本決定は判定 1 回あたりの単価を削る。

**成功指標（テレメトリ実測）**: dry ラウンドを **tier タグ付きで発火**する（`round_type ∈ {fast, final}` + tier 別 `model`。reviewer が既に用いる `emit_review_round` と同じ経路で、テレメトリ schema 変更は不要）ことを前提に、(i) **fast 完結率** = `count(fast round で同 scope の final 後続なし) / count(fast round)` を高く、(ii) **昇格率** = `count(final round) / count(fast round)` を低く保つ。(iii) **fast ラウンドの `duration_ms` 中央値**が単一段ベースライン（gpt-5.5、実測 100–352s）より低下すること、(iv) **較正失敗率**（全ペアを final で再判定した round の割合）が常態化しないことを監視する。fast 化が見逃しを生んでいない sanity は、dry-check.json の violation 比率（実測 ≈9%）が不自然に低下しないことで確認する。判定 1 回あたりのトークン単価の絶対値はテレメトリでは測れないため、これらの wall-clock / tier シェアで代理する。

### D5: dry check-approved を純読み取りゲートにする — 類似検索は dry write に集約する

`sotp dry check-approved` を、ref-verify / review の check-approved と同型の**安価な純読み取りゲート**にする。現在は呼び出しごとに `corpus_fragments` からインデックスを再構築し埋め込み + 類似検索して現在の above-threshold ペアを導出している（`libs/usecase/src/dry_check/approval_interactor.rs`）が、これをやめる:

- **類似検索は `dry write`（resolver）にのみ置く。** `dry write` は処理した diff フラグメントの FragmentRef（= path + content_hash の組。カバレッジ記録）と発見ペア + verdict を記録する。同じ content_hash でも path が異なるフラグメントは別 FragmentRef として扱い、hash だけでは covered と見なさない。
- **`dry check-approved` は読み取り + FragmentRef 照合のみ**で判定する:
  1. 現在の diff フラグメントと各フラグメントの FragmentRef（path + content_hash）を git diff から得る（埋め込み・検索不要）。
  2. カバレッジ記録 + ペア記録（dry-check.json）を読む。
  3. **staleness**: 各 diff フラグメントの現 FragmentRef（path + content_hash）がカバレッジ記録に在るか。無ければ（編集または path 変更により未 `dry write`）→ Blocked。content_hash が一致しても path が異なる FragmentRef は covered と見なさない。
  4. **all-resolved**: 変更フラグメントに触れる記録ペアが全て not-a-violation / accepted か。violation あれば → Blocked。
  5. それ以外 → Approved。

これは ref-verify（verify-cache を読み、現在ペアの Pass 被覆を確認。モデル非起動）・review（review.json を読み、scope hash 失効と verdict を確認）の check-approved と同型である。blocking gate（`2026-06-02-0716-dry-checker.md` D7/D8）の意味論は保たれる: フラグメント編集または path 変更 → FragmentRef 変化 → カバレッジ記録に不在 → Blocked → `dry write` 強制（検索・判定・記録更新）→ 通過。新規類似（未変更フラグメントに新フラグメントが似る）も、後者が diff に居る以上その staleness で Blocked になり取りこぼさない。

理由: ゲートに「現在ペアの列挙」は不要で、必要なのは staleness 検出のみ。ゲート内の検索は `dry write` が必ず行う検索と冗長であり、削れる。これにより dfl ループの先頭 check（D1）・D2 の fixpoint 合成・commit ゲートの反復評価がすべて安価になり（埋め込み・検索なし）、check-approved を何度読んでも実害がなくなる（D1/D2 の責務境界が単純化する。D2 参照）。本決定はゲートの**評価コスト**を下げるもので、1042 D6/D7 が扱う索引 BUILD の最適化とは別軸（索引の作り方は変えず、ゲートが索引検索に依存すること自体をやめる）。

## Rejected Alternatives

### A. DFP と RFP の条件付き並列実行

dry violation が触る path 集合と review scope のファイル集合の交差が空なら DFP と RFP を並列に走らせる案。見送り: 編集衝突と scope hash 失効の管理が複雑になり、両ゲートの疎結合（`2026-06-02-0716-dry-checker.md` D1）にも負担をかける。本 ADR の低リスク改善で往復回数を削った後、なお並列化が必要かを実測してから再検討する。

### B. per-task DFP の間引き（全コードベース gate を PR 前 1 回に限定）

タスクごとの DFP をタスク diff 限定の軽量チェックに置き換え、全コードベースの gate は PR 前に 1 回だけ走らせる案。見送り: 既存決定（同 D7 の blocking gate / D13 の全コードベース単一スコープ、commit gate の DRY 前提条件）の改訂が必要なうえ、違反を後段に溜め込むほど修正コストが増えるリスクと相殺される。dfl ループ効率化（D1）により per-task DFP の定常コスト（違反ゼロ時）はほぼ判定のみになるため、間引きの動機自体が弱い。

### C. DRY 修正と review 修正の fix パス統合

reviewer の findings と dry の findings を単一 briefing で 1 つの fixer に渡し、1 パスで両方直す案。却下: 統合 fix-lead は `2026-06-02-0716-dry-checker.md` D12 で既に却下されており、phase 境界が消えて密結合になるという却下理由は現在も妥当。

### D. 複数ペアの一括判定（1 回の agent 呼び出しに複数ペアを詰める）

agent 呼び出し回数を減らすために、1 つのプロンプトに複数ペアを並べて一括で判定させる案。却下: ペアあたりの判定品質（precision）が落ちやすく、出力 schema も複雑になり、1 回の呼び出し失敗が複数ペアの判定を巻き添えにする。判定の単位は 1 ペア 1 呼び出しのまま、呼び出しの同時実行（D3）で速度を稼ぐ。

### E. 判定モデルの一律軽量化（2 段化なしで model 設定だけ下げる）

dry-checker の `model` を軽量モデルに差し替えるだけの案。設定変更のみで即実行できるが、却下: DRY gate は escalation の無い 1 段 blocking gate なので、軽量モデルの誤判定（見逃し = false not-a-violation / 誤 violation = 無駄な修正ループ）への保険が無くなる。較正付き 2 段化（D4）を採る。

### F. 較正なしの 2 段化（fast tier の非違反判定を無条件に信頼する）

既知違反プローブを使わず、fast tier の not-a-violation / accepted を常に確定し violation のみ escalation する案。誤 violation 側には重量級の保険がかかるが、却下: blocking gate の検出力が軽量モデルの能力に無条件で依存し、見逃し（false not-a-violation）はそのまま確定する。プローブ検出率による run ごとの較正（D4）で「fast の非違反を信頼してよいか」自体を機械判定する。

### G. dry check-approved を自己完結的な再検索ゲートのままにする

`dry check-approved` が呼び出しごとに索引を構築し再検索して現在ペアを導出する現行設計を維持する案（D5 を採らない）。利点はゲートが `dry write` の記録に依存せず単独で正しい verdict を出せること。却下: 毎ゲート呼び出しで埋め込み + 類似検索を払うため、dfl ループの先頭 check（D1）・D2 の fixpoint 合成・commit ゲートの反復評価が重くなる。staleness 検出はカバレッジ記録の hash 照合で足り、ゲート内検索は resolver（`dry write`）が必ず行う検索と冗長。ref-verify / review の check-approved は既に純読み取りで、dry だけが非対称に重い。D5 でこれを揃える。

### H. fixpoint 解決を既存 `track resolve` に統合する

`bin/sotp track resolve`（`TrackMetadata` + impl-plan から粗いライフサイクル `Planning/InProgress/...` と次コマンド `Implement/Done/...` を導く純関数。`libs/domain/src/track_phase.rs`）の `NextCommand` enum を拡張し、D2 の DFP/RFP/ref-verify/commit も同じコマンドで返す案。却下: 両者は altitude・入力・コスト・cadence・失敗様式が異なる — `track resolve` は metadata の純関数（ゲート I/O 無し、status 表示・registry レンダリング用）、D2 は working tree + 3 ゲート成果物に依存し eval 失敗を持つ per-iteration ルータ。lifecycle 用の軽い純関数にゲート評価を背負わせると関心が混線する。D2 は**別コマンド**とし、結果の形（phase / reason / next / blocker）は UX 一貫性のため踏襲しつつ、next-step の値空間は別 enum（`FixpointStep ∈ {RunDfp, RunRfp(scope), RunRefVerify, Commit}` 等）とする。`track resolve` = ライフサイクル、D2 = `InProgress` 内のゲートステップ、という階層で合成する（D2 は `track resolve` が `InProgress` のときのみ意味を持つ）。

### I. オーケストレーター外部で check-first して dfl 起動を gate する

DFP 入口で orchestrator が `sotp dry write` → `sotp dry check-approved` を先に走らせ、Blocked のときだけ dfl を起動する案（D1 の初期案）。dfl 起動を「悪いもの」とみなし外から止める発想。却下: dfl 起動そのものは高くなく、高いのは dfl ループ内の無条件 `cargo make ci-rust` + 冗長な 2 度目の `dry write`（Context の要因 1）。外部 check-first は (a) orchestrator の `dry write` と dfl 内の `dry write` を二重化し、(b) check-first ロジックを orchestrator に背負わせる。代わりに dfl ループ自体を効率化（D1）すれば、起動されても「直すものが無ければ安く no-op」で済み、無変更再入での起動回避は D2 のルーティングが担う。外部 gate は不要。

## Consequences

### Positive

- 違反ゼロの定常ケース（実測では DFP 周回の半数〜2/3）で、dfl ループが `cargo make ci-rust`（フル CI、隠れた分単位コスト）と冗長な 2 度目の `dry write` をスキップし、DFP がほぼ判定（`dry write`）コストだけになる（D1）。dfl 起動自体は安いまま no-op できる。
- fixpoint 判定が決定的になり、prompt 運用による再入の過不足（往復の無駄・再入漏れ）が構造的になくなる。commit gate を構成する 3 ゲート（review / ref-verify / dry）を 1 つの決定的出力で覆う（D2）。
- 未検証ペアが多い run の `sotp dry write` の wall-clock が並列度に応じて短縮される（D3）。
- 較正が健全な run では判定の大半（自明な非違反）が軽量 tier で完結し、判定 1 回あたりのコスト（モデル単価 × reasoning effort の思考トークン）が下がる（D4）。見逃し側はプローブ較正、誤 violation 側は escalation の重量級確認で、両方向の誤判定に保険がかかる。
- `dry check-approved` が安価な純読み取り（staleness + all-resolved の hash 照合のみ、埋め込み・検索なし）になり、dfl ループの先頭 check（D1）・fixpoint 合成（D2）・commit ゲートの反復評価コストが下がる。ref-verify / review と同型になり、二重実行も実害がなくなる（D5）。

### Negative

- 状態解決コマンド（D2）の実装・保守が増える。各ゲートの公開 API に限定して合成しても、出力（次フェーズ）の意味は 3 ゲートの仕様変更に追従し続ける必要がある。
- 判定並列化（D3）に伴い、記録順の決定化（ペア識別子順での append）とエラー収集の実装が増える。並列度の上限（`.harness/config/dry-check.json` の `max_parallelism`）は provider の rate limit / アカウント同時実行制限に合わせた調整が要る。
- 較正付き 2 段化（D4）には既知違反プローブ集合の整備・保守が必要で、プローブの質と量が較正の信頼性を決める（プローブが覆わないパターンの見逃しは較正をすり抜けうる）。プローブ判定の分だけ毎 run のコストが上乗せされ、較正失敗時は全ペア重量級再判定となるため最悪コストは現行より高い。escalation 機構と tier 別設定（`fast_model` / reasoning effort）の実装・保守も増える。
- 純読み取りゲート化（D5）には `dry write` がカバレッジ記録（処理した diff フラグメントの FragmentRef = path + content_hash）を残す必要があり、ゲートはその記録の存在に依存する（自己完結性を手放す）。staleness 照合は FragmentRef（path + content_hash の組）で行い、同一 content_hash・異なる path の FragmentRef を covered と誤認しない。記録の欠落・不整合時は fail-closed（Blocked）で安全側に倒す設計が要る。

### Neutral

- blocking gate（D7）・全コードベース単一スコープ（D13）・修正役分離（D12）・verdict 書き込み経路（D10/D11）（いずれも `2026-06-02-0716-dry-checker.md`）は変えない。
- 効果計測はテレメトリの wall-clock（`duration_ms`）と構造カウント（`findings_count` / round 数）で行い、トークン建ての絶対コストは測らない。`cargo make ci-rust` は cargo make 呼び出しでテレメトリに出ないため、D1 の効果（no-fix run での ci-rust スキップ）は dfl run の wall-clock 崩落で代理測定する（直接化したいなら no-fix completed を示す `GateEval` を 1 つ足す）。D4 の dry ラウンド tier タグ（`round_type` fast/final + tier 別 `model`）発火も既存 schema 内の配線で、新フィールド・新イベント型を要しない。

## Reassess When

- 本 ADR の決定群の導入後も DFP への violation 流入が恒常的に続く場合 — `dry ReviewRound.findings_count` の移動平均が閾値を超え続け、かつ dry-check.json の violation 比率が閾値を超えるとき（検出数 + 本物率の代理。**severity は現テレメトリに無いため指標から除外する**）— 別草案「DRY 検証のシフトレフト」の採否判断を起動する。
- dfl ループ効率化後も判定（`sotp dry write`）自体のコストがボトルネックとして残る場合 — 違反ゼロ dfl run の `dry ReviewRound.duration_ms`（ci-rust を除いた判定 floor）の中央値が閾値を超え続けるとき（fragment 単位の増分埋め込み — `2026-06-04-1042-dry-checker-operability-and-batch-index.md` Rejected Alternatives E — の再評価）。
- 機械化した fixpoint（D2）でも DFP⇄RFP が収束しない場合 — per-task で commit までに要した DFP⇄RFP の往復サイクル数（`dry ReviewRound`（DFP）と `review ReviewRound`（RFP）の交替を timestamp 順に数える）の中央値が、ベースライン（実測 3 サイクル）を恒常的に超えるとき（並列化や fix パス統合などの構造変更の再検討）。
- 判定並列度（D3）の前提（provider の rate limit / アカウント同時実行制限）が崩れた場合 — 外部要因（provider 側の方針変更）自体はテレメトリ外だが、症状は `ExternalSubprocess.retry_count` の上昇（rate-limit 由来のリトライ）や、`max_parallelism` を上げても未検証ペアが残る run の `dry ReviewRound.duration_ms` が短縮しない（並列が頭打ち）こととして観測される — 上限（`.harness/config/dry-check.json` の `max_parallelism`）の見直し。
- fast tier の見逃し（軽量 tier で確定した非違反が後に真の重複と判明するケース）が較正をすり抜けて観測された場合 — ①昇格率（`final` / `fast` round 比）の継続的な高止まり、②dry-check.json 履歴で同一 pair が not-a-violation →（内容変化後）violation に転じる頻度、③較正失敗率の常態化 のいずれか（D4 のプローブ集合の拡充・検出率閾値の見直し、または全件重量判定への回帰）。
- D5 の純読み取りゲートが staleness を取りこぼす（カバレッジ記録の被覆漏れで、未 `dry write` のフラグメントを誤って Approved とする）ケースが観測された場合 — ゲートの被覆モデル（フラグメント粒度のカバレッジ記録）の見直し、または自己完結再検索（Rejected Alternatives G）への部分回帰。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-06-02-0716-dry-checker.md` — DFP/RFP の 2 フェーズ構成・blocking gate・ペア verdict キャッシュ・修正役分離・単一スコープの原典
- `knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md` — 永続増分インデックス（本 ADR が前提とする索引基盤）
- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md` — 較正付き 2 段判定（既知の誤り例の注入・検出率閾値・段階引き上げ）の原型（D4 が踏襲）。ref-verify ゲート（D2 の fixpoint 合成対象の 1 つ）の定義元
- `.harness/config/agent-profiles.json` — dry-checker capability の provider / model / fast_model 解決
- `.claude/agents/dry-fix-lead.md` — dfl の DFP ループ定義（D1 が効率化する対象）
- `.harness/config/dry-check.json` — DRY ゲートの `threshold` / `max_parallelism`（D3）/ プローブ設定（D4）
