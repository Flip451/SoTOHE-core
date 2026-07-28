---
adr_id: "2026-07-24-0326-consumer-convention-ownership-and-harness-decoupling"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01FBDuBN1aC3eCyfjHsoAS9V:2026-07-26:phase0-boundary-approval"
    candidate_selection: "from:[file-by-file-include,directory-overlay-with-initial-set,empty-output] chose:directory-overlay-with-initial-set"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_01FBDuBN1aC3eCyfjHsoAS9V:2026-07-26:staleness-amendment-adjudication"
    candidate_selection: "from:[keep-under-conventions,move-under-workflows,split-into-harness-policy-and-reference] chose:split-into-harness-policy-and-reference"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:convention-boundary-hearing:2026-07-24"
    candidate_selection: "from:[export-all,export-none,curated-customizable-overlay] chose:curated-customizable-overlay"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_01FBDuBN1aC3eCyfjHsoAS9V:2026-07-26:phase0-boundary-approval"
    candidate_selection: "from:[fixed-file-paths,capability-directories,frontmatter-capability-resolution] chose:frontmatter-capability-resolution"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:convention-boundary-hearing:2026-07-24"
    candidate_selection: "from:[closed-capability-vocabulary,open-capability-identifiers] chose:open-capability-identifiers"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session_01FBDuBN1aC3eCyfjHsoAS9V:2026-07-26:phase0-boundary-approval"
    candidate_selection: "from:[provider-native-preflight,dispatcher-owned-resolution] chose:dispatcher-owned-resolution"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:convention-boundary-hearing:2026-07-24"
    candidate_selection: "from:[merge-artifact-convention-refs,inject-required-for-only] chose:inject-required-for-only"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session_01FBDuBN1aC3eCyfjHsoAS9V:2026-07-26:staleness-amendment-adjudication"
    candidate_selection: "from:[fixed-type-designer-extension-contract,split-type-design-ownership] chose:split-type-design-ownership"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:convention-boundary-hearing:2026-07-24"
    candidate_selection: "from:[mechanical-path-substitution,ownership-based-reference-migration] chose:ownership-based-reference-migration"
    status: proposed
  - id: D10
    user_decision_ref: "chat_segment:session_01FBDuBN1aC3eCyfjHsoAS9V:2026-07-26:phase0-boundary-approval"
    candidate_selection: "from:[compatibility-stubs,update-live-references-with-history-exemption,declaration-backed-dead-reference-check] chose:update-live-references-with-history-exemption"
    status: proposed
---
# consumer 規約の所有権分離と harness 固定依存の撤去

## Context

`.harness/config/template-boundary.json` は `knowledge/conventions/` の文書をファイル単位で
`include` / `exclude` に分類している。しかし、そのディレクトリには性質の異なる文書が
同居している。

1. テンプレート利用プロジェクトが自ら所有し、採用・改稿・削除できる工学規約。
2. SoTOHE workflow、capability、CLI が正しく動くための実行契約・schema・policy。
3. SoTOHE-core 自身またはテンプレート保守者だけに適用する開発規約。

この三者を「convention」という名前だけで同一の出荷境界に置いた結果、
`workflow-ceremony-minimization.md` のような保守者向け文書や、ADR schema、
branch strategy、review protocol のような harness 実行契約まで consumer 規約として
出荷されている。ファイルごとの汎用性判定だけでは、新しい文書が増えるたびに同じ判断を
繰り返し、harness が所有すべき固定契約と consumer が変更してよい規約の境界も曖昧なまま
残る。

`knowledge/conventions/` はプロジェクト固有規約の置き場である。SoTOHE-core
リポジトリでは SoTOHE-core 開発用規約を置き、テンプレート出力後は利用プロジェクトが
所有する規約だけを置く、という対称な原則が望ましい。したがって、source 側の
`knowledge/conventions/` をそのまま export するのではなく、consumer 向けの初期規約だけを
overlay として供給する必要がある。

一方、規約を「利用者が変更できる」と宣言するだけでは不十分である。現在の
`type-designer` などは特定の convention ファイル名を直接参照しているため、利用者がその
ファイルを改名・分割・削除すると適用関係が壊れる。`track:adr2pr` から辿る workflow には
convention 索引を使って必読規約を発見する一般契約もなく、後続の review / implement が
spec に既に記録された `convention_refs` を読むだけでは、プロジェクト全体に常時適用する
規約を capability へ注入できない。

規約と capability の関係は多対多である。同じ規約を `type-designer`、
`implementer`、`reviewer` が読む場合があり、利用者が custom capability を追加する場合も
ある。capability ごとのディレクトリへ分類すると文書の複製または特別な共有規則が必要に
なり、固定ファイル名を別の固定配置へ置き換えるだけになる。

この ADR は、consumer 規約、harness 実行契約、SoTOHE-core 開発規約の所有境界と、
capability が consumer 規約を発見する読み取り契約を決める。既存の convention
作成・索引更新・索引検証機能の存廃や再設計は決定しない。

## Decision

### D1: knowledge/conventions を consumer 所有の export 境界にする

`.harness/config/template-boundary.json` では、source 側の `knowledge/conventions` ディレクトリ全体を次の原則で `overlay` とする。

```json
{ "pattern": "knowledge/conventions", "classification": "overlay" }
```
<!-- illustrative, non-canonical -->

テンプレート出力へ渡す規約は `overlay/knowledge/conventions/` から明示的に供給し、出力側の内容は overlay だけで決まる。
`exclude` は「何も出荷しない」を意味し overlay からの供給経路を持たないため採らない。
分類は入れ子にできないので、ディレクトリ単位の `overlay` 一件で表現する。
これにより、source 側の同ディレクトリは SoTOHE-core というプロジェクト自身の規約置き場、
出力側の同ディレクトリは利用プロジェクト自身の規約置き場になる。

overlay は初期値であり、出力後の利用者は文書を追加・改稿・改名・削除できる。
consumer が削除できない workflow 実行契約や schema を overlay convention に置いては
ならない。

この決定は、conventions をファイル単位の `include` / `exclude` に分類する `2026-07-23-0117-export-surface-minimization.md` D1 の方向を、所有権に基づく directory `overlay` へ置き換える。
既存 ADR 自体は歴史記録として変更しない。

### D2: harness 実行契約を policies と reference へ移す

利用プロジェクトが削除すると workflow または capability の意味が変わる文書は
consumer convention ではない。次のように `.harness/policies/` または
`.harness/reference/` へ移す。

| 現在の文書 | 移動先 |
| --- | --- |
| `knowledge/conventions/adr.md` | `.harness/reference/adr-schema.md` |
| `knowledge/conventions/catalogue-schema-reference.md` | `.harness/reference/catalogue-schema.md` |
| `knowledge/conventions/bash-write-guard.md` | `.harness/reference/guard-semantics.md` |
| `knowledge/conventions/branch-strategy.md` | `.harness/policies/branch-strategy.md` |
| `knowledge/conventions/git-notes.md` | `.harness/policies/git-notes.md` |
| `knowledge/conventions/review-protocol.md` | `.harness/policies/review-protocol.md` |
| `knowledge/conventions/sot-reentry-sequencing.md` | `.harness/policies/sot-reentry-sequencing.md` |
| `knowledge/conventions/task-completion-flow.md` | `.harness/policies/task-completion.md` |
| `knowledge/conventions/track-lifecycle.md` | `.harness/policies/track-lifecycle.md` |
| `knowledge/conventions/no-upstream-restatement.md` | `.harness/policies/no-upstream-restatement.md` |
| `knowledge/conventions/pre-track-adr-authoring.md` | `.harness/policies/pre-track-adr-authoring.md` |
| `knowledge/conventions/responsibility-boundary.md` | `.harness/policies/consumer-ownership.md` |

`impl-delegation-arch-guard.md` は分割する。provider に依存しない委譲手順と
architecture guard の実行責務は `.harness/policies/implementation-delegation.md` に置く。
具体的な layer 名、role × layer 配置、利用プロジェクトが選択する設計方針は
`architecture-rules.json`、catalogue lint 設定、consumer convention に残す。

`.harness/workflows/` は `.harness/capabilities/` と対を成す運用手順 SSoT の置き場である。
policy や reference 文書をそこへ混在させず、workflow は必要な
`.harness/policies/` / `.harness/reference/` 文書を直接参照する。

移動先ディレクトリは `.harness/config/template-boundary.json` に分類エントリを新設する。
`.harness` は per-subdirectory / per-file に分類されており、未分類のファイルは export が fail-closed で拒否するため、`.harness/policies` と `.harness/reference` を宣言しない限り出荷できない。

`workflow-ceremony-minimization.md` は consumer runtime の必須入力ではなく、
SoTOHE-core／テンプレート保守者の開発規約として source 側にだけ残し、export しない。

### D3: consumer-neutral な初期規約だけを overlay する

初期 overlay は次の規約と `knowledge/conventions/README.md` に限定する。

- `coding-principles.md`
- `testing.md`
- `security.md`
- `prefer-type-safe-abstractions.md`
- `type-designer-kind-selection.md`

overlay 版は SoTOHE-core の crate 名、内部 path、JST 運用などを含まない
consumer-neutral な初期値にする。特に `type-designer-kind-selection.md` の overlay 版は、
利用プロジェクトが role × layer 方針を変更または全面削除できる project-owned rule とする。

`enforce-by-mechanism.md` と `filesystem-persistence-guard.md` は初期 overlay に含めない。
前者の harness 自己強制に必要な内容は harness policy／mechanism 側で所有し、後者は
すべての利用プロジェクトへ課す既定規約とはしない。
`language-policy.md`、`typed-deserialization.md` を含むその他の source convention も、
consumer-neutral 化と実際の capability consumer が別途確定しない限り overlay しない。

### D4: required_for frontmatter を読み取る convention resolver を導入する

consumer convention は任意の YAML frontmatter に `required_for` を持てる。
値は、その文書を必読とする capability ID の配列である。

```yaml
---
required_for:
  - type-designer
  - implementer
  - custom-threat-modeler
---
```
<!-- illustrative, non-canonical -->

読み取り専用コマンド `bin/sotp conventions resolve --capability <capability-id>` を導入する。
resolver は `knowledge/conventions/**/*.md` を読み取り専用で走査し、指定された capability ID と `required_for` の要素が完全一致する文書の repository-relative path を、重複なく安定順の機械可読な正本として返す。
人間向け表示を持つ場合も、その正本と同じ解決結果からレンダーする。
resolver は文書の作成、更新、削除、索引生成を行わない。
既存の convention 作成・更新・削除・索引機能の存廃や仕様変更は本 ADR の対象外である。
frontmatter が YAML として解析できない場合、`required_for` が文字列配列でない場合、`required_for` に空文字または空白だけの capability ID がある場合、解決対象 path が `knowledge/conventions/` の外へ逸脱する場合、または対象文書を読み取れない場合は、構造上の異常として fail-closed とする。
frontmatter がない文書、`required_for` がない文書、指定 capability に一致する文書がゼロ件である状態は正常な空結果とする。

### D5: required_for の capability ID を open-ended にする

`required_for` の capability ID は harness 組み込み capability の closed enum にしない。
`.harness/capabilities/` や `agent-profiles.json` に現在登録されていない ID も、
将来または利用者定義の custom capability を表す正当な値として受理する。
resolver は登録有無を検証せず、非空文字列の完全一致だけで解決する。

未知の capability ID を許容するため、タイプミスと未使用 custom capability を
resolver が区別することはできない。この曖昧さは open extension point の代償として
受け入れ、未知 ID を error に変換しない。

### D6: capability exec が orchestrator-output capability へ resolver 結果を注入する

`orchestrator-output` capability は canonical な dispatcher である `bin/sotp capability exec <capability> --host <host> --briefing-file <path>` を必ず経由する。
dispatcher は実行主体を決定する前に、対象 capability ID を使って D4 の resolver を一度だけ実行する。
解決結果の引き渡しは dispatcher outcome ごとに次のように行い、外部 provider 実行と in-host 委譲のどちらの経路でも解決結果が実行主体へ届くことを保証する。

- `CAPABILITY_EXEC_OUTCOME: executed`: 解決した規約 path と全文書を読む義務を、dispatcher が provider prompt／実行入力へ注入してから外部 provider を実行する。
- `CAPABILITY_EXEC_OUTCOME: delegate-in-host`: 解決した規約 path と全文書を読む義務を、dispatcher が返す delegation payload の `discipline` に含め、host orchestrator は `briefing_file` と `discipline` をそのまま provider-native subagent の依頼へ渡す。

Claude host と Claude provider の組合せで Claude Code の subagent を使う場合も、先に `capability exec` が `delegate-in-host` を返し、その指示を host orchestrator が Agent 呼び出しへ運ぶ。
host orchestrator は resolver を再実行せず、規約本文を独自に要約しない。
dispatcher を経由しない Agent／skill の直接起動は supported entrypoint とせず、同等の preflight を別実装する経路も設けない。

これにより `track:adr2pr` 自体が規約一覧を管理しなくても、そこから `capability exec` で起動される各 capability が自身の必読規約を canonical dispatch 境界で取得できる。

`reviewer`、`review-fix-lead`、`dry-fix-lead` 等、専用 CLI が prompt 構築と provider dispatch を所有する `typed-pipeline` capability は本決定の対象外とする。
特に review は scope ごとに必読規約の集合が異なるため、単一 capability ID による解決をそのまま適用しない。

### D7: dispatcher が注入する規約を required_for の解決結果だけに限定する

D6 の dispatcher が注入するのは、resolver が capability ID の `required_for` に対して
返した project-wide convention だけとする。

`spec.json` 等の track 成果物に記録された `convention_refs` を dispatcher が探索・解決・
合成してはならない。成果物参照には、どの成果物を入力にするか、phase 間でどう伝播するか、
anchor をどう検証するかという別の責務があるため、frontmatter resolver の注入機能へ
混在させない。

D6 対象 capability は consumer convention の固定ファイル名を参照しない。実行結果が
project-wide convention を根拠として示す場合は、その実行で resolver が返した実際の文書
path を使う。harness 自身が所有する policy／reference への固定参照はこの禁止対象ではない。

### D8: type-designer の固定 extension contract を所有責務へ分解する

type design の責務は次の境界へ分解する。

- role 語彙と catalogue schema: `.harness/reference/`
- 汎用的な選定・生成・注釈手順: `.harness/capabilities/type-designer.md`
- 機械制約: `architecture-rules.json` と catalogue lint
- project 固有の選定規則: `required_for: [type-designer]` を持つ consumer convention

type-designer は project 固有規則のファイル名を知らず、D6 で注入された resolver output を
読む。overlay の `type-designer-kind-selection.md` は初期値にすぎず、利用者による改名、
分割、削除を許容する。

D6 対象 capability は、consumer convention の固定ファイル名だけでなく、その文書内の固定節見出しも参照しない。
capability 手順が project 固有の設計規則を確認する場合は、resolver が当該 capability ID に対して返した convention 群を読み、そこに宣言された規則を確認する形にする。
resolver が 0 件を返す状態は D4 の正常結果であり、その場合その確認手順は対象を持たない。

role × layer マトリクスの層値は consumer が所有する layer id であり、出荷 catalogue-lint config の `permitted_layers` はその写像である。
したがってマトリクス本体は harness 所有の reference ではなく consumer convention に置く。
harness が所有するのは role 語彙、`KindLayerConstraint` という rule 種別と検査意味論、および type-designer が lint gate を通す義務までとする。
「マトリクスで許可された層を lint config に宣言する」という規範は、export 後は consumer 内部の整合義務として働き、harness 契約ではない。

### D9: export 対象から外れる convention への live reference を所有権に基づいて移行する

D2 の移動と D3 の export 縮小を行う前に、現在 include されている convention への
track 成果物以外からの参照を全件監査する。参照 path の機械置換だけではなく、参照して
いた規範の所有者に従って、各参照を次のいずれかへ分類して移行する。

1. **複数 workflow／capability が共有する harness 契約**:
   `.harness/policies/` または `.harness/reference/` へ移し、live consumer の参照を
   新しい path へ更新する。
2. **単一 workflow／capability に固有の短い実行条件**:
   その条件を所有する workflow／capability SSoT の本文へ吸収し、元 convention と
   path 参照を廃止する。移動後に同じ規範を別文書へ重複保持しない。
3. **consumer が変更できる project convention**:
   D6 対象 capability の固定 path 参照を削除し、D4、D6、D7 の resolver 注入へ
   置き換える。実行結果が根拠を示す場合は、実際に resolver が返した文書 path を使う。
4. **説明・設計背景だけの非規範的参照**:
   実行に不要なら参照自体を削除する。現行の実行条件を理解するために必要な最小説明だけを、
   その live SSoT へ自己完結する文章として残す。
5. **SoTOHE-core／maintainer だけの source convention**:
   source-only consumer からは参照できるが、export される workflow、capability、
   prompt、entry document からは参照しない。

「参照を文章に置き換える」は分類 2 または 4 の場合だけに行う。複数箇所で共有する規範を
各 consumer へ複製したり、consumer が変更すべき規約を harness 文面へ固定化したりしては
ならない。

review CLI と `.harness/custom/review-prompts/` は resolver 注入へ移行しない。review prompt
にある現在の固定 convention 参照も監査対象とし、分類 1、2、4 のいずれかとして harness
policy／reference への移動、scope prompt 本文への必要最小限の吸収、または不要参照の削除で
解消する。consumer 所有規約への固定参照は残さないが、その規約を scope ごとに自動選択して
review briefing へ注入する代替機構は本 ADR の実装へ混在させず、後続判断に委ねる。

### D10: live surface の参照を共同更新し、歴史成果物の後方互換は持たない

D2、D3、D9 の変更では、同じ変更集合の中で surviving live surface の参照を新しい
`.harness` path、resolver 経由、または自己完結した実行条件へ更新する。対象には少なくとも
`.harness/workflows/`、`.harness/capabilities/`、`.harness/custom/`、
`.agents/`、`.claude/`、top-level entry document、設定、script、production source、
test fixture、overlay を含める。

template smoke は export 済み tree を対象に、overlay 以外の source convention が出荷されていないことを検査する。
workflow／capability／entry document の散文から convention path を抽出する dead-reference 検査と、そのためだけの新たな契約や再発検出機構は導入しない。
移動漏れは同一変更集合内の live surface 更新 (D9) と review で担保する。

移動前 path の stub、redirect document、alias は作らない。
`track/items/**`、`track/archive/**`、および現行手順として参照されていない既存 ADR／research note の旧 path は歴史記録として遡及更新せず、dead live reference として扱わない。

## Rejected Alternatives

### A. conventions を引き続きファイル単位で include / exclude する

個別分類では、harness 実行契約と consumer 規約が同じ名前空間に残る。新規文書ごとの
出荷判断も継続し、利用者が変更してよい範囲が path から判別できないため却下する。

### B. knowledge/conventions を空で出力し、初期 overlay を持たない

所有境界としては最も純粋だが、利用者が規約 schema と適用方法を一から設計する負担が
大きい。削除可能な consumer-neutral 初期値を overlay する方が、所有権を損なわず
導入負担を下げられるため採らない。

### C. harness 実行契約を .harness/workflows/ にまとめる

`.harness/workflows/` は運用手順 SSoT、`.harness/capabilities/` は役割 SSoT であり、
この対へ schema と横断 policy を混在させると責務が崩れる。
`.harness/policies/` と `.harness/reference/` を分けて用いる。

### D. capability が既知の convention ファイルを直接参照する

実装は単純だが、利用者による改名・分割・削除で壊れる。初期値のファイル名が事実上の
永久 API となり、「project-owned convention」という境界を形骸化させるため却下する。

### E. capability ごとの convention ディレクトリを作る

一つの規約を複数 capability が必読とする場合に、文書複製、symlink、共有ディレクトリ、
複数走査といった例外が必要になる。多対多関係を文書 metadata で直接表す方が単純である。

### F. required_for を用途タグまたは既知 capability の closed vocabulary にする

用途タグから実際の読者への別 mapping が必要になり、custom capability を追加するたびに
harness 側の vocabulary 更新が必要になる。capability ID の open string を直接使う。

### G. resolver を後続変更へ先送りする

resolver がない期間は、固定ファイル参照を残すか、overlay 規約が自動適用されない状態を
許容するしかない。customizable な初期規約を実効的な契約として出荷するための最小機構
なので、本変更に含める。

### H. 既存の convention 管理機能を同時に廃止または再設計する

既存 CRUD／索引機能は canonical track workflow に十分接続されていないが、その事実だけで
即時廃止を決める必要はない。resolver は読み取り専用で独立して導入できるため、管理機能の
存廃は利用実態と移行範囲を改めて調査する後続判断へ委ねる。

### I. 移動前 path に互換 stub を残す

後方互換を不要とするプロジェクト原則に反し、旧 path と新 path のどちらが SSoT かを
再び曖昧にする。live surface を共同更新し、歴史成果物は当時の記録として残す。

### J. dispatcher を経由しない provider-native entrypoint に resolver を再実装する

現在の canonical route は、同一 host/provider の in-host delegation でも先に
`capability exec` を通る。直接 Agent／skill 起動を追加で support すると、
provider／model 解決、sandbox discipline、規約解決の三つが二重実装になるため採らない。

### K. 既存参照を新 path へ一律に機械置換する

参照先の中には harness 共通契約、単一 capability の実行条件、consumer 所有規約、
非規範的背景が混在する。一律置換では誤った所有者の下へ規範を残すため、D9 の意味的分類を
先に行う。

### L. review 用 CLI と scope prompt に resolver 注入を同時導入する

review は domain、usecase、types、impl-plan 等の scope ごとに参照すべき規約集合が異なる。
単一の `reviewer` ID に対する集合を全 scope へ注入すると過剰読込になり、scope を
`required_for` に混ぜると capability ID という D5 の意味が崩れる。scope-aware な metadata
と briefing 合成は独立した設計問題なので、本変更には含めない。

### M. dispatcher が track 成果物の convention_refs も解決して注入する

project-wide な適用関係を解決する frontmatter resolver と、track の SoT chain に記録された
明示参照の探索・anchor 検証・phase 間伝播は異なる責務である。同じ dispatcher preflight に
まとめると、resolver導入と成果物schema／phase依存が結合するため、本変更には含めない。

### N. convention 参照を機械可読な宣言に集約して dead reference 検査を残す

参照元を manifest 等の宣言へ集約すれば構造検査として成立するが、その宣言自体が検査を残すためだけの新設契約になる。
移動漏れの検出価値より、契約を増やす代償が大きい。

## Consequences

### Positive

- template consumer は、自分が所有する規約だけを `knowledge/conventions/` で扱える。
- harness の必須 schema／policy が consumer の任意規約から分離され、削除可能範囲が
  path で判別できる。
- 文書と capability の多対多関係を複製なしで表現できる。
- custom capability は harness の closed vocabulary 更新なしに規約解決へ参加できる。
- capability は固定ファイル名ではなく project-owned metadata を通じて規約を取得する。
- same-host の in-host delegation でも dispatcher が一度だけ規約を解決し、host ごとの
  resolver 再実装を避けられる。
- 既存参照は単なる path 変更ではなく、規範の実際の所有者へ移される。

### Negative

- 文書移動、内容分割、hard-coded reference の意味的な仕分けが広い変更集合になる。
- resolver、frontmatter codec、capability dispatch への注入とそのテストが必要になる。
- 外部 provider 実行と in-host 委譲の双方について、解決した規約 path と全文書を読む義務が実行入力へ渡ることを適合テストで確認する必要がある。
- capability 起動ごとに convention tree を走査する読み取りコストが増える。
- open-ended ID では typo と未使用 custom capability を機械的に区別できず、規約が
  意図せず未参照になる可能性が残る。
- consumer-neutral overlay を SoTOHE-core の規約変更とは別に保守する必要がある。
- review は本 ADR の時点では consumer convention を自動解決しないため、利用者が変更した
  規約を scope-aware に review へ反映する機能は後続設計まで提供されない。
- dead reference の機械検出を持たないため、移動漏れは live surface 更新と review に依存する。

### Neutral

- 既存の `conventions add` / `update-index` / `verify-index` は本 ADR では廃止も拡張も
  決定しない。
- `.harness/workflows/` と `.harness/capabilities/` の現在の役割分担は維持する。
- provider-native Agent／skill の直接起動は supported entrypoint に追加しない。
- review 用 CLI、scope ごとの briefing 合成、typed-pipeline capability へのresolver注入は
  本 ADR の対象外である。
- `convention_refs` による track-specific な明示参照は維持するが、D6 の dispatcher は
  読み取りも注入も行わない。
- 歴史的な track 成果物に残る旧 path はその時点の記録であり、互換性保証の対象外となる。

## Reassess When

- capability ID の typo による未適用が繰り返し発生し、open-ended ID を維持したまま
  advisory diagnostics を追加する必要が生じたとき。
- capability ID だけでは適用条件を表せず、layer、phase、profile 等との条件合成が必要に
  なったとき。
- convention tree の走査コストが capability 起動時間へ有意に影響し、cache または
  manifest が必要になったとき。
- consumer から初期 overlay が過剰または不足しているという反復的なフィードバックが
  得られたとき。
- 既存の convention CRUD／索引機能を canonical workflow へ接続するか廃止するかを
  判断できる利用実態が集まったとき。
- review scope ごとに consumer convention を選択する必要が実証され、scope-aware な
  metadata と briefing 注入を独立して設計するとき。
- track 成果物の `convention_refs` を writer capability へ自動注入する必要が実証され、
  対象成果物、phase 間伝播、anchor 検証を独立した機能として設計するとき。

## Related

- `.harness/config/template-boundary.json` — source / overlay / export の分類境界
- `.harness/workflows/` — provider-neutral な運用手順 SSoT
- `.harness/capabilities/` — capability 契約と規約注入先
- `architecture-rules.json` / `.harness/catalogue-lint/` — project 固有の機械制約
- `knowledge/adr/2026-07-06-1717-template-extraction-boundary.md` — copy + overlay による
  template 抽出境界
- `knowledge/adr/2026-07-23-0117-export-surface-minimization.md` — file 単位 convention
  分類の先行判断
- `knowledge/adr/2026-07-17-0247-docs-architecture-ssot-realignment.md` — 文書 SSoT と
  機械制約の分離
