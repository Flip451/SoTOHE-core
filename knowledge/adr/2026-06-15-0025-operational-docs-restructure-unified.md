---
adr_id: 2026-06-15-0025-operational-docs-restructure-unified
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-10:root-docs-consolidate-into-readme"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-10:delete-developer-ai-workflow"
    candidate_selection: "from:[keep-and-rewrite,delete-and-absorb-into-readme] chose:delete-and-absorb-into-readme"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-15:merge-adr1-adr2-unified-docs-restructure"
    candidate_selection: "from:[keep-as-is,dedupe-in-place,split-to-3-conventions,single-reference-doc] chose:split-to-3-conventions"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-06-10:engineering-rules-to-conventions"
    candidate_selection: "from:[move-and-delete-old-path,move-and-keep-pointer-stub] chose:move-and-delete-old-path"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-06-15:merge-adr1-adr2-unified-docs-restructure"
    candidate_selection: "from:[keep-as-list,move-to-CLAUDE.md,new-00-principles-file,distribute-to-existing-rules-drop-redundant] chose:distribute-to-existing-rules-drop-redundant"
    status: proposed
  - id: D6
    user_decision_ref: "chat:2026-06-10:agents-md-strengthen"
    candidate_selection: "from:[pointerize-to-conventions,strengthen-standalone-briefing] chose:strengthen-standalone-briefing"
    status: proposed
  - id: D7
    user_decision_ref: "chat:2026-06-10:drop-briefing-reference-rule"
    status: proposed
  - id: D8
    user_decision_ref: "chat:2026-06-10:fix-dangling-mechanism-refs"
    status: proposed
  - id: D9
    user_decision_ref: "chat:2026-06-15:merge-adr1-adr2-unified-docs-restructure"
    candidate_selection: "from:[keep-checklist-in-doc,move-checklist-to-rules,drop-checklist-use-cargo-make-help] chose:drop-checklist-use-cargo-make-help"
    status: proposed
  - id: D10
    user_decision_ref: "chat:2026-06-10:right-size-autoloaded-docs"
    status: proposed
---
# 運用ドキュメント再編（統合版）— ルート文書一本化・track/workflow.md 分散・工学規約の conventions 移管

## Context

本リポジトリの運用ドキュメントは、正規フロー（`/adr:add` で ADR を起こし `/track:adr2pr` で PR まで進める）と各 `/track:*` コマンド定義・`knowledge/conventions/` が SoT として確立した後も、それ以前の構成のまま残っている。結果として複数箇所で重複と陳腐化が同時に進み、重複側が古いまま放置される「同期破綻」が観測されている。

本 ADR は、この再編を 2 つの観点（① ルート直下の人間向け文書と工学規約の配置、② `track/workflow.md` の reference 集の解体）をまとめて扱う。両者は編集対象（`DEVELOPER_AI_WORKFLOW.md`・`.claude/rules/`・`CLAUDE.md`・`knowledge/conventions/`）が広範に重なるため、別々に適用すると一方の集約先を他方が削除する前提衝突（例: 一方が `DEVELOPER_AI_WORKFLOW.md` を集約先にし、他方がそれを削除する）が起きる。本 ADR は最終状態を直接記述することで、その衝突を設計時に解消する。

確認された問題:

1. **ルート直下の人間向け文書の重複**: `README.md` / `START_HERE_HUMAN.md` / `LOCAL_DEVELOPMENT.md` / `DEVELOPER_AI_WORKFLOW.md` の 4 つが役割を分け合っているが、内容の大半は `/track:*` コマンド定義・`track/workflow.md`・`.claude/rules/` と重複している。`LOCAL_DEVELOPMENT.md` は廃止済みの Python 実行基盤を前提にした記述を残し、`DEVELOPER_AI_WORKFLOW.md` は正規フロー `/track:adr2pr` に言及せず、手動分解フロー前提の説明・存在しない `docs/` への参照・現行 `Makefile.toml` と乖離した CI コマンド一覧を含む。

2. **`track/workflow.md` の reference 集が重複・陳腐化している**: `track/workflow.md` は track 運用の reference 集として、Guiding Principles・phase 構成・Standard Task Process・Quality Gates チェックリスト・Track Commands リスト・SSoT ライフサイクル・observations.md・Branch Strategy・Git Notes など 11 トピックを抱えるが、その大半が他文書と重複し、かつ重複側が新しい状態になっている。実例として `/track:dry-check` / `/track:pr` / `/track:merge` が `track/workflow.md` のコマンドリストから欠落している。

   | 項目 | 重複先 | 重複先の状態 |
   |---|---|---|
   | phase 構成表 | `DEVELOPER_AI_WORKFLOW.md` | 同等 |
   | Standard Task Process | `DEVELOPER_AI_WORKFLOW.md` | 新しい |
   | Track Commands リスト | `DEVELOPER_AI_WORKFLOW.md` | 新しい（workflow.md 側に欠落あり） |
   | Quality Gates 17 項目 | `Makefile.toml` の `ci-local` / `ci-container` task dependencies（`ci` wrapper から委譲） | 機械可読な真実の源泉 |
   | Guiding Principle 群 | `.claude/rules/04/05/07/08/10` ほか | 既出・詳細はそちらが上 |

3. **工学規約の SoT の向きの逆転**: `knowledge/conventions/README.md` は conventions をプロジェクト固有ルール（設計方針・エラー処理・テスト戦略・命名規則）の正本と宣言しているが、実際にはその中核である Rust 工学規約が `.claude/rules/04-coding-principles.md` / `05-testing.md` / `06-security.md` に置かれ、conventions 側の文書が `.claude/rules/04` を正本として逆参照している。さらに `AGENTS.md`（外部レビュー bot がリポジトリルートから読み込む規約ファイル）に同じ規約の要約が第 3 のコピーとして存在し、3 コピー間に同期の仕組みがない。

4. **存在しない機構への参照**: 削除済みの Python スクリプト群（`scripts/check_layers.py`、`scripts/verify_orchestra_guardrails.py` など）や存在しない `cargo make` タスク（`test-one-exec`）を、`.claude/rules/08/09/10`・`knowledge/conventions/security.md`・`05-testing.md` が現役の手順として案内している。多くの検証実体は `sotp verify` 系（Rust CLI）に移行済みだが、利用者 provider / agent 設定を hard-fail していた `verify-orchestra` は ADR `2026-06-13-0002-codex-orchestrator-settings-addition` により全廃済みなので、該当箇所は verifier への張替えではなく削除または警告文への縮約が必要である。

5. **実装されていない宣言ルール**: `.claude/rules/08-orchestration.md` は「すべての capability briefing は `.claude/rules/04-coding-principles.md` を参照しなければならない」と定めるが、外部 provider 向け briefing はこれを実装しておらず、従っているのは Claude 専用 writer agent 定義のみである。

なお、過去 track の `spec.json` / `impl-plan.json` の多数が `.claude/rules/04-coding-principles.md` 等のパスを convention 参照として引いている。ただし参照整合の検証（`sotp verify plan-artifact-refs` など）は現在のブランチから解決した現行 track のみを対象とし、過去 track の参照はゲートの対象外である。過去の参照先は git 履歴からいつでも復元できる。

## Decision

### D1: ルート直下の人間向け文書を README.md に一本化する

`README.md` を GitHub ランディングページ兼唯一の人間向け入口に位置づけ、SoTOHE の価値説明（SoT Chain・信号機・track モデル）と最小の使い方（前提条件、`/adr:add` で ADR を作り `/track:adr2pr` で PR まで進める）を載せる方向で増強する。`START_HERE_HUMAN.md` と `LOCAL_DEVELOPMENT.md` は削除する。

理由: 正規フローがコマンド定義側に自己完結しており、入口文書を複数維持すると更新漏れによる乖離だけが蓄積する。`LOCAL_DEVELOPMENT.md` の生きている内容（Docker compose 操作）は他文書と重複しており、`START_HERE_HUMAN.md` の固有内容（人間の編集可否区分・承認ポイント）は README の短い節で足りる。

### D2: DEVELOPER_AI_WORKFLOW.md を削除し、固有内容を README.md に吸収する

`DEVELOPER_AI_WORKFLOW.md` を削除する。固有価値が残る部分（前提条件、自由文での依頼例）は README.md の使い方の節に吸収する。重複していた phase 構成・コマンド一覧・標準フローは README.md と各 `/track:*` コマンド定義に集約され、ブランチ運用の節は D3 で新設する `knowledge/conventions/branch-strategy.md` に集約される。本文書を参照している箇所（`.claude/rules/08/09/10`、`README.md`）は D8 / D10 で同時に張り替える。

理由: 本文書は正規フロー `/track:adr2pr` を反映しておらず、説明している手動分解フローは各コマンド定義の重複となっている。内部委譲ルールの節は `.claude/rules/08-orchestration.md` と `knowledge/conventions/dry-check-workflow.md` の重複、CI コマンド一覧は現行 `Makefile.toml` から乖離している。全面書き直しで維持するより、正本（コマンド定義・conventions）への一本化の方が乖離の再発を防げる。

### D3: track/workflow.md を廃止し、Branch Strategy / lifecycle / Git Notes を 3 つの convention に分散する

`track/workflow.md` を削除し、Branch Strategy / SSoT lifecycle / Git Notes の固有内容を以下の 3 ファイルに分散する。Guiding Principles 由来の固有 guidance 2 件は D5 のとおり `.claude/rules/08-orchestration.md` / `.claude/rules/10-guardrails.md` に追記する。重複内容は移送せず削除する。

| 新規 convention | 含める内容 |
|---|---|
| `knowledge/conventions/branch-strategy.md` | Branch Strategy + ガードポリシー |
| `knowledge/conventions/track-lifecycle.md` | plan.md と metadata.json SSoT + observations.md + registry.md 更新ルール |
| `knowledge/conventions/git-notes.md` | Git Notes |

削除する重複内容（集約先は本再編後の最終状態）:

- phase 構成表 → `README.md`（D1/D2）+ 各コマンド定義
- Task Workflow / Standard Task Process → `README.md` + 各 `/track:*` コマンド定義
- Track Commands リスト → `README.md` + 各コマンド定義
- Mermaid Diagram Convention → 1 例のみで convention 化に足りないため削除

`track/workflow.md` を参照する箇所の張替え:

| 参照元 | 張替え先 |
|---|---|
| `.claude/commands/track/setup.md`（Read 操作） | 新 3 convention を Read 対象に列挙 |
| `.claude/rules/08-orchestration.md`（Source Of Truth） | 新 3 convention を SoT リストに列挙 |
| `.claude/rules/08-orchestration.md`（Operational split） | `track/workflow.md` 行を削除し、conventions が「day-to-day workflow rules」を持つことを明記 |
| `.claude/rules/09-maintainer-checklist.md` | 新 3 convention に置換 |
| `.claude/rules/10-guardrails.md`（Operational details） | `track/workflow.md` / `DEVELOPER_AI_WORKFLOW.md` 行を削除し、新 3 convention + README に張替え（D8 と整合） |
| `CLAUDE.md`（priority references） | `track/workflow.md` 行を削除し、conventions 索引へ統合（D10 と整合） |

`knowledge/conventions/README.md` の convention 索引は `bin/sotp conventions update-index` で再生成する。

理由: 正本の所在が convention 名で自明になる／重複削除で同期 burden が消える／後続の Branch Strategy 可変化 ADR の編集対象が `branch-strategy.md` 1 ファイルに絞れる／既存 convention 群と粒度が揃う。

### D4: provider に依存しない工学規約を knowledge/conventions/ に移管し、旧ファイルは削除する

`.claude/rules/04-coding-principles.md` / `05-testing.md` / `06-security.md` の内容を `knowledge/conventions/` 配下の正式 convention に移管し、旧ファイルは削除する（ポインタ stub も残さない）。`06-security.md` のコード実装パターンは既存の `knowledge/conventions/security.md` に統合する。`04` / `05` の移管先 convention の分割は既存の conventions 構成に合わせて決める。`.claude/rules/` は以後、Claude Code 固有の運用規則（orchestration、permission/hook ガードレール、開発環境コマンド、言語運用）だけを置く場所とする。

理由: conventions の守備範囲宣言と実際の配置を一致させ、「conventions が正本、他はすべて参照」に参照方向を揃える。規約変更時の編集先が一箇所になり、3 コピー状態が解消される。旧パスへの後方互換は不要とする — 過去 track の参照は歴史的記録であり、参照整合の検証は現行 track のみが対象でゲートに影響せず、過去の内容は git 履歴から復元できる。

### D5: Guiding Principles 11 項目を分散し、冗長な項目は削除する

`track/workflow.md` の Guiding Principles 11 項目のうち 9 項目は他箇所で既に enforce 済みの restatement なので削除し、残り 2 項目を既存 rules に 1 行ずつ追記する。`00-principles.md` は新設しない。

| # | 原則 | 扱い |
|---|---|---|
| 1 | 仕様が真実の源泉 | 削除（Phase 1 workflow + `08-orchestration.md` SoT 節が enforce） |
| 2 | 型が嘘をつかない | 削除（Phase 2 + 移管後の coding-principles convention が enforce） |
| 3 | テスト駆動 | 削除（移管後の testing convention と重複） |
| 4 | Tech Stack 厳守 | 削除（`verify-tech-stack` が機械検証） |
| 5 | Context 効率 | 削除（`08-orchestration.md` delegation rules と重複） |
| 6 | CI グリーン | 削除（`07-dev-environment.md` + `10-guardrails.md` + コミットゲートが機械強制） |
| 7 | No Panics in Production | 削除（移管後の coding-principles convention に詳細あり） |
| 8 | Rust Edition 2024 | 削除（`Cargo.toml` の `edition` が機械強制） |
| 9 | Layer 強制 | 削除（`check-layers` が機械検証、`CLAUDE.md` と重複） |
| 10 | 自己修復優先（3 回詰まったら researcher で原因切り分け） | `.claude/rules/08-orchestration.md` の "If unsure" 末尾に 1 行追記 |
| 11 | レビューサーフェース最小化（タスク単位レビュー、O(N²) 根拠） | `.claude/rules/10-guardrails.md` の "Small task commits" 項目に O(N²) 根拠を 1 行追記 |

理由: 1-9 を残すと正本が二重化する（詳細は他箇所にあるのに簡易版を再宣言すると根拠が曖昧になる）。10・11 は他箇所にない固有 guidance なので 1 行ずつ追記して保存する。list 自体を残すと項目内容が他箇所と乖離した時の同期 burden が新たに発生する。

### D6: AGENTS.md は PR レビュー専用 briefing として強化する

`AGENTS.md` はリポジトリルートに残し、conventions へのポインタ化はしない。severity policy（P0/P1 のみ報告する方針）を維持したうえで、ローカルレビュワーが取りこぼしうるレビュー観点（ブランチ全体を通した整合性、複数コミットにまたがる変更の一貫性など、PR 単位でしか見えない観点）をすべて盛り込む方向で内容を強化する。

理由: `AGENTS.md` は外部レビュー bot が読み込む独立した文脈であり、参照を辿って適用される保証がないため、ポインタ化はレビュー品質を下げる方向に働く。PR レビューはローカルレビューの後段に置かれた最終関門なので、正本との重複を避けることよりも、ローカルで拾いにくい観点を確実に網羅する自己完結 briefing であることを優先する。conventions との重複は読み手が異なる意図的なコピーとして許容し、規約変更時に手動で同期する。

### D7: 「全 capability briefing は coding-principles を参照せよ」ルールを廃止する

`.claude/rules/08-orchestration.md` の Briefing Requirements 節を削除する。

理由: このルールは外部 provider 向け briefing 生成に実装されておらず、文書上の宣言だけが残っている。規約の適用は briefing の文言ではなく、writer agent 定義の必読指定とレビューゲートで担保されており、実態のない must 宣言は守られているという誤認だけを生む。

### D8: 存在しない機構・削除済み文書への参照を排除し、記述を現行実装に揃える

運用文書から、削除済み機構への参照と実行すると失敗する手順案内を排除する。確認済みの対象に加え、本再編で新たに生じる張替えを含める。

- `.claude/rules/08-orchestration.md`: `scripts/check_layers.py` → 検証の実体は `sotp verify layers`
- `.claude/rules/09-maintainer-checklist.md`: 「Docker 内に python3 が必要」とする前提 → Python 実行基盤は全廃済み
- `.claude/rules/10-guardrails.md`: `scripts/verify_orchestra_guardrails.py` → `verify-orchestra` は全廃済みなので verifier 参照を削除し、危険な permission 設定は `knowledge/conventions/responsibility-boundary.md` の provide-not-enforce 方針に沿って docs の警告として残す
- 移管後の security convention: `EXPECTED_DENY` の追加先・回帰テストの追加先が削除済みスクリプト / `verify-orchestra` を指す保守手順 → verifier 追加手順を削除し、利用者設定の強制ではなく危険例の説明に縮約する
- 移管後の testing convention: `cargo make test-one-exec` → 現行 `Makefile.toml` に存在しないため、実在するテスト実行手段に差し替える
- 本再編で削除する `DEVELOPER_AI_WORKFLOW.md` への参照（`.claude/rules/08/09/10` ほか）→ `README.md` / 新 3 convention に張替え
- 本再編で新設する 3 convention（`branch-strategy.md` / `track-lifecycle.md` / `git-notes.md`）も、`track/workflow.md` から引き継いだ dead-ref（旧 `cargo make` タスク名等）がないかスキャン対象に含める

理由: 文書が案内する手順が実行不能であることは、文書全体への信頼を毀損し、誤った保守作業を誘発する。本再編で文書の削除・新設が起きるため、スキャン対象は再編後の状態を基準に再算出する。

### D9: Quality Gates チェックリストは doc 化せず、Makefile.toml の ci-local / ci-container task dependencies を真実の源泉とする

`track/workflow.md` の Quality Gates チェックリストは（D3 の workflow.md 廃止に伴い）削除し、doc 化しない。代替として:

- `Makefile.toml` の `ci-local` / `ci-container` task の `dependencies` を機械可読な真実の源泉とする（`ci` wrapper は docker compose 経由で `ci-local` に委譲）
- `cargo make help` がカテゴリ表示で内訳を出す（`.claude/rules/07-dev-environment.md` で紹介済み）
- 必要なら `Makefile.toml` のタスクに `category` メタを整備して `cargo make help` の出力を読みやすくする

理由: doc 化したチェックリストは `ci-local` / `ci-container` task 更新時の手動同期 burden を生む。`Makefile.toml` の `dependencies` は実行時に必ず参照され機械可読なので、こちらを真実の源泉とした方が乖離が起きない。

### D10: 自動ロード文書の情報量を見直す

`CLAUDE.md` は自動ロードされる索引として残し、本再編（削除・移管・新設）に合わせて参照一覧を更新する（`DEVELOPER_AI_WORKFLOW.md` / `track/workflow.md` 行の削除、移管後の conventions と新設 3 convention の反映、`.claude/rules` を 01 / 07 / 08 / 09 / 10 に限定）。残置する `.claude/rules`（01 / 07 / 08 / 09 / 10）も同じ基準で精査し、索引・運用規則として必要十分な情報量に直す（重複説明の削減、現行コマンドとの突合）。

理由: 自動ロードされる文書は全セッションの文脈を常時占有するため、過剰な記述はコストであり、不足・乖離は誤動作の原因になる。再編後の参照構造（README = 人間向け入口、conventions = 工学規約の正本、`.claude/rules` = Claude Code 固有運用、コマンド定義 = フローの正本）を索引が正しく映している状態を保つ。

## Rejected Alternatives

### A. track/workflow.md をそのまま維持する

重複と古い情報のドリフトが進行する。現に `/track:dry-check` / `/track:pr` / `/track:merge` が `track/workflow.md` のコマンドリストから欠落している事例が発生済み。

### B. 重複だけ削除して reference 集として track/workflow.md に残す

`track/` 直下に reference 集を置く意義がない。`track/` 配下は track 運用文書（`tech-stack.md`、生成 `registry.md`）のみとし、convention は `knowledge/conventions/` に集約する方が責務分離が明確になる。

### C. workflow.md の固有内容を 1 つの track-workflow-reference.md にまとめて移送する

3 トピック（branch-strategy / lifecycle / git-notes）が独立しているため、それぞれ単独で読める方が探索コストが低い。既存 convention 群が topical に分かれているのと粒度が揃う。

### D. DEVELOPER_AI_WORKFLOW.md を全面書き直して維持する

正規フロー反映と重複削除を毎回手動で行う負担が残り、乖離が再発する。正本（コマンド定義・conventions）への一本化の方が安定する。

### E. 工学規約をポインタ stub を残して移管する

stub も同期対象になり 3 コピー問題が完全には解消しない。旧パスは過去 track の歴史的参照として git 履歴に残るので stub は不要。

### F. AGENTS.md を conventions へのポインタ化する

外部 bot が参照を辿って適用する保証がなく、レビュー品質を下げる。自己完結 briefing を優先する。

### G. Guiding Principles 11 項目を 00-principles.md に集約する

9 項目が他 rules と重複し正本が二重化する。残り 2 項目を既存 rules に吸収する方が情報密度が高く同期も不要。

### H. Quality Gates チェックリストを .claude/rules/ 配下に移送する

同期 burden の置き場所が変わるだけで、`Makefile.toml` との同期問題は解消しない。

## Consequences

### Positive

- 人間向け入口が `README.md` 1 つになり、工学規約の正本が `knowledge/conventions/` に一本化される
- 正本の所在が convention 名（`branch-strategy` / `track-lifecycle` / `git-notes` / 移管後の coding-principles・testing・security）で自明になる
- `DEVELOPER_AI_WORKFLOW.md` / `track/workflow.md` との同期忘れによるドリフトが消える
- 自動ロード文書（`CLAUDE.md`・残置 `.claude/rules`）の情報量が減り、セッション文脈コストが下がる
- Branch Strategy 可変化を扱う後続 ADR の編集対象が `branch-strategy.md` 1 ファイルに絞れる
- 存在しない機構への手順案内が消え、文書の信頼性が回復する

### Negative

- 参照張替えが広範（root docs / `.claude/rules/08/09/10` / `CLAUDE.md` / 移管後 conventions / 新設 3 convention）に及ぶ。機械的だが量がある
- 移管後の工学規約は Claude Code セッションに自動ロードされなくなる。writer / implementer agent 定義の必読パス指定を移管先に更新して参照経路を維持する必要がある
- 旧 `.claude/rules/04/05/06` を引いている過去 track の参照はリンク切れになる（歴史的記録として残す。検証は現行 track のみ対象でゲートに影響しない）
- `AGENTS.md` と conventions の意図的な重複は、規約変更時に手動同期が必要
- `LOCAL_DEVELOPMENT.md` のトラブルシュート記述（Docker 失敗時の対処など）は削除により失われる（git 履歴から復元可能。再び必要になった時点で convention として正式化）
- 移行直後は古いパス（`track/workflow.md` / `DEVELOPER_AI_WORKFLOW.md`）を覚えている開発者・AI セッションが一時的に混乱しうる（`CLAUDE.md` と `.claude/rules` の更新で誘導可能）

### Neutral

- `knowledge/conventions/` のファイル数が増える（新設 3 + 移管 04/05 分）。索引は `bin/sotp conventions update-index` で再生成する

## Reassess When

- `README.md` が肥大化し、人間向け入口として分割が必要になったとき
- `knowledge/conventions/` のファイル数が増えて索引の探索性が問題化したとき
- Branch Strategy 可変化 ADR の実装で `branch-strategy.md` を config-driven 記述に書き直す必要が出たとき
- 外部レビュー bot の入力規約が変わり、`AGENTS.md` の自己完結方針を見直す必要が出たとき
- 残置した `.claude/rules`（01 / 07 / 08 / 09 / 10）にさらなる重複・乖離が生じたとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-04-27-0554-doc-reorganization.md` — 運用ドキュメント断捨離方針（本再編の先行 ADR）
- `knowledge/adr/2026-06-03-1327-python-runtime-full-removal.md` — D8 の背景（Python 実行基盤の全廃）
- `knowledge/conventions/README.md` — conventions の守備範囲宣言・索引（移管・新設に伴う再生成対象）
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR の配置・参照スタイル
- `knowledge/conventions/workflow-ceremony-minimization.md` — 実効性のない宣言を廃止する原則（D7 の判断基準）
- `knowledge/conventions/responsibility-boundary.md` — provider / agent 設定は provide-not-enforce とし、`verify-orchestra` を持たない方針（D8）
- `knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md` — `verify-orchestra` 全廃の先行判断（D8）
- `knowledge/conventions/security.md` — `06-security.md` のコード実装パターンの統合先（D4）
- `knowledge/conventions/dry-check-workflow.md` — `DEVELOPER_AI_WORKFLOW.md` 内部委譲ルールの正本（D2）
- `Makefile.toml` — Quality Gates の真実の源泉（D9、`ci-local` / `ci-container` task dependencies）
