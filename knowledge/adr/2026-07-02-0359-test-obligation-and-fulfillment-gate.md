---
adr_id: 2026-07-02-0359-test-obligation-and-fulfillment-gate
decisions:
  - id: D1
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 裁定「A+B+D: 義務+検証のみ」+ 2026-07-04 確認・訂正「結合構造は『テストの存在検証』と『鮮度の乖離検出 + fulfillment 検証』」→ 存在検証を独立検査、鮮度⇄fulfillment を不可分の対として明文化"
    candidate_selection: "from:[義務+検証のみ, コード生成込みフル, 義務+driftのみで起動し検証は後続] chose:義務+検証のみ"
    status: proposed
  - id: D2
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 ユーザー提案「roleは形を決め、anchorが意味論レビューの観点を与える」+ 2026-07-03 追補（role payload / pattern 宣言も形の入力、VO は invariants 宣言の有無で shaped）+ 同日精緻化「形の指定は最低限テストを作成する必要がある義務の一覧を決める」+ 2026-07-04 指摘「義務数×参照数の乗算になっていないか」→ 導出を entry 単位に修正（edge は会計単位に限定）"
    status: proposed
  - id: D3
    user_decision_ref: "chat:session-86a37bf1:2026-07-04 指摘「型エントリが引用している時点で何かしらの制約を含むのでは。義務の個数はエントリの形が規定するのだから、対象外を作らなくてもいい」→ 種別フィルタを廃止し、テスト可能性を edge ごとの verdict に委ねる形へ改訂（種別は prior と edge ゼロ警報範囲にのみ使用）"
    status: proposed
  - id: D4
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 ユーザー提案「黙って義務ゼロにせず、LLMから『テスト義務なし』のverdictを得る機構を持ち込むべき」+ 2026-07-03 指摘「anchor 単位評価は黙って消える経路を発生させる悪手。すべての ref chain を評価対象とすべき」+ 2026-07-06 指摘「仕様が複雑になりすぎ」「test-bindings の JSON をレビュー対象にするのは不適切。waived の理由散文を ref-verify と同じ仕組みで検証させるべき」→ waiver を『implementer が author し LLM が検証する』分業に全面簡素化（warrant 機構群を Rejected H へ）+ 同日指摘「本機能へのルーティングが実装と無関係のところで起きたら、特にテンプレート利用者は解決困難」→ 自発的 binding を新設し決定表の漏れを非ブロッカー化（上流報告へ格下げ）"
    status: proposed
  - id: D5
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 二軸モデル+waiver+多重cite整理一式への承認 + 2026-07-03 指摘「この重複排除って必要？」→ dedup 条項を削除（edge モデルでは重複は定義上生じず、dedup は silent-merge の再発）"
    status: proposed
  - id: D6
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 ユーザー指摘「evidenceはcatalog-spec chainの一本一本でありSoT chainのノードではない。ref-verifyの型に嵌め込むのに慎重になるべき」"
    status: proposed
  - id: D7
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 一階/二階整理+core抽出方針への承認「OKです」"
    status: proposed
  - id: D8
    user_decision_ref: "chat:session-86a37bf1:2026-07-02 命名議論（chain3不採用・coverage族・task-contract被覆検証への命名付与）+ 2026-07-03 指摘「test-grounding はテストが grounding を持つことの検証に見える。義務を果たしているかを端的に表す命名が正しい」→ obligation-fulfillment に改名"
    status: proposed
  - id: D9
    user_decision_ref: "chat:session-86a37bf1:2026-07-03 ユーザー提案「ソースコードに埋め込むのではなく、義務一覧ファイルにレイヤー名・モジュールパス・テスト名を記録してそれをキーにソースコードを走査する」+ 2026-07-06 指摘「義務 id が上流の変更のたびに変化すると下流が不安定。entry と anchor が同一なら同じ値をとることが望ましい」→ id は同一性のみから構成（義務 id は entry/宣言項目、edge id は entry/anchor に分離し、内容 hash・index 不使用）"
    candidate_selection: "from:[marker埋め込み, bindingファイル+走査] chose:bindingファイル+走査"
    status: proposed
  - id: D10
    user_decision_ref: "chat:session-86a37bf1:2026-07-06 ユーザー提案「決定表を config ファイル化するのが（実装は重いが）この問題の解決策として最適では」→ hard-code（旧案・Rejected I）から config 化 + ロード時 totality 検証へ全面変更"
    candidate_selection: "from:[config化+totality検証, Rust網羅match hard-code, trait方式] chose:config化+totality検証"
    status: proposed
  - id: D11
    user_decision_ref: "chat:session-86a37bf1:2026-07-06 裁定: calc は微妙・verify は強引との指摘を経て derive を採用、意味論レーンは「run にするくらいなら evaluate の方がよい」で evaluate に確定（derive / check / evaluate / results）"
    candidate_selection: "from:[calc/check/verify/results, derive/check/run/results, derive/check/evaluate/results] chose:derive/check/evaluate/results"
    status: proposed
  - id: D12
    user_decision_ref: "chat:session-86a37bf1:2026-07-03 指摘「D12っている？1トラックで全ロールの義務を定義できない？」→ role 段階導入（旧: usecase-first 裁定）を撤回し、単一 track での全 role 一括定義に差し替え"
    candidate_selection: "from:[全role一括定義, usecase-first段階導入, ValueObject-first段階導入] chose:全role一括定義"
    status: proposed
  - id: D13
    user_decision_ref: "chat:session-86a37bf1:2026-07-04 裁定（Reference 除外に同意 / DomainService は per-メソッド確認）+ 2026-07-06 裁定（emits 一律生成: 指摘「anchor 不在は chain② で別途拒絶される」により空振り懸念が消滅し維持確定 / trait_impls 契約適合: 「含めておいて、不要ならテンプレート利用者ごとに config で無効化」で確定 / when_trait_role_in 条件式は map 形式へ統一 / config JSON を D13 に直接記載）"
    status: proposed
  - id: D14
    user_decision_ref: "chat:session-86a37bf1:2026-07-09/10 ユーザー直接指示「63 個の義務 id を implementer が手写する体制は現実的でない。`test-bindings.json` と同一 wire 形状の schema-pure draft を stdout に出す authoring helper を追加せよ。gate 面には含めず、副作用（`test-bindings.json` / verdict cache への write）は一切持たせない。`results` は informational（exit 常に 0、verdict gate は `check` の責務）である旨を明文化」"
    status: proposed
---

# テスト義務ゲートと obligation-fulfillment 意味論検証 — SoT chain 第三リンクの意味論検証の完成

## Context

SoT chain の各リンクは「構造検証 + 意味論検証」の二層で守られる設計だが、第三リンク（impl → catalogue）だけ意味論検証が未構築である。現行のリンク検証インターフェースは `bin/sotp signal` の calc/check 対で提供されている。

```
user (承認来歴)
 ↑ ⓪  signal calc-adr-user / check-adr-user            意味論検証: (来歴のため対象外)
adr
 ↑ ①  signal calc-spec-adr / check-spec-adr            意味論検証: ref-verify (spec-adr)
spec
 ↑ ②  signal calc-catalog-spec / check-catalog-spec    意味論検証: ref-verify (catalog-spec)
catalog
 ↑ ③  signal calc-impl-catalog / check-impl-catalog    意味論検証: ★未構築 ← 本 ADR
impl
```

第三リンクの意味論——「実装が設計意図どおり**振る舞う**か」——は静的比較では問えず、実行可能な検証物＝**テスト**を媒介にして初めて問える。よって本 ADR はテスト関連の仕組みとして設計されるが、その正体は第三リンクの意味論検証レーンである。

ただし本ゲートは既存 ref-verify と**階数が異なる**。既存 ref-verify は SoT chain の隣接ノード間に人が author した citation（edge）をなぞり、その意味整合を問う**一階の検証**である。本ゲートの evidence は catalog-spec chain の edge 一本一本（catalogue entry × anchor の citation）であり、SoT chain のノードではない。テストという新しい人工物（witness）が edge という約束（promise）の履行を実証しているかを問う**二階の検証**である（witness は「存在主張を真だと示す具体物」を指す論理学の用語。ここでは、約束が果たされていることを実行可能な形で示す物＝テストを指す。判定 pair の evidence——根拠側の語——とは別物であることに注意）。この構造差が本 ADR の設計判断の多くを規定する（D6）。

型カタログの各 entry には `role` が割り当てられ、`spec_refs` で spec 要素と紐づく。この2つの情報は性質が異なる: role（とその payload / pattern 宣言）は**最低限作成すべきテスト義務の件数と種類**を決め、anchor は各義務の**意味**（何を検証すべきか）を与える（D2）。

## Decision

### D1: スコープは「義務の導出 + 検証」であり、テストコード本文の自動生成は行わない

本機能は次の**三段構え**である: **義務の導出**が必要なテストを列挙し（何を果たすべきか）、**存在検証**が義務に対する履行の試み＝テストの記述を検証し（試みがあるか）、**鮮度⇄fulfillment 検証**がその履行が現に・意味的に正しいことを検証する（正しく果たしたか）。Rust テストコード本文の自動生成は**行わない**。

- **義務の導出**: catalogue + spec から「どのテストが存在すべきか」（義務一覧 + per-義務 brief）を決定論的に導出する
- **テストの存在検証**: binding artifact をキーにソースを走査し、義務に対応するテストが存在するか（missing / orphaned）を検出して CI ゲート化する（D9）。verdict の状態と独立に完結する決定論検査
- **鮮度の乖離検出 + obligation-fulfillment 検証**: 書かれたテスト本体が、cite した anchor の約束する振る舞いを実際に検証しているかを LLM verdict で判定し（D6。author された waived 理由の検証も同型の pair として同レーンで行う・D4）、verdict を hash 組に凍結する。鮮度の乖離（evidence 側の `spec_changed` / `decl_changed`、claim 側の `test_changed` / `reason_changed`・D9）は独立した検査ではなく、**この verdict の失効という同一事象の表示名**である

結合構造は「存在検証（独立）」と「鮮度⇄fulfillment（不可分の対）」である。hash は単独ではゲートにならない——機械的に再計算すれば通ってしまい、「変更を読んで整合を確かめた」のか「盲目的に再発行した」のかを区別できない（既存 sot-chain 意味論レビューゲート ADR が層②/③について確立した関係と同一）。鮮度の回復手段は fulfillment 再検証の pass のみとする（D6）。

テスト本文の執筆は implementer capability（人/AI）が brief を元に行う。呼び出し部の機械的正確さは rustc が保証し、テストと義務の結び付けは binding artifact（D9）が担う。実装が AI 前提である本プロジェクトにおいて、テスト本文まで機械生成する価値は、その生成器（型参照の解決と、生成コードと人の編集の共存を管理する仕組み）の実装・保守コストに見合わない。

### D2: 二軸モデル — 宣言（role / payload / pattern）が義務の件数と種類を決め、anchor が意味を与える

テスト義務を、**entry 単位**の導出関数の出力として定義し、意味の会計は **edge 単位**で行う。

```
derive:  宣言 → [義務]           // entry 単位、0..n 件。件数は宣言のみの関数
binding: 義務 → [test]           // implementer が author。1義務に複数テスト可（D9）
edge 会計（D4）: (entry, cite 先 anchor) ごとに verdict {fulfilled / waived / fail} へ解決——
    fulfilled … bind された test 群が anchor の約束する振る舞いを検証している（LLM 検証）
    waived   … implementer が author した免除理由（散文）が waiver 検証（LLM）を pass している
    義務の有無を問わず、どの edge もいずれの verdict でも解消できる
    （義務が導出されていない edge へのテスト結束は 自発的 binding・D9）

  宣言の軸 → catalogue の宣言が義務の「件数と種類」を列挙する:
             role（DataRole / ContractRole / FunctionRole）
             + role payload（ValueObject / Entity / AggregateRoot の invariants 等）
             + pattern 宣言（typestate の遷移メソッド等）
  意味の軸 → anchor 本文が「検証されるべき内容」＝約束された振る舞いを規定する
```

**義務数は cite 数に乗算されない**。これが成り立つのは **1義務 = 1テストではなく、1義務 = 1テスト群**だからである（binding は1義務に複数テストを許す・D9）。invariants は entry の性質であって edge の性質ではなく、ある1つの invariant に対する境界テスト義務は、entry が cite する anchor が何本あっても1件である。anchor の多様さは義務の個数ではなく**テスト群の内訳**に現れる: 1つの義務に bind された test 群が、entry の cite する複数 anchor の約束する振る舞いを分担して検証する（例: UseCase の result 義務1件に、成功系 AC とエラー系 AC をそれぞれ検証する2テストを bind する）。cite 先のうち一部の anchor について、その約束を検証するテストがテスト群に1本もない状態（例: 上の result 義務に happy path のテストしか bind されず、エラー系 AC の約束が手つかず）は、その anchor の edge の fulfillment fail——D6 に定める fail 類型 (c)「中心部の未検証」——として検出される。義務の個数は anchor ごとに増えないが、fulfillment 判定は edge（= anchor）ごとに独立に走るため、乗算なしでも黙って消える経路は生じない。

2つの軸はそのまま D1 の検証2段に対応する: 件数と種類（＝義務一覧の充足）は**存在検証**（binding 走査、決定論）が検査し、意味は**鮮度⇄fulfillment 検証**（obligation-fulfillment verdict、hash はその失効トリガ）が検査する。

**宣言の軸の出力は「最低限作成すべきテスト義務の一覧」（下限集合）である**。宣言の内部構造が義務の件数を列挙する: `ValueObject { invariants: [I1, I2] }` なら invariant ごとの境界テスト、typestate pattern なら宣言された遷移メソッドごとに1義務、`ContractRole` なら trait メソッドごとの契約義務——1つの entry から立つ義務は宣言構造に応じて複数になり得る（D10 の導出関数が `Vec<TestObligation>` を返すのはこの意味論）。role 別の生成数ルールは D13 の決定表に定める。この一覧は**下限であって上限ではない**: obligation-coverage がゲートするのは下限の充足のみで、implementer が追加テストを書くことは自由である（binding は1義務に複数テスト可）。

anchor の仕事は、cite された spec 要素が約束する振る舞い（分岐条件・不変条件・期待される結果）を各義務のテストが検証していることの内容規定であって、件数や種類を言わない（例: 「in_progress/done 帰属エントリ全🔵で pass、todo の🟡は無影響、🔴は常時 block」という AC なら、その3分岐の検証が意味の要求になる）。両軸は直交する。

**義務生成数は role 名だけでなく宣言内容で決まる**。代表例:

- `ValueObject { invariants }`: invariants 宣言が**非空**（= validating constructor で境界を守る）なら invariant ごとに境界値テスト義務が立つ（n ≥ 1）。invariants が**空**なら実行時境界が存在せず（型システムが不正状態を表現不能にしている）、0 件
- `ErrorType`: 型としては 0 件。「どの条件でどの variant か」は enum の形から導けないため、義務はそのエラーを**返す関数 / メソッド**側の宣言から立つ
- `Command` / `Query` / `Dto` / `DomainEvent` / `CompositionRoot` / `PrimaryAdapter`: 0 件
- role 全種の生成数ルールは決定表 config（D10）として管理する

義務 0 件の entry は anchor を cite していても義務を生まない。その edge の解消は waived 理由の author + 検証、または 自発的 binding によるテスト + fulfillment 検証（D4 / D9）による（例: invariants 宣言が空なのに anchor が境界の振る舞いを記述している場合、「テスト不要」の理由は成立せず waiver 検証が reject し、「宣言の不足」として type-designer への差し戻しに routing される）。

spec_refs が型レベルにしか付いていなくても義務は導出できる（件数と種類は宣言が型レベルで与え、対象関数の特定は brief 内で implementer に委ねる）。メソッド単位 spec_refs は義務の粒度を上げる任意の精緻化であり、必須にしない。

### D3: anchor 種別で対象外を作らない — テスト可能性は edge ごとの verdict で判定する

spec.json の要素種別は既に構造化されている: `goal`（GO-*）/ `scope.in_scope`（IN-*）/ `constraints`（CN-*）/ `acceptance_criteria`（AC-*）。追加のスキーマ整備は不要である。

**種別を edge 宇宙のフィルタにしない**。型 entry が cite している時点でその edge は約束であり、義務の個数は宣言のみが規定する（D2）ため、種別による事前除外は件数に影響しない。一方で「この種別はテスト不能」という静的規則は、内容のテスト可能性の不存在を証明できない（D4 と同じ原理）——実データでも IN（in_scope）の本文が「artifact はタスクリストと schema_version を含む」のような構造的制約を含む例があり、spec 著者の種別分類の揺れは静的除外では黙って沈む。テスト可能性の判断は、edge ごとの verdict（D4 の解消語彙: waived は裏付け引用必須）に属する。

種別の用途は次の2つに限る:

- **verdict の事前情報（prior）**: 種別を判定 prompt に供給する。GO は目標の grounding、IN はスコープ宣言である蓋然性が高く、waived 解消の自然な候補——だが最終判断は本文の内容に基づく
- **edge ゼロ警報の範囲**: どの entry からも cite されていない `AC` / `CN` は決定論的 finding とする（受け入れ基準・制約が誰にも claim されていない状態は構造的に疑わしい）。`GO` / `IN` の非引用は正常であり警報しない

### D4: obligation-coverage — 義務の不存在も verdict を要する

決定表（宣言 → 義務一覧）は決定論的機構であり、義務の存在は導出できるが**不存在は証明できない**（導出 0 件が「テスト不要」を意味する保証はない）。「静かに義務ゼロ」というだんまり通過を許さず、義務の解消経路を次の2つに限定する。

```
edge の解消 = (a) implementer が test を bind し、obligation-fulfillment 検証を pass する
            | (b) implementer が waived 理由（散文）を author し、waiver 検証を pass する
黙って消える経路 = 存在しない（全 edge の解決宣言が bindings 上で必須 = totality を check が決定論検査）
```

waiver は **implementer が author し、LLM が検証する**——spec / catalogue / テストと同じ分業である。waiver 検証は ref-verify と同型の pair: claim = waived 理由の散文、evidence = edge（entry 宣言 × anchor 本文）、合格には理由のどの主張が edge のどの記述に裏付けられるかの引用を必須とし、判定は edge 局所（「この edge の entry 関連部分について、テスト不要の正当化が成立するか」）である。受理されうる理由の代表形は3つ——検証すべき振る舞いを含まない（設計根拠の grounding）/ 型システムが担保（構造の約束はフィールド定義と rustc が守る）/ 決定論ゲートが担保（設定・構成の検査は verify ゲートが守る）——これは執筆と判定の指針であり、機構ではない。理由は、edge の内容のみから検証できる自己完結の形で書く（判定の edge 局所性は D6）。

waiver 検証が reject した場合は implementer に差し戻す（理由を書き直すか、テストを書いて 自発的 binding で結ぶ）。上流の欠陥——cite している entry の宣言不足、カタログの被覆漏れ——が疑われる場合は、rollback-diagnoser と同型の back-and-forth routing で type-designer へ回す。

**義務導出の決定表の欠陥は track のブロッカーにしない**。解決手段は2層あり、いずれも利用者側で完結する: (1) **ルールレベル**——決定表は利用者所有の config（D10）であり、表の漏れ・自プロジェクト向けの調整は config 編集で解決する（template default への還流報告は任意）。(2) **一回性の edge レベル**——config を歪めたくない単発の事例では、義務が導出されていない edge にテストを直接結ぶ（**自発的 binding**・D9）。検証は通常の fulfillment pair と同一機構で行われ、後に表が更新されて正規の義務が導出されれば、自発的 binding はその履行として引き継がれる。利用者の track が tooling 内部の修正でブロックされる routing は存在しない。

**会計単位は edge（entry × cite 先 anchor、種別を問わない・D3）であり、全 ref edge を評価対象とする**。各 edge は必ず verdict {fulfilled / waived / fail} のいずれかに解決される（D2 の edge 会計）。waived は「0 件 entry 専用の別レーン」ではなく全 edge に共通の解消の一種である。義務を持つ entry でも、その全 edge がテストで解消されるとは限らない: 例えば UseCase entry が、result 義務の対象である AC に加えて、この機能の目的を述べる GO も cite している場合、AC への edge は fulfilled（テストで解消）を要するが、GO への edge は implementer が author した waived 理由（「この anchor は設計目的の記述であり、この edge に検証すべき振る舞いの約束は含まれない」）が waiver 検証を pass して解消されうる。waived が成立するかは種別ではなく、author された理由に対する verdict の判断による（D3 / D4）。1つの anchor の約束は、cite する entry ごとに関わる部分が異なり得る（例: 「拒否し、かつ記録する」という anchor に対し、guard entry に関わるのは拒否、telemetry entry に関わるのは記録）。したがって、anchor が entry E2 の edge で fulfilled になっていても、それは「約束のうち E2 に関わる部分が E2 のテストで検証された」ことしか意味しない。別の entry E1 が同じ anchor を cite しているなら、「約束のうち E1 に関わる部分」は edge (E1, anchor) 自身の verdict でしか解消されない——他の edge の結果を理由に edge の評価を省略してはならない（anchor 単位への縮約の却下は Rejected E）。どの entry からも cite されていない `AC` / `CN` は「edge ゼロ」として決定論的 finding とする（範囲の根拠は D3）。コストは edge 単位の hash 凍結（D6）で抑える。

**保証境界**: 本ゲートが保証するのは「存在する edge の全数会計」と「anchor 単位の完全不参照（edge ゼロ）の検出」であり、「特定の entry が関連 anchor へ edge を伸ばしていない」という **citation の完全性は保証しない**——それは chain②（catalogue → spec）の執筆品質であり、Phase 2（type-designer）の責務領域である。この境界は fulfillment 判定の **edge 局所性**（D6）の帰結でもある。例: anchor A が「拒否する（entry E2 の責務）、かつ記録する（entry E1 の責務）」という2つの振る舞いを約束し、E2 だけが A を cite している（E1 の edge が欠けている）とする。edge (E2, A) の判定対象は「A の約束のうち E2 に関わる部分（拒否）」であり、拒否が検証されていれば pass する——「記録」は E2 の責務部分ではないため、この edge の fail にはならない。そして「記録」の担い手であるべき E1 の edge は存在しないので評価自体が発生しない。つまり**部分的に claim された anchor の、未 claim 部分は本ゲートの視界の外**である（edge ゼロ警報は完全非引用の AC / CN しか拾わない）。この盲点の治療は citation 完全性の検証（Reassess When 参照）であり、日常的には Phase 2（type-designer）の執筆品質と chain② のレビューが担う。

**waiver 検証の reject は義務を obligations に直接追加しない**。obligations は (catalogue, spec) からの決定論的射影（導出物）であり、verdict 発の義務を注入すると再導出で消える幽霊義務になるか、それを守る persist / merge 政策（D1 で排除した複雑性）が必要になる。reject の理由は routing 先への差し戻し brief に同梱するにとどめ、義務そのものは上流成果物の修正（宣言追加 / cite 追加 / 決定表 config の修正）後の再導出（`derive` 再実行）でのみ出現する。修正後は当該 entry から義務が導出されるため、この edge は以後「免除（waived）」ではなく「導出された義務の履行（テスト + fulfillment 検証）」で解消される——宣言修正の場合は entry 宣言 hash の変化により元の waiver pair も自然に失効する。

### D5: 多重 cite の扱い — 義務は edge 間で dedup しない

同じ anchor を複数の entry が cite し、それぞれから義務が立つ場合、義務はすべて維持し、**edge をまたぐ dedup は行わない**。義務の同一性には entry が含まれる（テストの対象はその entry 自身）ため、種別ラベルが同じでも別 entry の義務は対象コードが異なる別物であり、そもそも重複ではない。仮に dedup すれば「ある edge の義務が、別 entry を対象とするテストで解消された」ことになり、Rejected E と同型の silent-merge（別々の約束を1つの verdict に合流させる）が再発する。異なる義務種別（例: usecase 結果テストと port 契約テスト）で同じ意味内容を検証することも冗長ではなく多層防御である。

### D6: obligation-fulfillment ゲートは ref-verify の型に相乗りせず、独立の二階検証として設計する

本ゲートを既存 ref-verify の型（pair 型・chain filter・cache scope・verdict artifact）に**組み込まない**。理由は階数の違いによる構造差である。

| 観点 | ref-verify（一階） | obligation-fulfillment（二階） |
|---|---|---|
| 検証対象 | SoT chain の隣接ノード間 citation | chain② の edge（約束）に対する witness（テスト）の履行 |
| claim | SoT ノード断片（spec 要素 / catalogue entry） | test pair は bind されたテスト関数本体の集合、waiver pair は author された waived 理由散文（いずれも SoT ノードではない bindings / 実装側の人工物） |
| evidence | SoT ノード断片 | **edge**（entry 宣言 × anchor 本文の対） |
| 対象集合 | 人が author した citation | 決定表から**導出**された義務集合（entry 単位） + edge 会計の totality（D4） |
| キャッシュキー | (claim_hash, evidence_hash) の対 | test pair は (bound tests 集合 hash, entry宣言 hash, anchor hash)、waiver pair は (waived理由 hash, entry宣言 hash, anchor hash) の**三つ組** |
| fail routing | claim ノードの writer へ一意に差し戻し | テスト不備・理由不備→implementer / cite・宣言不備→type-designer / anchor 検証不能→spec-designer に分岐。決定表の欠陥は非ブロッカー（利用者所有の config の編集、または自発的 binding で解消・D4） |

**判定は edge 局所である**: fulfillment 判定の対象は「anchor の約束のうち、当該 entry の宣言に関わる部分」であり、他 entry の責務に属する部分の未検証を fail 理由にしない（それは当該他 entry の edge の問題であり、その edge が存在しない場合は D4 の保証境界の外）。判定器は他の edge の存在・不在を前提にしない（waiver 検証も同様: waived 理由は edge 局所で自己完結していることが求められる・D4）。

判定の規律は既存の意味論検証と共通にする: 合格 verdict には evidence のどの記述が裏付けかの引用を必須とする。fail 類型は「テストと anchor の意味的な関係」に応じて次の3つとする。以下の例は anchor が「全エントリ🔵なら pass / todo の🟡は無影響 / 🔴は常時 block」という3分岐を約束している場合。

- **(a) 矛盾**: anchor の約束と逆のことを assert している。例: 「🔴があっても pass する」ことを検証するテスト
- **(b) すり替え**: anchor を cite しながら、無関係な内容を検証している。例: この anchor を cite しつつ、gate 結果の JSON シリアライズ形式だけをテストしている
- **(c) 中心部の未検証**: 矛盾も無関係もないが、anchor が約束する中心的な振る舞いが検証されないまま残っている。例: 「全🔵→pass」の正常系だけをテストし、「🔴常時 block」「todo の🟡無影響」を検証するテストが bind されたテスト群のどこにもない

(c) を独立の類型として持つ理由: (a)(b) がなくても、正常系テスト1本で「anchor を検証した」ことにされる抜け道が残るからである。

(c) の判定対象は、anchor 本文のうち**「条件 → 観測可能な結果」の対として読める記述（振る舞いの主張）**に限る。「🔴は常時 block」「orphan 検出時にブロックする」はこの形で読めるため対象であり、理由の説明（「〜のため」）・例示（「例: …」）・背景記述は振る舞いの主張ではないため assert の対象に数えない——過剰要求でゲートを形骸化させないための操作的な線引きである。境界例の判定に残るブレは、semantic-verdict core の規律——裏付け引用の必須化と calibration probe（既知の誤り例の注入）による較正——で統制する（D7）。

**鮮度と verdict の結合（fail-closed）**: verdict はキャッシュキーの hash 組に凍結され、いずれかの hash が変化した時点（= drift 鮮度系の `spec_changed` / `decl_changed` / `test_changed` / `reason_changed`・D9）で既存 verdict は無効となり「存在しない」ものとして扱う。`check` ゲートの合格条件は「全 edge が bindings 上で解決宣言され（tests または waived・D9）、現行 hash と一致する fulfilled / waived verdict が存在すること」であり、hash 再計算だけで鮮度を回復する経路はない——回復は再評価（`evaluate`）の pass のみで達成される。

スコープ解決は existence-based 原則に従う: obligations / test-bindings artifact（D9）が不在なら zero pairs で通過し、部分的・不整合な存在（binding だけ存在する等）は fail-closed とする。

### D7: 責務中立な semantic-verdict core の抽出を先行させる

既存 ADR（sot-chain-semantic-review-gate D1）は「将来3つ目の検証器が出たら責務中立な core を抽出する」と予見していた。本ゲートが3つ目の検証器である。よってコピペで3系統目を作らず、次の責務中立部品を core として抽出し、ref-verify と obligation-fulfillment の両方が乗る形を先行タスクとする:

- verdict 語彙 + 裏付け引用必須の規律（だんまり通過禁止）
- hash 凍結キャッシュの機構（キーの型は検証器ごとに定義）
- calibration probe の注入と劣化検出
- fast → final → 人間 の段階引き上げドライバ
- capability → provider 解決（既に共有済み）

検証器固有に残すのは: pair / 義務の型、対象集合の生成（citation 走査 vs 決定表導出）、スコープ解決、fail routing。DRY 債務を増やす新規系統のコピペ実装は取らない。

### D8: 命名体系 — 検証は「リンク検証」と「被覆検証」の2族に分類する

**リンク検証（一階・位置番号は SoT chain のリンク位置 ⓪〜③）**: 既存の `signal calc/check-*` および ref-verify（spec-adr / catalog-spec）の呼称は維持する。本ゲートは機能的に第三リンクの意味論検証を担うが、二階検証（D6）であるため **`chain3` の呼称は採用しない**。ゲート名は **obligation-fulfillment**（導出されたテスト義務が果たされているかの意味論検証）とする。ゲートの主語は義務であり、coverage（存在の充足）/ fulfillment（内容の充足）の対で被覆検証族と語彙が揃う。

**被覆検証（completeness ゲート族・`*-coverage` で命名統一）**:

- `task-coverage` — spec 要素 ⊆ tasks（既存・既名）
- `contract-coverage` — catalogue entry ⊆ task-contract + 🔵信号（既存 PreReviewGate の被覆検査。**従来未命名だったものに本 ADR で命名を与える**）
- `obligation-coverage` — 全 ref edge ⊆ 導出義務（テスト + 検証） ∪ waiver（本 ADR で新設）

capability は問いの種類ごとに分離する: obligation-fulfillment 検証（「テスト群が anchor の約束を検証しているか」: コード→自然言語比較）と waiver 検証（「author された waived 理由がこの edge に対して成立しているか」: 自然言語→自然言語比較）は別 prompt / 別 capability とする（chain 別 prompt 分離と同じ論理）。

### D9: binding artifact 方式 — ソースコードに marker を埋め込まず、artifact をキーにソースを走査する

テストと義務の結び付けはソースコード内の marker コメントではなく、track artifact に記録する。導出物と author 物を分離する既存パターン（impl-plan.json + task-coverage.json）に揃えて2ファイルとする:

- **obligations（導出物・ツールが書く）**: 義務一覧（義務 id・対象 entry・義務種別・宣言内項目識別子・brief・entry宣言 hash）。anchor / anchor hash は義務 id と義務一覧の同一性には含めず、現行 `spec_refs` から edge 会計側で解決する
- **test-bindings（implementer が author）**: 解決宣言の写像。3形態を持つ——義務 id → `[{layer, module_path, test_name}]`（導出義務の履行。1義務に複数テスト可。このテスト群は対象 entry の各 edge に対する fulfillment claim 候補になる）/ edge id → `{waived: "理由の散文"}`（免除）/ edge id → `[{layer, module_path, test_name}]`（**自発的 binding**: 義務が導出されていない edge への直接テスト結束。fulfillment 検証は導出義務の場合と同一・D4）。**全 edge がいずれかで解決宣言されていること（totality）を check が決定論的に検査する**

**id の安定性 — id は同一性から、鮮度は hash から**: 義務 id は (entry 識別子, 義務種別, 宣言内項目の識別子〔invariant 名・メソッド名・handles の型名等〕) のみから決定論的に構成し、edge id は (entry 識別子, anchor id) から構成する。**anchor id を義務 id に含めない**ため、D2 の「義務数は cite 数に乗算されない」を id 体系でも保てる。anchor 本文の hash は義務 id ではなく edge ごとの verdict cache key（D6）にだけ入る。**内容 hash を id に含めず**、項目の識別に位置（index）を使わない。これにより、上流の内容変更（宣言の修正・anchor 本文の変更）は id を変えず——binding は生き残り——鮮度失効（`decl_changed` / `spec_changed`）として再検証だけを引き起こす。id が変わるのは同一性そのものが変わったとき（項目の改名・削除等）に限られ、そのときは `orphaned` / `missing` drift が binding の正当な追随を要求する。

走査は binding をキーに対象 crate のテスト関数を特定し、本体を span 抽出して obligation-fulfillment 検証の claim に供する。テストソースは素の Rust のままで異物構文を持たず、hash 類はすべて artifact / verify cache 側に閉じる（鮮度更新でソースを編集しない）。

drift 分類は次の6種で、**2族に分かれる**（D1 の結合構造に対応）:

- **存在系**（独立の決定論検査）: `missing`（義務に binding がない、または binding の指すテストが存在しない — テストの rename もここで検出される）/ `orphaned`（binding はあるが義務が導出されない）
- **鮮度系**（verdict の失効の表示名。キャッシュキーの各成分に対応する）: evidence 側の `spec_changed`（anchor 本文の hash 変化）/ `decl_changed`（entry 宣言の hash 変化）、claim 側の `test_changed`（bound test 本体の hash 変化）/ `reason_changed`（waived 理由散文の hash 変化）。解消は hash 再計算ではなく再評価（`evaluate`）の pass のみ（D6）

CI は書き換えなしに drift を検出できる。

### D10: 決定表は config ファイルとして repo に置く — 完全性は fail-closed のロード時検証で守る

義務導出の決定表（role → 義務生成ルール）を sotp の Rust ソースに hard-code せず、**`.harness/config/` 配下の config ファイル**（例: `.harness/config/test-obligation-rules.json`）として持つ。ゲートの policy を機械可読 config で持つ点で `.harness/config/review-scope.json` と同族であり、capability 割り当ての `.harness/config/agent-profiles.json` と同じく利用者所有の SSoT である。SoTOHE は推奨 default を template として出荷し、**利用者が自 repo の表を所有する**——表の欠陥や自プロジェクト向けの調整は、tooling の修正を待たず利用者側の config 編集で解決できる（責任境界: 表 = policy は利用者、導出エンジンと語彙 = mechanism は template）。

<!-- illustrative, non-canonical -->
```json
{
  "ValueObject":  { "obligations": [{ "kind": "boundary", "per": "invariant" }] },
  "UseCase":      { "obligations": [{ "kind": "result",   "per": "handles", "min": 1 }] },
  "Dto":          { "obligations": [] }
}
```
<!-- illustrative, non-canonical -->

**完全性保証**: `derive` / `check` はロード時に、config が全 role（DataRole 17種 / ContractRole / FunctionRole——role enum は Rust 側が SSoT）を被覆し、義務 0 件も `"obligations": []` と**明示**していることを fail-closed で検証する。sotp に新 role が追加されると既存 config は loud に fail し、「生成数の決め忘れ」は依然として構造的に不可能である（コンパイル時網羅 match による保証の、config ロード時への移設）。

**語彙**: 生成ルールは per 軸（`invariant` / `method` / `handles` / `reacts_to` / `transition` / `trait_method` / 定数——宣言 payload のフィールドに対応する閉集合）× 義務種別 × brief テンプレートで表現する。語彙の解釈器は Rust 側で per 軸への網羅 match として実装し、**語彙の拡張のみが template 上流の責務**となる。

### D11: CLI surface — fail-closed、lenient / force 系フラグなし

単一の `sotp test-obligation` グループに、義務ゲートの lifecycle を構成する4サブコマンド（導出 / 被覆ゲート / 意味論評価 / verdict 閲覧）を置く。ここで数える4件は pass/fail gate 面とその verdict 閲覧面のコマンドであり、D14 で追加する同グループの authoring helper `bindings-skeleton` はこの4件にも gate 面にも含めない。命名は機能の直接表現とする（他コマンド群からの類推借用をしない）:

| 役割 | コマンド | 命名根拠 |
|---|---|---|
| 義務導出（obligations artifact の書き込み） | `sotp test-obligation derive` | D2 の導出関数 `derive: 宣言 → [義務]` の形式語彙そのもの |
| 被覆ゲート（obligation-coverage: 解決宣言の totality + 全 edge に現行 hash 一致の verdict が存在すること。pure-read、CI / commit gate 用） | `sotp test-obligation check` | 決定論の読み取りゲート（`task-contract check` / `signal check-*` と同じ用法） |
| 履行の意味論検証の実行（test pair + waiver pair） | `sotp test-obligation evaluate` | LLM による意味論評価という操作の正確な表現。`verify` は既存の `sotp verify *`（決定論検査群）と意味が衝突し、`run` は「テストを実行する」との誤読余地があるため、いずれも不採用。CLI surface への新動詞だが、正確さを優先する（なお本コマンドはテスト本体を静的に読むのであって実行しない） |
| verdict 閲覧 | `sotp test-obligation results` | `ref-verify results` と同じ用法 |

lenient / force 系フラグは採用しない — remove-lenient-and-force-flag-paths の既決方針に従い、解決不能な義務・不整合は finding + 非零 exit とし、治療は上流成果物（カタログ / spec / binding）側の修正で行う。スコープは existence-based に自動解決し、scope 指定フラグを持たない。

### D12: default config は全 role を一括定義して出荷する — role 段階導入はしない

template が出荷する default の決定表 config は、全 role（DataRole 17種 / ContractRole / FunctionRole）の生成ルールを本機能の導入 track で一括して定義する。これは選択ではなく D10 の帰結である: ロード時の totality 検証は全 role の明示宣言なしに `derive` / `check` を通さず、「一部 role だけ先に対応する」段階導入は、未対応 role に仮の `"obligations": []`（0 件）を置く＝**嘘の導出**を制度化することでしか実現できない。それは waiver レーンに大量の誤評価を流し込む——D4 が排除した経路を導入初期に自ら作る行為である。

role 1つ分の実体は config の1エントリ + brief テンプレートであり（コード生成なし・D1）、17 role 分でも単一 track で完結する判断作業である。大半の role は `"obligations": []` として明示的に決着する。track 単位の適用は existence-based scope（D6）が自然に制御する: obligations artifact を持つ track だけでゲートが発火するため、機能側に導入順序の概念は不要。

### D13: role 別義務生成の初期決定表（default config の出荷内容）

義務は entry 単位で導出され（D2）、件数は宣言の内部構造のみの関数である。template が出荷する default config（D10）の初期内容を、config そのものの形で以下に定める（正本は出荷される `.harness/config/test-obligation-rules.json`。brief テンプレートは省略）:

<!-- illustrative, non-canonical -->
```json
{
  "data_roles": {
    "ValueObject":     { "obligations": [ { "kind": "boundary",               "per": "invariant" } ] },
    "Entity":          { "obligations": [ { "kind": "invariant_preservation", "per": "invariant" } ] },
    "AggregateRoot":   { "obligations": [ { "kind": "invariant_preservation", "per": "invariant" },
                                          { "kind": "event_emission",         "per": "emits" } ] },
    "DomainService":   { "obligations": [ { "kind": "logic_result",           "per": "method" },
                                          { "kind": "event_emission",         "per": "emits" } ] },
    "Specification":   { "obligations": [ { "kind": "predicate_both_branches", "per": "entry" } ] },
    "Factory":         { "obligations": [ { "kind": "construction_result",    "per": "entry" } ] },
    "UseCase":         { "obligations": [ { "kind": "result", "per": "handles", "min": 1 } ] },
    "Interactor":      { "obligations": [ { "kind": "result", "per": "entry" } ] },
    "EventPolicy":     { "obligations": [ { "kind": "reaction", "per": "reacts_to" } ] },
    "Command":         { "obligations": [] },
    "Query":           { "obligations": [] },
    "Dto":             { "obligations": [] },
    "DomainEvent":     { "obligations": [] },
    "ErrorType":       { "obligations": [] },
    "SecondaryAdapter": { "obligations": [] },
    "CompositionRoot": { "obligations": [] },
    "PrimaryAdapter":  { "obligations": [] }
  },
  "contract_roles": {
    "SecondaryPort":      { "obligations": [ { "kind": "contract", "per": "trait_method" } ] },
    "SpecificationPort":  { "obligations": [ { "kind": "contract", "per": "trait_method" } ] },
    "Repository":         { "obligations": [ { "kind": "contract", "per": "trait_method" } ] },
    "ApplicationService": { "obligations": [ { "kind": "result",   "per": "trait_method" } ] }
  },
  "function_roles": {
    "UseCaseFunction": { "obligations": [ { "kind": "result", "per": "entry" } ] },
    "FreeFunction":    { "obligations": [ { "kind": "logic",  "per": "entry" } ] }
  },
  "patterns": {
    "typestate": { "obligations": [ { "kind": "transition", "per": "transition" } ] }
  },
  "trait_impls": {
    "SecondaryPort":      { "obligations": [ { "kind": "contract_conformance", "per": "trait_impl" } ] },
    "SpecificationPort":  { "obligations": [ { "kind": "contract_conformance", "per": "trait_impl" } ] },
    "Repository":         { "obligations": [ { "kind": "contract_conformance", "per": "trait_impl" } ] },
    "ApplicationService": { "obligations": [] }
  }
}
```
<!-- illustrative, non-canonical -->

**補足根拠**（0 件・非自明な行）:

- `ValueObject`: invariants **非空**なら invariant ごとの境界テスト（valid 受理 / invalid 拒否）。空なら実行時境界が存在せず（型が不正状態を表現不能にしている）、per 軸が空集合なので自然に 0 件
- `Entity` / `AggregateRoot`: invariant の**維持**テスト（`&mut self` メソッド通過後も保たれる）。identity の同値性は derive の領分で対象外
- `event_emission`（emits 宣言からの一律生成）: 義務の正当性の源泉は anchor ではなく**宣言そのもの**である（catalogue は SoT ノードであり、emits 宣言は「このイベントを発行する」という型契約）。anchor が emission に言及しなくても義務は正当で、宣言が余計な場合の治療は waiver ではなく **type-designer による宣言の削除**（`derive` 再実行で義務が自然消滅——「宣言の不足」の双対としての「宣言の過剰」）。なお「emission を語る anchor が1本もないのに emits が宣言されている」状態は、本ゲート以前に chain②（catalogue → spec）の意味論検証が fail 類型「evidence に記載のない新規の behavioral commitment」として拒絶する領分であり、chain② を通過した emits 宣言には裏付け anchor が存在する——その edge の fulfillment 評価で emission テストの意味論品質も検証される（宣言の接地は chain②、履行の検証は本ゲート、という多層防御の分担）
- `DomainService`: メソッドごとのロジック結果テスト（DDD の定石どおり小さく、実カタログでも 1〜4 メソッド。trait 側の per-`trait_method` と一貫）
- `Specification`: 述語の両分岐（満たす / 満たさない）を1義務で要求
- `UseCase` の `"min": 1`: handles 宣言が空でも result 義務を最低1件立てる
- `Command` / `Query` / `Dto` / `DomainEvent` / `ErrorType`: データ運搬体。ErrorType の「条件→variant」は返す側の義務。Command に検証があるなら VO として宣言すべきで、その宣言不足は waiver 検証の reject（D4）が検出する
- `SecondaryAdapter`: 型としては 0 件。契約適合は `trait_impls` セクションが義務化する（port 系 trait への impl 1件につき、契約 harness をその impl で実行する適合義務 +1）。`trait_impls` も他セクションと同形の「trait role をキーにした map」であり、ContractRole 全 variant の明示が totality 検証で強制される。`ApplicationService` の impl を 0 件とするのは、result 義務（trait 側 per-`trait_method`）が通常唯一の impl を直接対象にするため。契約適合義務は default で有効とし、不要と判断した利用者は自 repo の config で当該 role を `"obligations": []` に編集して無効化する（D10 の所有権の帰結。default は安全側に倒す）
- `CompositionRoot` / `PrimaryAdapter`: wiring はコンパイル + arch ゲートの領分

**config に載らない導出エンジン側の規則**（D10 の mechanism 側）:

- **trait_impls の rule 解決**: trait_impl entry は identity-only（role を持たない）なので、`trait_impls` セクションの適用は `trait_ref` を track の層別カタログ中の TraitEntry に解決し、その ContractRole をキーとする（port trait = domain 層、impl = infrastructure 層という層跨ぎ解決を含む）。外部 trait（`From` / `Display` 等）などカタログに解決できない `trait_ref` の impl は**明示的に義務 0 件**とする
- **契約適合義務の evidence**: trait_impl entry は spec_refs を持たず自身の edge がないため、契約適合義務（`contract_conformance`）の fulfillment 評価は**実装対象 trait の edge**（trait entry × その cite する anchor）を evidence に用いる
- **メソッド単位 spec_refs がある場合**: 義務の粒度はメソッドへ精緻化されるが、anchor には乗算しない。メソッド単位の義務に bind されたテスト群を、そのメソッドから伸びる各 edge の fulfillment claim として edge ごとに評価する
- **ItemAction**: 義務を導出するのは `Add` / `Modify` の entry のみ。`Reference` は「この track の変更面ではない」ため edge 宇宙の外とする（**定義上の境界**であり silent drop ではない——referenced item の義務はそれを Add した track に属する）。`Delete` は 0 件で、binding の残骸は orphaned drift が掃除を要求する

### D14: `bindings-skeleton` 執筆補助サブコマンドの追加と `results` の informational 性質の明文化

D11 が定めた4サブコマンド（`derive` / `check` / `evaluate` / `results`）は `sotp test-obligation` グループ内の義務ゲート lifecycle surface である。このうち `derive` / `evaluate` は gate が読む artifact / verdict を生成し、pass/fail gate としての判定は `check` が担い、`results` は下で明文化する通り informational な閲覧コマンドである。これに加えて、同じ CLI グループに5つ目のコマンドとして、pass/fail gate 面から独立した**執筆補助サブコマンド** `sotp test-obligation bindings-skeleton` を1つ設ける。

**bindings-skeleton の契約**:

- **入力**: track の `obligations.json`（`derive` の出力＝導出義務一覧）
- **stdout**: `test-bindings.json` と**同一 wire 形状**の schema 準拠 draft を1度だけ出力する。各導出義務 id を `fulfillment` レコードとして事前充填し、テスト位置（`layer` / `module_path` / `test_name`）は TODO placeholder のまま。stdout は fail-closed codec が受理する key のみを含む **schema-pure** な形に保ち（`deny_unknown_fields` が未知 key を reject する）、利用者は stdout をそのまま `test-bindings.json` として materialize し値だけ置換して仕上げられる
- **stderr**: 案内文（レコード件数、TODO 差し替え / waiver・自発的 binding への転記の指示、brief と対象 entry は `obligations.json` を参照するという pointer）を1度だけ出力する。stdout の schema-purity を汚染しないための stream 分離
- **副作用の完全な不在**: `test-bindings.json` を書かない、verdict cache を書かない、`obligations.json` も含む repo file を書き換えない、gate verdict に影響しない。command は **read-only**
- **fail-closed セマンティクスの不変性**: TODO placeholder が残る間は当該 binding が実在テスト関数を指せないため、`check` が missing drift として fail-closed で reject する。skeleton の生成は義務・履行・鮮度いずれの検証も肩代わりせず、D11 の fail-closed gate と D6 の鮮度 → verdict 結合を弱めない
- **gate 面ではない**: `bindings-skeleton` は D11 の4サブコマンドにも pass/fail gate にも含まれない。CLI grouping としては同じ `sotp test-obligation` グループに置くが、gate ではない本コマンドは fail-closed verdict 判定の対象外である。CI からは呼ばれない

**`results` の informational 性質の再確認**（D11 の該当行の明文化）: `sotp test-obligation results` は verdict 閲覧のみを目的とし、**exit は常に 0**（informational）である。verdict の合否判定＝ゲートとしての pass/fail は **`check` の責務**であり、`results` は結果の可視化に専念する（既存 `sotp ref-verify results` / `sotp review results` / `sotp dry results` と同じ用法）。verdict gate と閲覧の責務分離は、rerun 時の副作用不在（`results` の複数回実行は等価な出力を返すのみ）と、CI の pass/fail 経路の唯一性（gate は `check` のみ）を同時に保つための構造判断であり、`results` に非 0 exit を持たせない選択はこの構造の帰結である。

**動機の grounding**:

- **手写しコストの構造的解消**: 本ゲートが導出する義務 id 群は role 別導出ルール（D2 / D10）の出力であり、実装が AI 前提の本プロジェクトでも、義務 id 文字列そのものの機械的な複製（正確な区切り・エスケープ・全 id の網羅）を implementer に強いる合理性がない。skeleton は義務 id boilerplate の生成を機械化し、転記過誤（typo / 欠落 / 順序）を構造的に排除する
- **implementer authored の binding 原則の保持**: D9 の binding は「implementer が author する」設計（テスト位置 / waived 理由 / 自発的 binding のいずれも人が書く）。skeleton は**空欄（TODO placeholder）だけを生成**し、テスト位置の意味的な決定は依然として implementer に委ねる——事前充填されるのは形（同一 wire 形状 + 網羅された義務 id）のみで、内容（どのテスト関数を bind するか）は空である。この分業は Rejected A（テスト本文自動生成）との境界を保つ: 呼び出し部・import・雛形は生成せず、生成対象は binding artifact の「空きスロット」の網羅にとどまる

**位置づけの明確化**: bindings-skeleton は fail-closed gate 面（D11）でも意味論検証 core（D7）でもなく、**執筆補助レイヤ**である。D6 の鮮度 → verdict 結合と D11 の fail-closed セマンティクスは、この executable の存在によっても弱まらない（skeleton は draft を stdout に出すだけで、gate 面の pure-read verdict / hash 凍結キャッシュには一切触れない）。gate の完全性を担保するのは依然として `check` の decision（現行 hash と一致する verdict の全 edge 存在）であり、skeleton は**そこに至るための implementer 側の boilerplate コストを削減する装置**である。

## Rejected Alternatives

### A. テストコード本文の自動生成を含むフルパイプライン

義務の導出だけでなく、テスト関数の本文（呼び出し部・import・雛形）まで決定論的に生成する案。カタログの型参照を実際の Rust パスへ解決する仕組みと、生成コードと人の編集を同一ファイルで共存させる管理（再生成時に人の記述を壊さない上書き制御）が必要になり、機能全体で最大の実装・保守コストを占める。実装者が AI である本プロジェクトでは、brief（対象 + anchor + 形）があれば呼び出し部の執筆は自明であり、限界価値がコストに見合わない。却下（D1）。

### B. role 起点の一軸マッピング

role からテスト種別へ直行するマッピング。実カタログの検分により、(1) 振る舞いの記述は outcome 型（義務 0 件の ValueObject）が cite する anchor に載ることが多く、role 単独では義務の在り処を外す、(2) テストの価値を決めるのは anchor の記述の質である、と判明した。宣言（件数と種類）と anchor（意味）を直交させる二軸モデル（D2）に転換。却下。

### C. 義務 0 件の edge を黙って落とす

導出結果 0 件を決定表の帰結として静かに落とす案。「だんまり通過を許さない」原則に反し、決定表の漏れ・カタログの宣言漏れが検出不能のまま沈む。waiver verdict 必須の被覆ゲート（D4）を採用。却下。

### D. obligation-fulfillment を ref-verify の chain3 として実装する

既存の ref-verify pair 型 / chain filter / cache scope に第三種を追加する案。evidence がノードではなく edge であり、対象集合が author されず導出され、キャッシュキーが三つ組になり、fail routing が分岐する——一階と二階の構造差（D6)を単一の型に押し込むと、pair 型が2責務を抱え分岐が増える SRP アンチパターンになる（sot-chain ADR D1 が code-review 相乗りを拒否したのと同じ論理）。責務中立 core の抽出（D7）で共有すべきものだけを共有する。却下。

### E. waiver 評価を「義務に覆われていない anchor 単位」に縮約する

LLM コスト削減のため、他 entry の義務で覆われた anchor への義務 0 件 cite は waiver 評価を省略し、未被覆 anchor だけを anchor 単位で評価する案。一見合理的だが、別々の約束（edge）を1つの verdict に合流させるため、義務 0 件の entry の宣言に固有の anchor の側面が誰にも問われないまま消える——D4 の「黙って消える経路を作らない」原則に自己矛盾する。全 ref edge の評価（D4）を採用し、コストは edge 単位の hash 凍結で抑える。却下。

### F. role の段階導入（usecase-first / ValueObject-first）

一部の role から義務導出を始め、残りを後続 track で足す案。旧 draft（コード生成前提）では role ごとに emitter 実装の重量があり段階化に意味があったが、A+B+D スコープでは role 1つ分は config の1エントリ + brief テンプレートに縮んでおり、段階化の便益が消えた。一方でコストは残る: ロード時 totality 検証（D10）の下で未対応 role を表現するには仮の `"obligations": []`（嘘の導出）しかなく、waiver レーンの誤評価を制度化する。全 role 一括定義（D12）を採用。却下。

### G. waiver fail 時に LLM 提案の義務を obligations へ直接注入する

waiver fail の verdict が示唆するテストを、そのまま義務として obligations artifact に追記する案。obligations の導出物としての純度が壊れ（混合 writer）、再導出との整合には D1 で排除した persist / merge 政策が必要になる。さらに根本原因（宣言不足・cite 欠落・決定表の漏れ）が上流に放置され、SoT chain の外に幽霊義務が生まれる。verdict の示唆は routing 先への diagnostic にとどめ、義務は上流修正後の再導出でのみ出現させる（D4）。却下。

### H. waiver を LLM が author する裁定として設計する

waived の成立可否を LLM の verdict そのものに決めさせる案（免除理由も verdict が生成する）。LLM が「決める」ためには判断材料を運び込む機構が必要になり、担保（warrant）類型の構造化・同一 anchor を cite する兄弟 edge 一覧の供給・その fingerprint の凍結キーへの追加・warrant 参照先の check 時生存検証・reject 原因の消去法手順——と、fulfillment 本体に匹敵する機構群が連鎖的に生えた（検証器に hop 用クエリコマンドを与える変種も検討したが、payload-only 実行規律の破壊と provider 分裂を招くうえ、fingerprint 凍結の必要は消えない）。「人（implementer）が理由を author し、LLM は author 済みの理由を edge に対して検証する」という他レーンと同型の分業（D4）に置き換えることで、この機構群は丸ごと不要になる。却下。

### I. 決定表を Rust ソースに hard-code する（網羅 match によるコンパイル時完全性）

決定表を sotp 内の catch-all なし網羅 `match` として実装し、「新 role 追加で non-exhaustive エラー」というコンパイル時保証を得る案。保証としては最強だが、決定表は policy であり、その欠陥・調整の解決手段が「sotp ソースの修正」になる——テンプレート利用者は tooling 内部を修正できず（fork 逸脱・理解コスト・責任境界の侵犯）、track が template ドメインの欠陥でブロックされる。config 化 + ロード時 totality 検証（D10）は同じ fail-closed 保証を config ロード時に与えつつ、表の所有権を利用者に移す。網羅 match 自体は per 軸語彙の解釈器として Rust 側に残る。却下。

### J. テストと義務の結び付けをソース内 marker コメントで行う

テストブロックを `// sotp-test:begin id=... spec=... sig=...` 形式の marker で囲む案。テスト本体の抽出は自明になるが、(1) テストソースに異物構文が常駐し形式 lint が必要になる、(2) hash がソースに埋まるため鮮度更新のたびにソース編集が発生する、(3) 結び付け情報が source と track artifact に分散し SSoT が壊れる。binding artifact 方式（D9）を採用。却下。

## Consequences

### Positive

- SoT chain の全リンクが「構造 + 意味論」の二層検証で覆われ、設計→実装のトレーサビリティが振る舞いレベルまで到達する
- テスト義務が機械導出されるため、「書くべきテストが書かれていない」がCI で検出可能になる
- waiver verdict の引用必須化により、「テストしない」判断が担保方式の明文化として記録される
- 充填者（人/AI）を信頼する必要がない: gate は pure-read の verdict であり、DFP/RFP と同じ哲学に乗る
- テスト本文生成・上書き制御を持たないため、実装コストと将来の ceremony 増殖リスクが小さい
- semantic-verdict core の抽出（D7）により、既存 ref-verify との重複なしに3系統目が立つ
- 決定表が利用者所有の config（D10）になり、テンプレート利用者は tooling を修正せずに義務ポリシーを調整できる（architecture-rules.json と同じ policy/mechanism 分離）

### Negative

- LLM verdict のコストが増える（obligation-fulfillment pair + waiver）。hash 凍結キャッシュで全 cache hit 時コスト 0 を保つが、初回と変更時のコストは real
- 義務導出の決定表（宣言 → 義務一覧）自体が新たな保守対象になる。表の漏れは waiver reject や 自発的 binding の使用として顕在化し、利用者の config 編集で解決される（track はブロックされない・D4）が、顕在化までのラグはある
- 決定表の config 化（D10）により、生成ルール語彙の設計・解釈器・ロード時 totality 検証の実装コストが hard-code 比で増える
- brief の質が implementer の執筆品質を左右する。型レベル spec_refs のみの track では brief が粗くなる（対象関数の特定を implementer に委ねる）
- semantic-verdict core の抽出が先行タスクとして乗るため、機能単体の見積もりより track が大きくなる

### Neutral

- メソッド単位 spec_refs は必須化しない。粒度向上の任意手段として残る
- obligations / test-bindings / drift の語彙は本機能専用であり、既存の signal / ref-verify の語彙に変更はない
- テストの rename は drift（missing）として検出され、binding の更新で解消する（ソース側に追随義務はない）

## Reassess When

- 決定表（宣言 → 義務一覧）の漏れ起因の waiver reject が頻発した場合 — default config の生成ルールの再設計、またはメソッド単位 spec_refs の必須化を検討
- per 軸語彙（D10）で表現できない生成ルールが必要になった場合 — 語彙の拡張（template 上流の責務）を検討
- obligation-fulfillment / waiver の LLM コストが運用上支配的になった場合 — 評価単位・キャッシュ粒度・probe 率の再設計
- implementer の執筆品質が brief では安定しない場合 — テスト本文生成（Rejected A）の限定的再導入を検討
- semantic-verdict core に検証器別の分岐が漏れ始めた場合 — core の責務境界の再設計
- 型カタログの schema / role 語彙が大きく変わった場合（二軸モデルの前提）
- spec 要素の種別構成（goal / in_scope / constraints / acceptance_criteria）が変わった場合（D3 の prior / edge ゼロ警報範囲の前提）
- GO / IN への cite に由来する waived pair が肥大してコストを圧迫した場合 — 種別フィルタの限定的再導入（D3 の再検討）ではなく、まず prior の精度と cite 慣行の見直しで対処する
- waived 理由の執筆品質が安定せず、自己完結な理由だけでは edge を解消しきれない事例が繰り返し確認された場合 — 判定への文脈供給の拡充（Rejected H の機構の限定的再導入）を検討する
- 過去 track の bound test の黙った弱体化（assert の弱体化改変。削除は hook が防ぎ、diff は review が見るが、検出は機械化されていない）が実害として確認された場合 — track artifact の snapshot 原則を壊さない対策（例: 過去 bindings の read-only 走査による protected-test ガード + 契約の active track への所有権移転）を**別 ADR** で検討。本機能のカバー範囲は track 内のテスト義務の導出・検証であり、cross-track の回帰防衛は範囲外
- 現 track の Modify が過去 track の anchor の約束を意味的に破る事例（cross-track の振る舞い契約の非継続）が実害として確認された場合 — 過去 track の spec 要素に対する再検証レーンの追加を別 ADR で検討
- cite 漏れの見逃し（特定の entry が関連 anchor へ edge を伸ばしていない、または部分的に claim された anchor の未 claim 部分に担い手 edge がない）が実害として繰り返し確認された場合 — per-entry citation 完全性の意味論スイープ（全 entry × 全 anchor の関連性判定）、あるいは anchor 単位の被覆残余検査（anchor の約束全体が edge 群で覆われているかの集約判定）を別レーンとして追加するか検討（D4 の保証境界と D6 の edge 局所性の前提）

## Related

- `knowledge/adr/` — ADR 索引（sot-chain 意味論レビューゲート、ref-verify existence-based scope、pre-review task-contract gate、lenient/force フラグ排除、TDDD taxonomy / typestate 直交配置 / pattern semantics の各 ADR はここから辿る）
- `knowledge/conventions/tddd-product-correctness.md` — TDDD と実装正しさの分担
- `knowledge/conventions/testing.md` — テスト規約
- `knowledge/conventions/workflow-ceremony-minimization.md` — 「file 存在 = phase 状態」原則（スコープ解決が従う）
