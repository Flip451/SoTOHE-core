---
adr_id: 2026-05-27-1601-sot-chain-semantic-review-gate
decisions:
  - id: D1
    user_decision_ref: "chat:2026-05-27:independent-review-lane"
    candidate_selection: "from:[signal-integrated,independent-review-lane] chose:independent-review-lane"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-05-27:layered-with-cache"
    candidate_selection: "from:[phase-only,merge-only,layered-with-cache] chose:layered-with-cache"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-05-27:anchor-existence-check-plus-git-blob"
    candidate_selection: "from:[canonical-hash-for-adr,git-blob-hash-only,anchor-existence-check-plus-git-blob] chose:anchor-existence-check-plus-git-blob"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-05-27:per-entry-hash-for-catalogue"
    candidate_selection: "from:[per-file-hash,per-entry-hash-for-catalogue] chose:per-entry-hash-for-catalogue"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-05-27:model-tier-funnel"
    candidate_selection: "from:[human-direct,model-tier-funnel] chose:model-tier-funnel"
    status: proposed
  - id: D6
    user_decision_ref: "chat:2026-05-27:hard-gate-with-deterministic-cache"
    candidate_selection: "from:[advisory-only,hard-gate-with-deterministic-cache] chose:hard-gate-with-deterministic-cache"
    status: proposed
  - id: D7
    user_decision_ref: "chat:2026-05-27:dedicated-capability"
    candidate_selection: "from:[reuse-reviewer,dedicated-capability,fast-only-dedicated] chose:dedicated-capability"
    status: proposed
  - id: D8
    user_decision_ref: "chat:2026-05-27:per-chain-artifacts"
    candidate_selection: "from:[integrate-into-review-json,single-dedicated-artifact,per-chain-artifacts] chose:per-chain-artifacts"
    status: proposed
  - id: D9
    user_decision_ref: "chat:2026-05-27:dedicated-command"
    candidate_selection: "from:[fold-into-phase-commands,dedicated-command] chose:dedicated-command"
    status: proposed
---
# SoT Chain に意味論レビューゲートを追加する

## Context

SoT Chain は ADR → spec.json → `<layer>-types.json` → 実装(コード) のノードがリンクで繋がった構造である。本 ADR が対象とするのはそのうち2つのリンク — **Chain① (spec → ADR)** と **Chain② (catalogue → spec)** — の整合性検証である (catalogue → 実装 のリンクは TDDD の type-signals が別途担うため対象外)。

各ノードは 🔵🟡🔴 信号を持つが、測っている対象は2種類に分かれる。

- **ADR decision の来歴信号 (per-node)** — decision 自身が user 承認 / review 由来を引用しているかを `user_decision_ref` / `review_finding_ref` の有無で評価する。`libs/domain/src/adr_decision/evaluator.rs` の `evaluate_adr_decision` が `DecisionGrounds` (🔵🟡🔴 ＋ grandfathered 免除) を返す。指す先はチャット等の外部承認であって他の SoT ノードではないため、これはノードに内在する信号でリンクの検証ではない。
- **spec / catalogue の grounding 信号 (link)** — spec requirement は `adr_refs` (ADR へのリンク) の有無で信号が決まり (`libs/domain/src/spec.rs` の `evaluate_requirement_signal`)、catalogue entry は `spec_refs` (spec へのリンク) の有無で決まる (`libs/domain/src/tddd/catalogue_spec_signal.rs` の `evaluate_catalogue_entry_signal`)。信号はノードに格納されるが値を決めるのは「外向きの grounding リンクを引用しているか」であり、これが実質 Chain① / Chain② の信号にあたる。

問題は、これらの信号がすべて「引用の有無」しか見ない presence 判定だという点である。🔵 は「ref を引用した」を意味するだけで、引用先が妥当か・意味的に裏付けるかは見ていない。各リンクには本来3つの層が積まれるべきだが、現状は埋まり方が不揃いである。

| 層 | 何を見るか | Chain① (spec → ADR) | Chain② (catalogue → spec) |
|---|---|---|---|
| ① presence 信号 | ref を引用したか | あり (spec requirement 🔵🟡🔴) | あり (catalogue entry 🔵🟡🔴) |
| ② 構造 / 新鮮度 | 引用先が実在し古くないか | **ファイル存在のみ** (anchor 実在・hash なし) | anchor 解決 + hash 新鮮度 |
| ③ 意味論 | 引用先が主張を実際に裏付けるか | **なし** | **なし** |

**層② の穴 (Chain①)** — `libs/infrastructure/src/verify/plan_artifact_refs/mod.rs` の検証器は `adr_refs` について `plan_artifact_refs/spec_refs.rs` の `check_ref_file` でファイル存在のみを確認し、`adr_refs[].anchor` が ADR front-matter の `decisions[].id` に実在するかは検証しない。型も `libs/domain/src/plan_ref/adr_ref.rs` の `AdrRef` が `{ file, anchor }` の2フィールドのみで hash を持たない (対照的に `SpecRef` は `{ file, anchor, hash }`)。一方 Chain② は spec element の per-element SHA-256 (`libs/usecase/src/catalogue_spec_refs.rs` の `SpecElementHashReader`) と catalogue 全体粒度の `catalogue_declaration_hash` (`libs/domain/src/tddd/catalogue_spec_signal.rs` の `CatalogueSpecSignalsDocument`) で新鮮度を持つが、Chain① には新鮮度機構自体がない。

**層③ の穴 (両リンク共通) = 本 ADR の主題** — 層①② をすべて通しても「引用先が主張を意味的に裏付けているか」は誰も確認していない。しかも層② の新鮮度は単独ではゲートにならない: 古くなった瞬間に hash を機械的に再計算して上書きすれば通ってしまい、「変更を読んで整合を確かめた」のか「盲目的に再発行した」のかを hash は区別できない。層③ の意味論検証 (エージェントレビュー) が判定を与えて初めて、presence 信号も新鮮度チェックもゲートとして意味を持つ。新鮮度 hash は廃止されず、「どのリンクを・いつ」再レビューするかのトリガ兼スコープとして層③ に供給される (D4)。

関連 ADR:

- `knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md` — 来歴信号機 (🔵🟡🔴) と decision 個別ライフサイクル (D1-D4)
- `knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` — Chain② (catalogue → spec) の信号機と hash 整合検証
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — SoT Chain のフェーズ分離と plan-artifact-refs 検証器の基本設計 (D2「強制機構: 構造化参照 + 検証 CLI」)

## Decision

### D1: 独立レビューレーンとしての位置づけ — presence 信号と意味論検証を分離する

意味論検証 (「引用先が主張を意味的に裏付けているか」) を presence 信号の色 (🔵🟡🔴) に統合せず、独立した **判定結果レーン** として設計する。

presence 信号 (層①) は「ref を引用したか」を見るにとどまり、意味論検証 (層③) は「引用先が主張を実際に裏付けているか」を見る。両者は別の層であり、層③ を層① の色に混ぜると「青 (= ref 引用済み) なのに意味的に矛盾している」と「健全な青」の区別が消える。

意味論検証は独立したレーンとして設計し、code-review インフラの責務固有部分は流用しない。`Reviewer` port (`ReviewTarget` = ファイル集合)・`Verdict`・`ReviewerFinding` (file/line/severity)・`review.json` (`ScopeRound`)・`ClaudeReviewer` adapter・review-fix-lead は「ファイル群を diff レビューし code を修正する」責務に最適化されており、意味論検証の「reference pair を入力に、引用付き verdict (D6) を返し writer へ差し戻す」形と噛み合わない。相乗りさせると共有型が2責務を抱え分岐が増える SRP アンチパターンになるため、専用の port・型・adapter・`ref-verifier` capability (D7)・`(claim_hash, evidence_hash)` キャッシュ (D4)・Chain ごとの verdict artifact (D8) を独立に持つ。

共有するのは責務中立な部品のみ: `agent-profiles.json` の capability→provider 解決 (`resolve_execution`、D7 が使用) と、commit ゲートで code-review 側 `check-approved` と意味論側 approved を AND する薄い合成。並列ディスパッチは概念として共通だが、実装は code-review 型に結合させず独立に持つ (将来3つ目の検証器が出たら責務中立な core を抽出する)。

参照ペアの並列処理には `agent-profiles.json` の `ref-verifier` 専用 capability を用いる (D7)。

### D2: 層化発火 — phase 第一防衛線 + commit 最終関門

Chain の整合性検証を2段階で発火させる。

- **phase 第一防衛線**: spec-design 後に Chain① (spec → ADR) の意味論レビューを実行し、type-design 後に Chain② (catalogue → spec) の意味論レビューを実行する。局所整合を早期に固めることで、commit 前の全チェックで発覚する矛盾を減らす。
- **commit 最終関門**: 全 Chain の意味論検証を独立した意味論ゲートとして実行し、code-review の `check-approved` と AND して commit を通す (D1/D8 の薄い合成。`/track:review` の scope には相乗りしない)。
- **差分キャッシュによる重複回避**: commit 前の最終関門では、phase 以降に変わったペアのみを再検証する (D4 のキャッシュキー設計で実現)。phase で通過済みの不変ペアは再レビューしない。

### D3: hash 境界と ADR の扱い — ADR は git blob hash のみ、anchor 存在検証は追加する

ADR は構造化ドキュメントではなく設計判断を散文で記述する文書層と位置づける。`spec.json` が構造化データとして管理する情報をここでは扱わない。

この位置づけに基づき、**ADR に canonical hash を作らない**。ADR の同一性は git blob hash に委ねる。

一方で、**anchor の実在検証は追加する**。spec.json の `adr_refs[].anchor` が指す値 (例: `"D1"`, `"D2"`) が ADR front-matter の `decisions[].id` に実際に存在するかを `plan-artifact-refs` 検証器で確認する。この検証は ADR の構造化ではなく「参照先 decision の実在確認」であり、既存の参照新鮮度ゲートと同じ層に収まる。

Chain① の **根拠 (ADR) 側**の再走トリガーは **ADR ファイルの git blob hash 変化**とする (主張 = spec element 側の変化も D4 のキャッシュキーで再走を引く)。blob hash は git の自然なコンテンツ同一性であり、追加の hash 計算は不要。

### D4: 統一キャッシュキー — (主張の hash, 根拠の hash) で判定結果を凍結する

意味論の判定結果を `(claim_hash, evidence_hash)` のペアで凍結し、どちらかの hash が変化したときのみ再レビューを実行する。

| Chain | 主張 (claim) | 主張の hash 種別 | 根拠 (evidence) | 根拠の hash 種別 |
|---|---|---|---|---|
| Chain① (spec → ADR) | spec element | per-element SHA-256 (既存) | ADR ファイル | git blob hash (既存) |
| Chain② (catalogue → spec) | catalogue entry | per-entry SHA-256 **(新規)** | spec element | per-element SHA-256 (既存) |

spec element の per-element hash は `SpecElementHashReader` として既に実装されている (`libs/usecase/src/catalogue_spec_refs.rs`)。SHA-256 計算は `libs/infrastructure/src/verify/plan_artifact_refs/spec_refs.rs` の `canonical_json_sha256` ヘルパーを流用できる。

**新規に追加する hash は catalogue entry の per-entry hash のみ**。現在の `catalogue_declaration_hash` は `<layer>-types.json` 全体粒度であり entry 単位の再評価ができない。per-entry hash を追加することで、変更されたエントリだけを再レビューする差分処理が可能になる。副産物として、既存の hash 整合ゲート (Chain②) の粒度が全体 → entry 単位に精緻化される。

### D5: 段階引き上げ階層 — 軽量モデル → 重量級モデル → 人間

検証コストを抑えつつ信頼性を確保するため、3段の階層で吸収する。

- **段1**: `ref-verifier` capability の fast 解決 (`fast_provider` / `fast_model`) で全ペアを並列レビュー (D7)。
- **段2**: 不合格または判定保留のペア、あるいは既知の誤り例検査 (後述) での劣化検出時に `ref-verifier` capability の final 解決 (`provider` / `model`) へ引き上げ。
- **段3**: 重量級モデルでも解決しないとき、または検証器の劣化が確認されたときに人間へ報告。

**引き上げの単位は参照ペア**: fast 解決全体 → final 解決全体を一括で引き上げるのではなく、**参照ペア単位の判定結果 (不合格または判定保留、あるいは既知の誤り例劣化)** がトリガーとなる参照ペア単位の段階引き上げである。段1で全ペアを評価し、赤になったペアのみを段2に上げる。`agent-profiles.json` の `RoundType::Fast` / `RoundType::Final` 解決機構 (`libs/infrastructure/src/agent_profiles.rs` の `resolve_execution`: Fast は `fast_provider ?? provider` / `fast_model ?? model`、Final は `provider` / `model`) が軽量・重量級のモデル解決を担うが、いつ段2を呼ぶかは ref-verifier 独自のペア駆動ロジックが決める。

**経路分離**: トリガー種別によって段2以降の処理が分岐する。

- 既知の誤り例検査での劣化 (検証器の故障疑い) → 軽量→重量級での検証器点検 → 確認後に人間報告。検証器が期待どおり機能していれば人間には届かない。
- ペアの不合格または判定保留 (結果が赤) → 軽量→重量級で確度確認 → 確定したら writer (spec-designer 等) へ差し戻す (writer が spec.json / catalogue を直し意味論検証が再走する fix→再検証ループ。code-review の review-fix-lead とは別系統)。

**並列の振り分け粒度**: Chain① は (spec element × ADR file) のペア単位、Chain② は layer 単位バッチ (domain / usecase / infrastructure / cli ごと)。バッチ上限とスロットリングの具体数値は設定ファイルで指定可能とする。

### D6: だんまり通過を許さない — 裏付け箇所の引用必須化と既知の誤り例による劣化検出

意味論レビューの非決定性を制御するため2つの仕組みを導入する。

- **裏付け箇所の引用必須化**: 合格判定の際、根拠 (ADR または spec element) のどの箇所 (文や段落) が主張を裏付けるかの引用を必須とする。引用を提示できない合格は判定保留として不合格方向に倒す。これにより「何もチェックせず素通り」を構造的に排除する。
- **既知の誤り例の注入**: 答えが不合格と分かっているペアを各バッチに混ぜて実行し、それらを正しく不合格と判定できているかを監視する。検出率が閾値を下回ったら検証器の劣化と判定し D5 の段階引き上げ経路に入る。既知の誤り例の注入率と検出率閾値は設定ファイルで指定可能とし、運用で調整できる（初期デフォルト: 注入率 10% / 検出率閾値 90%）。

### D7: 専用 capability `ref-verifier` を agent-profiles.json に新設する

意味論レビューの検証器を `reviewer` capability の流用ではなく、`ref-verifier` という専用 capability として `agent-profiles.json` に新設する。

専用 capability とする理由は3点ある。第一に、モデル・タイムアウト・プロンプトを `reviewer` の設定と独立に調整できる。`reviewer` は通常の code review を対象とし `ref-verifier` は参照ペアの意味論整合を対象とする — 求めるモデル特性が異なるため設定を分離する方が適切である。第二に、軽量モデルの選択肢を codex 系に限定されず claude の haiku 相当など任意の provider から選べる。第三に、`reviewer` と `ref-verifier` を同時に並列実行する場合に設定が干渉しない。

**provider をまたいだ段階引き上げのサポート**: `agent-profiles.json` の capability 設定は `fast_provider` / `fast_model` を指定することで Fast round と Final round に異なる provider を割り当てられる。`libs/infrastructure/src/agent_profiles.rs` の `resolve_execution` (行 188-207) が `RoundType::Fast` に対して `fast_provider ?? provider` / `fast_model ?? model` を解決し、`RoundType::Final` には `provider` / `model` を返す。テスト `test_resolve_fast_with_cross_provider` (行 320-348) はこの provider をまたぐ解決が正しく動作することを保証している。`ref-verifier` はこの機構を利用して段1 (Fast) と段2 (Final) に異なる provider を設定できる。

具体的なモデル値は `agent-profiles.json` で指定し、運用時に切り替えられるようにする。初期値はいくつかのモデルでテストして決定する。ADR は段1=軽量モデル / 段2=重量級モデルの役割のみを規定し、具体値は設定に置く（モデルを変更するたびに ADR を触らずに済むため。非 canonical な設定例を ADR 本文に置かない原則とも一致）。

### D8: 判定結果は Chain ごとに別 artifact として収容する

意味論レビューの判定結果を `review.json` に統合せず、Chain ごとに別 artifact として新設・収容する。

- **Chain① (spec → ADR)**: track 単位で1つ。ファイル名 `spec-adr-verify-cache.json`。
- **Chain② (catalogue → spec)**: 層別。ファイル名 `<layer>-catalogue-spec-verify-cache.json` (domain / usecase / infrastructure / cli ごと)。既存の `<layer>-catalogue-spec-signals.json` / `<layer>-type-signals.json` と同じ層別粒度に揃える。

Chain ごとに artifact を分ける理由は3点ある。第一に、発火 phase が違う — Chain① は spec-design 後、Chain② は type-design 後に発火する (D2)。第二に、キャッシュキーの種別が違う — Chain① のキーは `(spec_element_hash, ADR_blob_hash)`、Chain② のキーは `(catalogue_entry_hash, spec_element_hash)` である (D4)。第三に、収容粒度が違う — Chain② は層別、Chain① は track 単位である。性質の異なる判定結果を同一 artifact に混在させると責務が不明確になる。

意味論側の approved は、これらの artifact を読む専用ゲートとして評価する。commit 最終関門 (D2) は、既存 code-review の `check-approved` と意味論側 approved を独立した2つのゲートとして AND する薄い合成であり (既存 `check-approved` を意味論 artifact 読みに拡張しない — D1 の SRP 分離と整合)、これが D2 の具体的な実現形である。

### D9: 意味論検証の呼び出し口 — 専用 `/track:ref-verify` コマンド

意味論検証レーン (D1) は、専用の `/track:ref-verify` skill (公開 UI) と対応する `bin/sotp ref-verify` サブコマンドから呼び出す。`sotp verify *` (層② の構造/新鮮度チェック) にも `/track:review` (code review) にも相乗りせず、層③ 専用の呼び出し口を持つ。3つの検証層がそれぞれ独立した surface を持ち、D1 の SRP 分離と整合する。

**1 コマンドで全 Chain を扱い、スコープは phase に応じて自動で決まる。** 発火点ごとに引数でスコープを切るのではなく、呼び出しコンテキストから対象 Chain を解決する — spec-design 後は Chain①、type-design 後は Chain②、commit 最終関門は両 Chain。スタンドアロン実行 (finding 修正後の再検証・デバッグ) では未確定の全ペアを対象にする。いずれの場合も差分キャッシュ (D4) により、対象のうち変化したペアだけが再レビューされる。

D2 の発火点はこの同じコマンドを呼ぶ — 発火ごとに別実装を持たない。`/track:plan` の phase オーケストレーションが phase 第一防衛線を、commit ゲートが最終関門を、それぞれ `bin/sotp ref-verify` 経由で起動する。`/track:dry-check` がスタンドアロン兼 full-cycle 組み込みであるのと同じパターン。

## Rejected Alternatives

### ADR decision への canonical hash 導入

ADR decision ごとに canonical hash を計算し、`AdrRef` に `hash: ContentHash` を追加する案。

拒否した理由: ADR は設計判断を散文で記述する文書であり、そこへ構造的同一性を強制するのは spec.json が担う構造化を ADR に逆流させることになる。markdown 散文の hash は typo 修正・文言整理などの些細な変更で過敏に古くなり、意味のない更新を強制する。ADR の同一性は git blob hash と anchor 存在確認 + 意味論レビューで代替する (D3)。

### 意味論検証を信号機の色に統合する

🔵🟡🔴 の presence 信号に意味論スコアを加味して色を決める案。

拒否した理由: presence (引用したか) と意味論 (裏付けるか) が混線し、「青 (= ref 引用済み) なのに意味的に矛盾している」と「健全な青」が同じ色で表現されてしまう。信号の情報量が落ちる。両者は別の層であり、意味論は独立した判定結果として持つべき (D1)。

### 参考情報扱い (ゲート化しない)

意味論レビューの結果を参考情報として出力するにとどめ、ゲートに組み込まない案。

拒否した理由: 参考情報は忙しい開発フローの中で無視されがちであり、実質的に素通りになる。`(claim_hash, evidence_hash)` キャッシュで判定結果を凍結することで決定性が確保できるため、ハードゲートとして機能させる (D6)。

### 既知の誤り例検査での劣化検出時に人間直行

検証器の劣化を検知したら即座に人間へ引き上げる案。

拒否した理由: 人間は最もコストの高い検証器であり、より安価な手段 (重量級モデルへの昇格) を尽くす前に呼ぶのは非効率。D5 のモデル階層の漏斗でまず対処する。

## Consequences

### Positive

- 「青 (= ref 引用済み) なのに意味的に矛盾している」という現行では検出不能なケースを捉えられる。
- catalogue entry per-entry hash (D4) の副産物として、既存の hash 整合ゲート (Chain②) の粒度が `<layer>-types.json` 全体 → entry 単位に精緻化され、差分再評価が entry 単位で可能になる。
- phase 第一防衛線 (D2) により局所整合を早期に固めるため、commit 前の重量級 reviewer で発見される矛盾を減らし、review ループが短縮されやすい。

### Negative / コスト

- catalogue entry per-entry hash の新規実装が必要 (D4)。`canonical_json_sha256` ヘルパーの流用で実装コストは低いが、`<layer>-types.json` codec への追加が必要。
- 意味論の判定結果キャッシュ artifact (D8) と検証器 capability 設定 (モデル割り当て) の追加が必要。
- 軽量モデルの非決定性を既知の誤り例検査・裏付け箇所の引用必須化・モデル階層で抑える設計・運用コストがかかる。既知の誤り例ペアのメンテナンスも継続的に必要。

## Related

- `knowledge/adr/2026-04-27-1234-adr-decision-traceability-lifecycle.md` — 来歴信号機 (🔵🟡🔴)、decision 個別ライフサイクルの front-matter フォーマット (D1-D4)
- `knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` — Chain② (catalogue → spec) の hash 整合検証と信号機
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — SoT Chain のフェーズ分離と plan-artifact-refs 検証器の基本設計 (D2「強制機構: 構造化参照 + 検証 CLI」)
- `knowledge/adr/2026-05-23-2236-reviewer-provider-selectable-claude-option.md` — reviewer capability の provider 選択と provider をまたぐ fast/final 解決機構 (D1-D5)
