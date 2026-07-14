---
adr_id: 2026-07-13-0818-public-template-blocker-cleanup
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-public-template-blocker-cleanup:2026-07-09"
    candidate_selection: "from:[gitignore-aware-export,clean-checkout-only,manifest-exact-excludes] chose:gitignore-aware-export"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-public-template-blocker-cleanup:2026-07-09"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-public-template-blocker-cleanup:2026-07-09 + chat_segment:session-01GAmXz1CoicAsxZEFVrmW9H:2026-07-13 参照解消を自己完結化に一本化（選別同梱・overlay 差し替えの不採用）"
    candidate_selection: "from:[self-contained-conventions,selective-adr-bundling,overlay-convention-swap,bundle-full-adr-track-history,drop-conventions-harness-docs] chose:self-contained-conventions"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add-public-template-blocker-cleanup:2026-07-09 + chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 archive 完了状態を directory 位置から導出し、metadata status field を持たない"
    candidate_selection: "from:[cli-workflow-ssot-delegation,keep-manual-procedure,directory-derived-archive-state,metadata-status-archive-state] chose:[cli-workflow-ssot-delegation,directory-derived-archive-state]"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:adr-add-public-template-blocker-cleanup:2026-07-09"
    candidate_selection: "from:[rewrite-all-legacy-artifacts,exported-template-scan-only] chose:rewrite-all-legacy-artifacts"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-01GAmXz1CoicAsxZEFVrmW9H:2026-07-10"
    candidate_selection: "from:[verify-gate-plus-typed-paths,writer-fix-and-manual-check-only] chose:verify-gate-plus-typed-paths"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13"
    candidate_selection: "from:[name-key-fail-closed-gate,review-policy-only,defer-gate] chose:name-key-fail-closed-gate"
    status: proposed
---
# 公開テンプレート配布前の阻害要因解消

## Context

SoTOHE は、開発リポジトリそのものを利用者に clone させる方式ではなく、境界 manifest と overlay から汎用テンプレートを export する方式へ移行済みである。export は実行中の `sotp` バイナリを出力先の `bin/sotp` に同梱し、初回導入を固定 tag 非依存にする。これにより、公開テンプレートとして配布するための主要な機構は揃っている。

一方で、公開前確認で次の残課題が見つかった。

1. Git 管理下の file だけから作ったきれいな作業ツリーでは `sotp template export` が成功するが、通常の開発作業ツリーでは `.fastembed_cache/` や `.semantic_index*` のような `gitignore` 対象の一時生成物が存在し、template export smoke が manifest 未分類 path として fail-closed する。これは配布物の内容欠陥ではないが、公開前検査としては脆い。
2. 初回導入は実行中 `sotp` の同梱で tag 非依存になったが、更新・他ホスト再導入用の固定 tag 経路は残っている。公開 repository に設定済み tag が存在しない場合、利用者がその経路を踏んだ時点で失敗する。
3. テンプレートとして出荷する対象に含まれる harness / command / convention 文書は、利用者が新しく取得した直後の作業ツリーに届く。そこから、export されない具体 track、元リポジトリの具体 ADR file、`gitignore` 対象の生成物などを現行前提として参照すると、利用者環境では存在しない参照になる。
4. `/track:archive` の Claude 用 command 文書は、CLI に `sotp track archive` が存在するにもかかわらず、metadata 直接編集と `git mv` を長文手順として持っている。これは提供元別の command 文書と workflow SSoT の責務分離、および `git` 直接操作を CLI wrapper に集約する方針とずれている。
5. 過去の track 成果物や review 成果物には作業機の絶対パスが含まれる。exported template では `track/items/` と `track/archive/` は除外され、`knowledge/research/` も overlay されるため、出力されたテンプレートへの直接混入は避けられる。しかし SoTOHE-core を公開テンプレートの元リポジトリとして公開する以上、旧 track 成果物に残る絶対パスも利用者・確認者の目に触れる。したがって、公開前に旧 track 成果物の絶対パスも全件書き換える。

本 ADR は、公開テンプレート配布を妨げる範囲と、元リポジトリ公開時の漏えい防止範囲を分離しつつ、公開前に同じ阻害要因解消として潰すべき作業の境界を定める。

## Decision

### D1: template export は `gitignore` 対象の一時生成物を配布内容外として扱う

`sotp template export` と template export smoke は、Git 管理下の file または `gitignore` 対象ではない file の未分類 path には引き続き fail-closed する。一方で、`.fastembed_cache/`、`.semantic_index*`、ローカルのロック、ローカルのビルド・一時保存 directory など、`gitignore` 対象かつ Git 管理外の一時生成物は配布内容外として扱い、export 成否を左右させない。

実装は、exporter が `gitignore` 判定を使って Git 管理外かつ無視対象の生成物を skip する方式を第一候補とする。smoke gate 側で Git 管理下の file だけから作った作業ツリーを export する方式は補助的には有効だが、通常の開発作業ツリーで利用者が export を実行する導線を隠すだけになるため、根本対策とはしない。

この判断は境界 manifest の fail-closed 性を弱めるものではない。分類漏れを許す対象は、Git が配布対象として扱わない一時生成物に限る。Git 管理下の file、無視対象ではない生成 file、または作業ツリーに新しく追加された未分類 file は引き続き阻害要因として失敗させる。

### D2: 固定 tag 経路は公開前の約束事として検証する

実行中の `sotp` を export 結果に同梱することにより、初回導入は固定 tag 非依存にする。一方、`.harness/config/sotp-version.json` に残る固定 tag 経路は、更新・他ホスト再導入用の公開上の約束事として扱う。

公開前には、設定された `git_url` と `tag` が公開 remote 上で解決できることを公開前確認一覧または verify task で確認する。tag が未作成の場合は、公開前に tag を作成・push するか、設定を実在する公開 tag に更新する。

template export smoke は初回導入導線を検証するため、固定 tag の存在を必須にしない。ただし、README や bootstrap が更新・他ホスト導入経路を案内する以上、その経路の参照先が存在しない状態では公開しない。

### D3: 出荷対象から新規取得直後に存在しない具体参照を排除する

境界 manifest の `include` および `overlay` 分類から導出される最終 export 全体を、テンプレートの出荷対象として扱う。具体参照の排除は、そこに含まれる参照を記述し得るすべての file に適用する。`.claude/**`、`.harness/**`、`.codex/**`、`.agents/**`、`knowledge/conventions/**`、`README.md`、`CLAUDE.md`、`AGENTS.md`、`Makefile.toml` はその代表例であり、この列挙自体は出荷対象の境界を定義しない。出荷対象の file から、利用者が新しく取得した直後の作業ツリーに存在しない具体 path を現行前提として参照しない。

禁止対象は、具体 track id を含む `track/items/<some-real-track>/...`、元リポジトリの具体 ADR file、`tmp/...` の永続参照、`target/...` や `.semantic_index/...` を存在前提にした説明、削除済み file への参照である。参照先文書を特定できない符号（文書名を伴わない decision 符号や、track 固有の制約符号）による引用もこれに含める。

例外は、利用者が workflow 実行時に作る placeholder path、実行時に作られる tree の汎用 path、`gitignore` 対象のローカル設定、ビルド生成物や一時保存物の標準実行時 path としての説明に限る。元リポジトリの歴史的背景を残したい場合は、出荷対象ではなく ADR 側または commit message 側に残す。

具体 ADR file への参照、および参照先文書を特定できない符号は、convention 本文の自己完結化で解消する。規則の実行に必要な挙動・条件・例を convention 本文で完結させ、出典表示・supersession 来歴・決定符号への依存を外す。決定の来歴は ADR 側の front-matter と本文が保持し続けるため、この書き換えで規則側の情報は失われない。選別済み ADR の同梱と、export 用 overlay による convention の差し替えは、参照解消の手段として採用しない。

### D4: `/track:archive` は CLI と workflow SSoT に委譲する

`/track:archive` の提供元別 command 文書は、metadata 直接編集、手動 `git mv`、手動 stage file 作成の長文手順を保持しない。archive の業務ロジックは CLI (`sotp track archive`) と提供元非依存の workflow SSoT に集約する。

公開前に、archive workflow を `.harness/workflows/track/` 側へ置くか、既存 workflow に archive の責務を統合する。その上で `.claude/commands/track/archive.md` は、workflow SSoT を冒頭で指し、Claude Code 固有の起動形態・報告形式だけを残す薄い接続文書にする。

archive 完了状態の唯一の SSoT は directory 位置とする。track directory が `track/archive/` 配下に存在すること自体を archived 状態とし、metadata に status field を追加せず、archive 時にも metadata status を更新しない。registry などの rendered view における Archived 表示も同じ directory 位置から導出する。

この方式は人工的な状態 field を持たない方針と現行 CLI の directory 移動に整合し、directory 位置と metadata の二重管理による drift を生じさせない。これにより、`git` 直接操作を手順書側へ漏らさず、directory 移動 / rendered view 更新 / `gitignore` 対象 logs の保持といった詳細を CLI と workflow SSoT 側で一元管理する。

### D5: 旧成果物内の絶対パスも公開前に全件書き換える

Git 管理下の成果物に残る作業機の絶対パスは、公開前に全件書き換える。対象は directory 列挙で絞り込まず、混入が確認されている `track/items/**`、`track/archive/**`、track/review 成果物、`knowledge/research/` の research note はその代表例にすぎない。workspace 内を指す絶対パスは repo-relative path に正規化する。workspace 外の一時領域・一時保存領域・host 固有 path は、意味を保つ必要がある場合のみ汎用表記に置き換え、意味を持たない診断情報なら削除または伏せ字化する。

この修正は exported template への混入対策ではなく、SoTOHE-core の元リポジトリを公開するための漏えい防止確認である。ただし公開作業としては template 公開前の阻害要因解消と同じ完了条件に含める。

同時に、今後の成果物を書き込む処理は repo-relative path を保存するように修正する。既存成果物の一括書き換えだけで終えると、次の review / track 実行で絶対パスが再混入するためである。

template 公開の完了条件は、「exported template に絶対パスが含まれないこと」に加えて、「公開元リポジトリの Git 管理下の成果物に作業機の絶対パスが残っていないこと」とする。

### D6: 絶対パスの混入を機械検査ゲートで検出する

D5 の完了条件を手動確認に留めず、機械検査に接続する。検査面は 2 つ。

1. 公開元リポジトリ側: Git 管理下の全 file に作業機の絶対パスが含まれていないことを検査する verify ゲートを追加し、CI に接続する。対象は directory 列挙で絞り込まない。過去に混入が確認された `track/items/**`、`track/archive/**`、`knowledge/research/**` はその代表例にすぎない。検出の最低対象は home directory 配下を指す絶対パスとする。agent が自由記述で書く markdown への混入もこの走査で検出する。ゲートは fail-closed とし、D5 の一括書き換え完了後に有効化する。
2. exported template 側: template export smoke に、export 出力へ作業機の絶対パスが含まれていないことの走査を追加する。

補助として、path を永続化する構造化成果物の codec 境界には、repo-relative であることを構築時に強制する型を導入する。型による強制は構造化データ側の再混入を塞ぐが、自由記述には効かないため、走査ゲートと併用する。

### D7: 出荷対象への具体参照の再混入を名前キー検査で防ぐ

D3 の一括整理後に、同じ出荷対象集合へ存在しない具体参照が再混入することを防ぐ fail-closed の verify ゲートを有効化し、CI に接続する。検査対象は独自の directory 列挙を持たず、境界 manifest の分類から D3 の出荷対象集合を導出する。

機械検査は、参照の字面だけで具体性を判定できる名前キーを走査する。必須の検出対象は、`knowledge/adr/` 配下の日付時刻型 file 名を伴う具体 ADR 参照と、`track/items/` 配下の日付 suffix 付き track id を伴う具体 track 参照とする。`knowledge/adr/` directory および `knowledge/adr/README.md` への汎用参照と、`<id>` など角括弧形式の placeholder は許容する。文書名を伴わない `CN-` + 数字型の裸の制約符号は、誤検知せず識別できる条件が固まった時点で同じ名前キー検査へ追加する拡張候補とする。正確な検出 pattern と実装形式は、この分類を満たす範囲で実装時に定める。

具体 ADR file への link を要求する規定は、D3 の自己完結化と同時に、`knowledge/adr/README.md` への汎用 link のみを要求する規定へ揃える。これにより、参照を生む側の規定と検査ゲートを矛盾させない。

字面だけでは正当な実行時説明と区別できない参照は、このゲートへ含めない。`tmp/`、`target/`、`.semantic_index/` などを存在前提にした説明、削除済み file への参照、裸の符号の意味論上の不備は、既存の出荷対象 review policy が文脈を読んで検出する。機械検査と意味論 review の 2 層を維持し、機械検査の対象拡大だけで review lane を置き換えない。

## Rejected Alternatives

### A. きれいな取得直後の作業ツリーだけで export する運用にする

公開作業者が常に Git 管理下の file だけから作った作業ツリーで export すれば smoke failure は避けられる。しかし、通常の開発作業ツリーで `sotp template export` を実行する導線は残り、`gitignore` 対象の一時保存物があるだけで失敗する問題は再発する。配布対象外の生成物を配布対象外として扱う実装に寄せるため却下。

### B. 境界 manifest に実行時一時保存物の個別 exclude を列挙する

`.fastembed_cache` や既知の `.semantic_index*` を個別 entry として追加すれば、現在見えている失敗は解消できる。しかし実行時生成物は今後も増減し、manifest が `gitignore` の劣化コピーになる。境界 manifest は配布分類の SSoT として維持し、実行時生成物の無視は `gitignore` 判定に委ねる方が責務が明確なため却下。

### C. 元リポジトリの全 ADR / track 履歴を template に同梱する

具体 ADR 参照や track 参照の存在しない参照は消えるが、利用者に不要な元リポジトリの歴史、実装過程、review 成果物、作業機固有情報を出荷することになる。template の目的は再利用可能な harness と雛形 workspace の提供であり、元リポジトリの開発履歴配布ではないため却下。

### D. convention / harness docs を template から外す

存在しない参照の発生面は小さくなるが、SoTOHE の運用ルール、capability routing、review policy、workflow の約束事が利用者に届かなくなる。テンプレートとしての価値を失うため却下。

### E. `/track:archive` の手動手順を残す

手順書だけで archive 操作を説明し続けると、CLI と実際の workflow が乖離する。`git` 直接操作の扱いも接続文書ごとに分散する。archive は CLI と workflow SSoT に寄せるため却下。

### F. exported template だけを絶対パス検査の対象にする

exported template には `track/items/**` が含まれないため、この範囲だけを検査すれば出力されたテンプレートへの直接漏えいは防げる。しかし公開元リポジトリに旧 track 成果物が残る以上、作業機の絶対パスは公開対象に残る。出力されたテンプレートだけを見て公開可とするのは公開作業の実態に合わないため却下。

### G. 絶対パス対策を書き込み側修正と公開前の手動確認だけに留める

書き込み側の repo-relative 化で構造化データの再混入は減るが、agent が自由記述で書く成果物には効かない。公開前の手動確認は繰り返し作業で形骸化しやすく、混入に気づくのが公開時点まで遅れる。検出を CI 時点へ前倒しするため却下。

### H. 選別済み ADR を同梱して convention の具体参照を残す

参照を生かしたまま export できるが、利用者に不要な決定記録を出荷し、参照の増減に合わせて同梱リストの選別を維持し続ける必要が生じる。棚卸しの結果、出荷対象の ADR 参照はいずれも規則本文が既に自己完結している出典表示であり、同梱で守るべき意味論がないことが確認された。自己完結化に一本化するため却下。

### I. export 用 overlay で convention を template 向けに差し替える

元リポジトリ版と template 版の同一 convention を二重に維持することになり、規則を更新するたびに両版の同期が必要になって drift する。自己完結化なら 1 版のまま元リポジトリと template の両方で通用するため却下。

### J. 具体参照の再混入検出を review policy だけに任せる

意味論依存の参照は review が必要だが、具体 ADR file 名や日付 suffix 付き track id は字面だけで安定して判定できる。機械判定できる違反まで review の注意力に依存すると、再混入の検出が review 時まで遅れ、見落としも防げないため却下。

### K. D3 の一括整理だけを行い、機械検査は必要性が実証されるまで保留する

一括整理は現在の違反を解消するが、同じ参照形式を再び記述する経路を閉じない。検出対象を強い名前キーに限定すれば、意味論 review を維持したまま低い曖昧性で再混入を防げるため、ゲート自体の保留は却下。

### L. metadata の `status: archived` を archive 完了状態の SSoT にする

metadata status を canonical にすると、archive 時に directory 移動と status 更新の両方が必要になり、一方だけが更新された場合に状態が drift する。archive 済み track は directory 位置だけで識別でき、人工的な状態 field を追加する必要がないため却下。

## Consequences

### Positive

- template export smoke が通常の開発作業ツリーでも信頼できる検査になり、`gitignore` 対象の一時保存物の有無に左右されにくくなる。
- 初回導入、更新、他ホスト再導入の各導線がそれぞれ明確な約束事を持つ。
- 出荷対象が利用者の新規取得直後の作業ツリーで自己完結し、利用者が存在しない元リポジトリ履歴を追わされなくなる。
- `/track:archive` の操作責務が CLI / workflow SSoT に集約され、提供元別 command 文書の乖離が減る。
- archive 完了状態と registry の Archived 表示が directory 位置という単一の SSoT に揃い、metadata との二重管理による drift が生じない。
- exported template と公開元リポジトリの両方から作業機の絶対パス漏えいを除去できる。
- 絶対パスの再混入が公開直前の確認ではなく CI 時点で検出される。
- 具体 ADR file 参照と具体 track 参照の再混入が CI 時点で fail-closed に検出される。
- 出荷される convention が元リポジトリと template で同一の 1 版に保たれ、選別同梱や overlay 差し替えの維持管理が生じない。

### Negative

- exporter は `gitignore` / Git 管理状態の判定を扱う必要があり、単純に manifest だけを見て作業ツリーを走査するより実装が複雑になる。
- convention 内の具体 ADR 参照と文書内符号を平文へ展開する書き換え作業が発生する。
- archive workflow SSoT の追加または再編が必要になる。
- registry など archive 状態を表示する reader は、metadata status ではなく directory 位置から状態を導出するよう統一する必要がある。
- 旧 track 成果物の一括書き換えと、書き込み側の repo-relative 化が必要になり、変更量が増える。
- 書き換えにより過去成果物の診断情報の細部が一部失われる可能性がある。
- 絶対パス検査の verify ゲートと smoke 側走査の実装、および誤検知時の検査パターン調整が必要になる。
- 出荷対象の名前キー検査を実装し、境界 manifest の分類変更と検出対象を同期する必要がある。

### Neutral

- 実行中 `sotp` の同梱を初回導入の主経路とする方針は変えない。
- 境界 manifest の fail-closed 方針は維持する。
- 字面で判定できない参照漏れは、引き続き出荷対象 review policy が検査する。
- maintainer repo 限定の退行検査は、exported template の CI に接続しない限り残してよい。

## Reassess When

- Git 以外の配布経路で template export を実行する必要が出て、`gitignore` / Git 管理状態の判定に依存できなくなったとき。
- 利用者が元リポジトリの具体 ADR 履歴を参照したいという要求を持ち、選別済み ADR 同梱の必要性が実証されたとき。
- 複数 OS / 複数 architecture 向けの事前ビルド済みバイナリ配布を開始し、実行中 `sotp` の同梱と固定 tag からの導入の位置づけを再設計する必要が出たとき。
- 絶対パスを含む成果物が再発し、書き込み側の正規化境界を追加で見直す必要が出たとき。
- 具体参照の命名形式が変わり、名前キー検査の検出対象または許容 placeholder を見直す必要が出たとき。
- 意味論 review に残した参照漏れのうち、字面だけで低誤検知に判定できる新しいクラスが実証されたとき。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/adr.md` — ADR front-matter と decision の根拠追跡
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR の配置と lifecycle
- `knowledge/conventions/responsibility-boundary.md` — framework 側 / 利用者側の境界
- `.harness/config/template-boundary.json` — template export の境界 manifest
- `.harness/custom/review-prompts/harness-policy.md` — 出荷対象と参照漏れの review policy
