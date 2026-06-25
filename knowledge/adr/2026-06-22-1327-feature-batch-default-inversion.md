---
adr_id: 2026-06-22-1327-feature-batch-default-inversion
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01SZcZJqhrMJUAhVDuqWsEGr:2026-06-22"
    candidate_selection: "from:[per-task-status-quo,layer-wave,full-task-dag,worktree-per-task,feature-batch] chose:feature-batch"
    status: accepted
  - id: D2
    user_decision_ref: "chat_segment:session_01SZcZJqhrMJUAhVDuqWsEGr:2026-06-22"
    status: accepted
  - id: D3
    user_decision_ref: "chat_segment:session_01SZcZJqhrMJUAhVDuqWsEGr:2026-06-22"
    status: accepted
  - id: D4
    user_decision_ref: "chat_segment:session_01SZcZJqhrMJUAhVDuqWsEGr:2026-06-22"
    status: accepted
  - id: D5
    user_decision_ref: "chat_segment:session_01SZcZJqhrMJUAhVDuqWsEGr:2026-06-22"
    status: accepted
---
# feature バッチ消化への既定反転 — per-layer 並列レビューを始動させる

## Context

adr2pr の**内部スループット**（1つのトラックを ADR から PR まで通す速度）を上げたい。現状の取りこぼしは次の連鎖で説明できる。

**1. レビュー爆発がある.** レビューコストは差分量に対して O(N²)（理解 O(N) × 指摘 O(N)）で増え、差分が大きいほど急速に重くなる。

**2. その対処として、コミット単位を小さく保っている.** adr2pr の実装フェーズを担う `/track:full-cycle` は、1つの feature を**タスク単位**で implement → review → commit と細かく区切り、各コミットの差分を小さく抑える。複数タスクを束ねる例外（`adr2pr.md:41` の Constraint 3）はあるが、密結合で個別に CI を通せないときしか発火しないため、実質は per-task で回る。

**3. しかし、いまの full-cycle は安全側に倒しすぎている.** 上限を各レイヤーではなく**コミット全体**に課し、各レイヤーにまだ余裕があってもタスク境界ごとにコミットを切る。結果、継ぎ足せば1ラウンドで並列レビューできた差分が複数ラウンドに分断され、タスクの実装とレビューが直列化して、せっかくの並列レビューを生かし切れていない。

（補足）本来レビューは scope（レイヤー）ごとに独立・並列で走る（`review.md:79-81,:139`）。であれば抑えるべき上限は**各レイヤーの差分**でよく、各レイヤーが上限に収まる限りタスクをまたいで差分を**継ぎ足してよい**（依存は「先行のコードが working tree に在ればコンパイルできる」実装順序の制約にすぎず commit は要らない）。どれかのレイヤーで上限超過が**不可避になった時点で初めて**コミットを切れば足りるのに、full-cycle はこの余裕を使っていない。

**本 ADR はこれを是正する.** 上限の対象をコミット全体から各レイヤーへ移し、各レイヤーの上限が許す限り差分を継ぎ足してから、1回のレビューでまとめて捌くよう消化の既定を変える（Decision D1–D3）。

## Decision

### D1: 消化単位を per-task から feature バッチへ反転する

full-cycle の既定を per-task 直列から **feature バッチ**へ反転する。1つの feature を構成するタスク群（例: domain → usecase → infrastructure と層ごとに分割した T001/T002/T003）を、依存に従った**実装順序**で同一 working tree に一括実装し、その間に commit を挟まない。commit は per-task ではなく、次のいずれかで切る: (a) feature の完了、または (b) 次に実装するタスクが、すでに差分のあるいずれかのレイヤーの累積差分を D3 の上限超過に至らせるとき（その直前で現バッチを commit し、以降を新バッチに）。あるレイヤーが上限に達しても、控えているタスクが**別レイヤー**ならそのレイヤーの累積は増えないので、**継ぎ足しを続けてよい**。

### D2: バッチ差分に既存の per-layer 並列レビューを1回当てる

バッチ実装後、その差分に既存の `/track:review` を1回走らせる。差分が複数の scope（層）に跨る場合、それらが同時に埋まるので scope 独立の並列レビューがその並列度をフルに発揮する。**新しいレビュー機構は作らない** — 既存の review は scope 独立並列であり、必要なのは機構の追加ではなく「1 feature 分の差分をまとめて与え、ラウンドへの分断を避ける」ことだけである。

### D3: review コスト天井を per-layer-scope 単位で課し、その値は外部設定から注入する

レビューは scope（層）独立で**並列**に走るので、O(N²) のコスト天井は **per-commit ではなく per-layer-scope** に課す。各層が天井以下なら壁時計のレビュー時間は **1 層分の O(天井²)** で頭打ちになるため、バッチ commit は総差分が層数倍（最大「層数 × 天井」）まで膨らんでよい。

**天井値はコードに焼き込まず、`.harness/config/review-scope.json` に設定する**。review-scope.json は既に per-group（層）設定を持つので、per-scope 天井の自然な置き場所になる。グローバル既定（目安 ~500 行）を持ちつつ、scope（層）ごとに上書き可能とする。planner はこの設定値を読んでバッチを sizing し、ある層の累積差分が天井を超える場合は、その層の作業を**計画段階で複数バッチに細分化**する（その層内については直列化を受容）。1トラックの実レビュー round 数は概ね `max_over_layers( ceil(layer_work / ceiling_for_layer) )` に律速され、典型的な multi-scope feature では per-task 直列の N 回より小さくなりやすい。ただし作業が1層に集中する場合はその層内の分割数が律速し、N 回未満を保証しない。これは既存の小コミット規約（差分を <500 行に分割せよ。`.claude/rules/10-guardrails.md`）を、並列レビュー世界向けに **per-scope 粒度かつ設定可能な形へ精緻化**したものである。

### D4: バッチの単一 commit を各タスクに同一 commit_hash で記録する

バッチは1 commit にまとめ、その commit hash をバッチ内の全タスクに**同一 hash**で記録する（`bin/sotp track transition <id> done --commit-hash <hash>` を各タスクに適用）。`TaskStatus::Done` は commit_hash を所有すれば足り、hash の unique 制約は無い（確認済み）。これにより per-task のトレーサビリティ（metadata の commit_hash）を保ったままバッチ commit を実現する。

### D5: 機構の所在は既存コマンドの修正に限る（新スケジューラ・新スキーマを作らない）

実装面は既存資産の再利用と既定反転に限定する:

- `/track:implement` は既に対象タスク群を解決して並列実装でき、`/track:review` は既に per-scope 並列、`/track:commit` は既に単一 commit を作る。必要なのは `/track:full-cycle` がこれら既存コマンドを per-task ではなく feature バッチ単位で1回ずつ呼ぶようにすることだけで、**空回りを止めるだけ**で機構は足りる。
- 変更面は `full-cycle.md` のループ構造（`:28/:37/:53/:58`）、`adr2pr.md:41` の既定反転、`review-scope.json` からの天井設定読み出し、および D4 の hash 記録。
- 典型の単一 feature では **schema 追加も `layer`/`feature` タグも新スケジューラも不要**。
- 1トラックに複数の独立 feature が同居する場合のバッチ境界 grouping は本 ADR の対象外（将来課題、Reassess 参照）。

## Rejected Alternatives

### A. layer-wave fork-join スケジューラ

タスクを `layer` でバンドルし topological な wave として流し、wave 境界に barrier を置く案。却下: 波が層を直列化するため各波で1層しか動かず、**per-layer 並列レビューをむしろ殺す**。vertical slice では波が薄く利得が出ない。新 `layer` タグとスケジューラを要する。本質は「既定反転」で足り、スケジューラは過剰。

### B. full task-DAG（`depends_on` 辺）スケジューラ

タスク個別の依存辺で細粒度スケジュールする案。却下: `depends_on` の構造化コストと、辺誤りによるコンパイル崩壊リスク。典型の単一 feature では辺を持たずとも tree 内の実装順序で足りる。粒度不足が実証された段階で将来検討する。

### C. worktree-per-task + branch + merge

タスク毎に worktree/branch を切り最後に merge する案。却下: git merge が HITL ブロック対象。レイヤーがファイル素集合のため隔離は不要で、同一 working tree ＋ scope 境界規律で衝突しない。重い。

### D. per-feature 並列レビュー機構の新規実装

レビューを feature 粒度で再キーする新機構を作る案。却下: 既存 review v2 が既に scope 独立並列。空回りの原因は機構不足ではなく入力（1層差分）不足であり、バッチ差分を与えれば既存機構で per-layer 並列が起動する。新規実装は不要。

### E. 現状維持（adr2pr Constraint 3 のまま）

per-task を既定にし密結合時のみ batch する現状。却下: feature が複数タスクに分割されると review/CI/commit が round に分断され、まとめれば集約できるものを取りこぼす。各レビューを小さく保つ利点はあるが、その代償に余分な round を払う。D1+D3 は round を集約しつつ、per-scope は天井で小さく保てる。

## Consequences

### Positive

- レビュー・CI・commit の round が、典型的な multi-scope feature では per-task の N 回（直列）からバッチの `max_over_layers(ceil(layer_work/ceiling))` 回へ縮みやすい。差分が複数 scope に跨る分は同一 round 内で並列レビューされる。
- 完成済みの per-layer 並列レビュー機構が（例外時だけでなく）既定経路でも十全に働き、機構への投資が回収される。
- per-scope レビューコストは天井で抑制され、バッチ化しても O(N²) は各 scope に閉じる。天井値は外部設定（`.harness/config/`）で注入するため、プロジェクト・層ごとにチューニングでき、コード変更なしで較正できる。
- commit は feature 単位で coherent（bisect 可能）、各タスクへの hash 記録でトレーサビリティを保持する。
- 新スケジューラ・スキーマ・タグ不要（典型 feature）で、変更面が小さい。

### Negative

- optimistic batching: 未レビューの下層の上に上層を積むため、domain レビューの指摘が usecase/infra へ手戻りを波及させうる（多くの feature では domain が先に固まるため許容範囲）。
- バッチ commit の総差分は層数倍まで大きくなる（per-scope レビュー量は増えないが、commit の見かけサイズは増える）。
- 実装自体は依存チェーン上では直列のままで、実装を並列化する短縮は独立タスクの無い feature では得られない。バッチ化の主な利得は review/CI/commit の round 集約（依存チェーンでも得られる）であり、feature が元々1タスクなら利得は小さい。
- 同一層に大きな作業が集中する feature では D3 により層内が直列化され、その層が round 数の律速になる。
- 1トラックに複数 feature が同居する場合のバッチ境界決定は未解決（将来課題）。

### Neutral

- DRY ゲートとは直交する: 本決定は DRY の有効・無効に依存しない。DRY は commit ごとに whole-corpus で走るゲートで、レイヤー単位の review 上限とは別軸であり、batching は DRY の実行回数を減らすのみ。

## Reassess When

- 1トラックに複数の独立 feature が常態的に同居するようになる（バッチ境界を切る grouping ＝ feature タグが必要化する）。
- typical なトラックが単一縦チェーン偏重と判明する（intra-track バッチの費用対効果が低く、別軸の throughput 策を検討する）。
- review コストの O(N²) モデルや天井の既定値が実測と乖離する（設定値の再較正 — コード変更不要で `review-scope.json` の更新で対応）。
- per-layer 並列レビューの scope 定義（`.harness/config/review-scope.json`）が変わり、バッチ差分の scope 充足前提が崩れる。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/hexagonal-architecture.md` — レイヤー素集合・port 配置の前提
- `knowledge/conventions/track-lifecycle.md` — タスク状態遷移・commit_hash・トレーサビリティ
- `architecture-rules.json` — レイヤー依存方向の SSoT
- `.harness/config/review-scope.json` — D3 の review コスト天井の設定場所（既存の per-group 設定を拡張）
- `.claude/rules/10-guardrails.md` — 小コミット規約（O(N²) レビューコスト）。D3 はこれを per-scope 粒度かつ設定可能な形へ精緻化する
