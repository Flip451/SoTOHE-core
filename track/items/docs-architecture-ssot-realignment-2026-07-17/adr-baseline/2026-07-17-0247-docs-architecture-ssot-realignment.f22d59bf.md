---
adr_id: 2026-07-17-0247-docs-architecture-ssot-realignment
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 同梱運用ドキュメント監査 (同日実施) を受け、hexagonal-architecture.md の全面改訂ではなく廃止を選ぶ裁定"
    candidate_selection: "from:[retire-doc,slim-down-to-semantics,full-rewrite-to-6-crates] chose:retire-doc"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 usecase purity 規則の移設先を coding-principles.md とし、purity の Good/Bad 例のみ随伴移設・trait 例と async Note は廃棄とする裁定"
    candidate_selection: "from:[move-purity-to-coding-principles,new-thin-purity-convention,no-relocation] chose:move-purity-to-coding-principles + from:[relocate-purity-example-only,relocate-all-examples,discard-all-examples] chose:relocate-purity-example-only"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 監査 F-03/F-04 の是正方針 (cite 付け替えと更新チェックリストの SSoT 集合同期) を decision 化する裁定 (High+Medium 全 findings の decision 化を選択)"
    candidate_selection: "from:[structural-fixes-only,all-high-medium-findings,docs-ssot-reorg-only] chose:all-high-medium-findings"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 監査 F-05/F-06/F-07 の是正方針 (ポート配置記述の R1 マトリクス準拠統一) を decision 化する裁定"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 監査 F-08 の是正方針 (ValueObject の side-effect-free 導出メソッド許容) を decision 化する裁定"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 監査 F-11 の是正方針 (DRY の cross-layer 断定の是正とミラー型除外) を decision 化する裁定"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 監査 F-12 の是正方針 (入口ドキュメントの依存方向表記の一意化) を decision 化する裁定"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 監査 F-14 の是正方針 (overlay ADR テンプレートの現行 front-matter 形式化) を decision 化する裁定"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:session-01P6BqX8JsHL7ePVmPddtadn:2026-07-17 PR #197 CI 失敗調査を受け、release PR を明示免除し他 PR は fail-closed を維持する案を D9 として追記する裁定"
    candidate_selection: "from:[release-pr-exemption,head-ref-only-skip,tool-side-skip,pass-track-id] chose:release-pr-exemption"
    status: proposed
---
# 同梱運用ドキュメントのアーキテクチャ記述 SSoT 再編

## Context

テンプレート出力に同梱される運用ドキュメント全体（`template-boundary.json` の include 分類 + overlay 側 knowledge 文書、約 130 ファイル）を、クリーンアーキテクチャ / ヘキサゴナルアーキテクチャ / CQRS / SOLID / DRY の観点で 2026-07-17 に監査した。監査レポートは transient な作業領域の成果物であり永続参照先にならないため、本 ADR は判断に必要な事実（各 finding の内容と根拠）をすべて自立して記載する。検出は High 2 件・Medium 9 件・Low 5 件と改善提案 44 件で、High/Medium は全件、原文照合で検証済み。

監査が特定した構造的な根本原因は 2 つある。

1. **`knowledge/conventions/hexagonal-architecture.md` が delivery 3 crate 分割前の旧モデルのまま**取り残されていた。同文書は「CLI → domain, usecase, infrastructure（composition root）」という単一 CLI 前提の層依存表を掲げ、「CLI の非テストコードに `domain::` / `infrastructure::` への直接参照があっても良い」と明記していたが、出荷される機械可読 SSoT（`architecture-rules.json` + `deny.toml`）では `apps/cli`（bin）は `cli_composition` / `cli_driver` のみに依存でき、この記述に額面どおり従うと `cargo make deny` が失敗する（監査 F-01, High / F-02）。レビュープロンプトの cite 先も同文書の陳腐化した節を指し（F-03）、architecture-customizer スキルと maintainer checklist の更新対象リストに同文書が含まれないため、ドリフトが機構的に再発する構造だった（F-04）。ドリフトの本質は、機械可読 SSoT の内容を人間可読文書が再記述していたことにあり、これは `no-upstream-restatement.md`（SSoT の散文再記述の禁止）と `enforce-by-mechanism.md`（型 > CI > hook > lint > docs）が警告する形そのものである。
2. **ポート配置の 2 層規則（永続化・集約 → domain / アプリケーションサービス機能 → usecase）の記述が分散し、両極に崩れていた**。調査プロンプト 3 ファイルは「ポート = domain 層の trait」と断定し（F-05）、implementer の Architecture Guard は「ports は usecase」と要約し（F-06）、Repository の許可層は hexagonal-architecture.md と `type-designer-kind-selection.md` R1 マトリクスで食い違っていた（F-07）。同一規則が逆向きに 2 通り誤記されている事実は、規則の記述が単一の権威ある場所に置かれていないことを示す。なお R1 マトリクス（role × layer、draft 段階で違反を却下）は既にポート配置のより厳密な準機械ルールとして機能している。

このほか独立の問題として、ValueObject 規則が DDD の Value Object 定義と乖離し rich な値オブジェクトの表現経路を持たないこと（F-08, High）、DRY 修正 capability の「DRY violations cross layer boundaries by definition」という誤った断定が意図的ミラー型の統合＝層結合を誘発すること（F-11）、CLAUDE.md の層矢印が凡例なしで依存方向を反転して読めること（F-12）、overlay の ADR テンプレートが禁止形式（file-level `## Status`・front-matter 欠落）のまま出荷され、従うと day-one で CI が fail すること（F-14）が確認された。

また、本 ADR の起草後に PR #197（develop → main の release PR）の CI 失敗を調査した結果、`.github/workflows/ci.yml` の track-aware gate step の実行条件が pull_request イベントであることのみを見ており、gate が前提とする track/<id> branch 文脈の有無を見ていないことが判明した。前段の track branch 再作成 step は「pull_request かつ head が track/*」の両条件を持っており、同一条件が 2 箇所に手書きされて片側だけが更新される条件ドリフト — 根本原因 1 と同型 — である。release PR では gate 先頭の `sotp task-contract coverage` が設計どおり fail-closed し（`Makefile.toml` の `ci-track-container` は「Skip on base branch / non-track CI runs」と skip を workflow 境界に置く意図を明記している）、release PR の CI が構造的に green にならない。

本 ADR は、これらのうち High / Medium 全 11 件の是正方針（D1〜D8）と、監査後に判明した上記 CI 条件ドリフトの是正（D9）を決定として定める。Low 5 件と改善提案 44 件は本 ADR の決定対象外とし、実装 track のタスクまたは後続判断に委ねる。

## Decision

### D1: hexagonal-architecture.md を廃止し、層・配置の SSoT を機械可読ルールに一本化する

`knowledge/conventions/hexagonal-architecture.md` を削除する。全面改訂（6 crate 化）ではなく廃止を選ぶ。以後の SSoT は次の 2 つに一本化する:

- **crate 間の層依存**: `architecture-rules.json`（`may_depend_on`）+ `deny.toml`。`cargo make check-layers` / `cargo make deny` が機械強制し、可視化は `bin/sotp arch tree` が担う。
- **role × layer の配置（ポート配置を含む）**: `knowledge/conventions/type-designer-kind-selection.md` の R1 マトリクス。draft 段階で違反を却下する準機械ルールであり、SecondaryPort（domain / usecase）・Repository（domain のみ）・ApplicationService（usecase のみ）・SecondaryAdapter（infrastructure のみ）・CompositionRoot（cli_composition のみ）・PrimaryAdapter（cli_driver のみ）を既に規定している。

これにより監査 F-01 / F-02（陳腐化した層記述そのもの）と F-07（hexagonal 文書と R1 の Repository 配置矛盾）は、誤った側の記述の消滅として解消される。再記述構造を残さないことで、同型ドリフトの再発を構造的に断つ。

### D2: 残余コンテンツは usecase purity 規則のみ coding-principles.md へ移設し、他は廃棄する

hexagonal-architecture.md のうち機械可読ルールに還元できないコンテンツの処遇を次のとおりとする:

- **Usecase Layer Purity Rules**（std I/O モジュール・暗黙的外部依存・出力マクロの禁止表と rationale、および usecase の Good/Bad コード例）は `knowledge/conventions/coding-principles.md` へ節として移設する。機械強制は従来どおり `sotp verify usecase-purity` が担い、移設節には強制強度（現状 warning-only であること、error 昇格は採用者判断であること）を consumer 向けの表現で明記する（監査で指摘された、強制強度が内部チケット参照でしか読めない問題の同時解消）。
- **Trait-Based Abstraction 節（port trait + adapter 実装のコード例）と async 採用 Note は廃棄する**。role 配置は R1、型安全パターンは `prefer-type-safe-abstractions.md`、レビュアー向けの層別規範は `.harness/custom/review-prompts/*.md` が既にカバーしている。async 採用が ADR 決定事項であるという原則のみ、coding-principles.md に一文残す。
- ポート配置の判別基準（境界例の tie-break）は D4 で R1 側に追記する。

### D3: 参照の付け替えを行い、アーキテクチャ変更時の文書更新チェックリストを SSoT 集合と同期させる

hexagonal-architecture.md への参照（レビュープロンプト各所の「Cite `hexagonal-architecture.md` §...」、CLAUDE.md / AGENTS.md / README.md / `.claude/rules/` / architecture-customizer スキル等、および Rust ソースの doc comment に残る同規約への参照 — `libs/domain/src/spec_document_loader_port.rs` の port 配置根拠説明等の数箇所）を、D1 の新 SSoT（`architecture-rules.json` / kind-selection R1）または D2 の移設先（coding-principles.md の purity 節）へ付け替える。cite 先の節に主張が存在しない引用ずれ（監査 F-03）はこの付け替えで一括解消する。

再発防止として、`architecture-customizer` スキルの Update Documentation ステップと `.claude/rules/09-maintainer-checklist.md` の更新対象リストを、アーキテクチャ記述を持つ文書の実集合（本 ADR 適用後の集合）と一致させる（監査 F-04 の解消）。以後、層構成の変更時に更新すべき文書がチェックリストから漏れない状態を維持する。

### D4: ポート配置の記述を R1 マトリクス準拠に統一し、R1 に判別基準を追記する

ポート配置に言及する全ドキュメントを R1 の 2 層配置（永続化・集約ポート → domain / アプリケーションサービスが必要とする機能ポート → usecase）に揃える:

- 調査プロンプト 3 ファイル（`.gemini/GEMINI.md` / `.claude/skills/gemini-system/SKILL.md` / `.claude/skills/repomix-snapshot/SKILL.md`）の「Port definitions (traits in domain layer)」を、domain 層ポートと usecase 層ポートの両方を含む表現へ改める（監査 F-05 の解消）。
- capability の Architecture Guard（`implementer` / `dry-fix-lead` / `review-fix-lead`）に domain ポートの行と `apps/cli-driver`（primary adapter）の行を追加し、3 capability の guard を同一内容に揃える（監査 F-06 の解消）。
- R1 マトリクスに配置判別の tie-break 基準（「domain の不変条件・集約の語彙で説明できるなら domain、アプリケーションのオーケストレーションが必要とする技術的能力なら usecase」）と境界例の分類を追記する（F-07 の恒久対策）。

### D5: ValueObject 規則を是正し、side-effect-free な導出メソッドを許容する

`type-designer-kind-selection.md` R3 の「値以外の何かを計算して返す method は behavior であり ValueObject 違反」という bright-line を改め、禁止対象を「依存や外部リソースを扱う behavior 中心の service 的 struct（`Codec` / `Validator` / `Resolver` / `parse_*` 等）」に限定する。値等価で識別される型が自身の値から新しい値・述語を導出する side-effect-free なメソッド（例: `Money::add`、`DateRange::overlaps`）は ValueObject に許容する。R6（DomainService 採用条件）には「値等価で識別され導出メソッドのみを持つ型は DomainService ではなく ValueObject」という除外条項を前置し、構造条件（field 数・method 数）より意味論判定（値そのものか、値に働く操作の集合か）を先に適用する（監査 F-08 の解消。catch-all 濫用防止という R3 の本来意図は維持する）。

### D6: DRY の cross-layer 断定を是正し、意図的ミラー型を統合対象から明示的に除外する

`dry-fix-lead` capability の「DRY violations cross layer boundaries by definition, so cross-layer edits are expected and permitted」を「一部の DRY 違反は層をまたぐため whole-workspace scope を持つ」に改める。あわせて「関心分離に由来する意図的な構造類似（core 型とその adapter ミラー DTO/enum。type-designer の self-check が規定するミラー設計）は DRY 違反ではなく統合対象外」という除外規定と、「正当な cross-layer 共通化は双方が依存できる、より内側の層へ抽出する（上位層への引き上げは依存方向を逆転させるため禁止）」という抽出方向ルールを追記する（監査 F-11 の解消。`dry-check-workflow.md` への DRY 判断基準の明文化 — 知識の重複とテキスト類似の区別、偶発的類似は違反ではないこと — も同時に行う）。

### D7: 入口ドキュメントの層表記を依存方向が一意に読める形へ統一する

CLAUDE.md の Workspace shape 行の凡例なし矢印（`libs/domain` → `libs/usecase` → `libs/infrastructure`）を、依存方向が誤読できない表記へ改める（依存矢印を内向きに統一するか、「内側→外側の層順であり依存は逆向き」という凡例を付す）。あわせて delivery 3 crate の一言依存（cli-driver は usecase のみ / cli-composition が全層を配線 / cli は bin）を添える（監査 F-12 の解消）。README.md の SoT Chain 節で `→` が「派生順序」と「参照方向」の 2 通りに使われている不統一（監査の Low finding）も同じ表記統一作業の中で扱ってよいが、本決定の必須範囲は CLAUDE.md とする。

### D8: overlay の ADR テンプレートを現行の front-matter 形式へ差し替える

`overlay/knowledge/adr/README.md` のテンプレートと運用ルール記述を現行規約に一致させる: file-level の `## Status` 見出しと「**Status**:」運用ルールを削除し、YAML front-matter（`adr_id` + `decisions[]`、decision 粒度の `status`（5 値小文字）と根拠 ref）を必須として例示する（`knowledge/conventions/adr.md` の例に準拠）。これにより、テンプレートに従った consumer の初 ADR が `bin/sotp signal check-adr-user` で fail する事故（監査 F-14）を解消する。maintainer 側 `knowledge/adr/README.md` のテンプレート節にも同じ陳腐化があるため、同時に是正する。

### D9: CI の track-aware gate は release PR のみ明示的に免除し、その他の PR では fail-closed を維持する

`.github/workflows/ci.yml` の track-aware gate step（`cargo make ci-track-container` の実行）の条件を、「pull_request であること」から「pull_request のうち `develop` → `main` の release PR を除くすべて」へ改める（head が `develop` かつ base が `main` の組合せのみを免除する否定条件）。

release PR は track/<id> branch 文脈を持たず、track-aware gate の検証対象（単一 track の spec/catalogue/impl-plan 連鎖・task-contract）がそもそも存在しない。その内容は各 track PR が develop へ merge される時点で同 gate を通過済みであるため、境界での skip は回避策ではなくカテゴリ整合である。一方、track/* 以外の head を持つその他の PR（feature branch から develop への直行 PR 等）では gate を走らせたまま fail-closed で落とす状態を維持し、「develop への変更は track PR 経由」という運用を CI が機構として強制し続ける。あわせて、track branch 再作成 step と gate step に分散している実行条件の対応関係を step コメントに明記し、片側だけが更新される条件ドリフト（本件の直接原因。D3 が扱う更新チェックリスト同期と同型）の再発を防ぐ。

## Rejected Alternatives

### A. hexagonal-architecture.md の全面改訂（6 crate 化して維持）

監査レポートの当初推奨。人間可読の一枚絵が残る利点はあるが、機械可読 SSoT の再記述という構造が残る限り、次の層構成変更で同型のドリフトが再発しうる。`enforce-by-mechanism.md` の優先順位（docs は最弱 tier）と `no-upstream-restatement.md` の趣旨に照らし、更新規律で守る改訂よりも重複自体の除去を選ぶ。

### B. 縮退（restatement 部分のみ削除し、意味論だけ残した薄い文書として維持）

残す価値のある意味論には既により適切な行き先がある（purity 規則 → coding-principles.md、配置判別 → R1 マトリクス）。中途半端に薄い文書を残すと cite の分散と索引ノイズが続き、「アーキテクチャの正典はどれか」という問いに二重の答えが残るため却下。

### C. purity 規則を独立の薄い convention に切り出す

verify ツールと 1:1 対応する文書になる整理は成立するが、規約ファイル数が増え、coding-principles.md（error handling / no-panics 等の既存のコード規範置き場）との棲み分けが新たな判断点になる。既存文書への統合で足りるため却下。

### D. ValueObject 規則の現状維持（Money 型は DomainService または FreeFunction 分割で扱う）

DDD の標準的な Value Object 定義（値等価・不変・side-effect-free な導出メソッドを持ちうる）から乖離した分類が制度化され、値に帰属すべき振る舞いを型の外へ追い出す anemic value model をテンプレートが構造的に推奨してしまうため却下。

### E. DRY 断定の現状維持（`sotp dry` の judge 判定を最終防波堤として信頼する）

fixer の prior に埋め込まれた誤った一般化は、判定の最終段だけで防ぐには事故経路が太い。意図的ミラー型の統合は依存方向の破壊という高コスト事故につながるため、prior 側の文言是正と明示的除外を選ぶ。

### F. overlay ADR テンプレートの現状維持（front-matter は各自が adr.md を読んで補う運用）

`bin/sotp signal check-adr-user` が front-matter 欠落を即 fail とする以上、テンプレート自体が fail する成果物を生成させるのは fail-closed 設計の意図に反する。テンプレートは従えば通る形であるべきなので却下。

### G. track-aware gate の skip 条件を「head が track/* か」だけで判定する

branch 再作成 step と条件が対称になり最も単純だが、任意の非 track PR（feature branch → develop 等）が track gate を素通りする側門を作る。SoT chain の merge gate を CI 上で回避する経路が生まれるため却下。免除は release PR という列挙可能な 1 形状に限定する（D9）。

### H. ツール側で非 track branch を skip 扱いにする / release PR に track id を明示指定する

前者は NotTrackBranch の fail-close というツール本体の設計を弱め、track branch 上の設定不備まで silent skip になりうるため却下。後者は複数 track の成果が混在する release PR に「単一の active track」という概念が適合せず不成立。

## Consequences

### Positive

- 監査 High 2 件（F-01 / F-08）を含む High/Medium 全 11 件の解消方針が確定する。
- 「機械可読 SSoT の再記述」という文書ドリフトの発生源が除去され、F-01/F-02/F-07 型の矛盾クラスが構造的に再発しなくなる。
- ポート配置・層依存について「どの文書が正か」が一意になる（依存 = architecture-rules.json、配置 = R1 マトリクス）。
- テンプレート consumer が day-one で踏む事故（ADR テンプレート起因の CI fail、規約に従った deny 違反）が消える。
- release PR（develop → main）の CI が構造的に fail する状態が解消され、かつ非 track PR に対する fail-closed の門番は維持される（D9）。

### Negative

- アーキテクチャ全体を散文で概観する単一文書が消える。onboarding は README / AGENTS.md / CLAUDE.md の要約と `bin/sotp arch tree`、R1 マトリクスに委ねられ、初学者にはやや不親切になる可能性がある。
- cite 付け替えの対象が多い（レビュープロンプト・スキル・rules・入口ドキュメント）。付け替え漏れは一時的に dead reference になる（実装 track のレビューで検出する）。
- `knowledge/conventions/README.md` の索引再生成（`bin/sotp conventions update-index`）と、conventions を参照する既存文書の微修正が必要になる。

### Neutral

- 本 ADR の実装フットプリントはドキュメント（`.md`）と CI 設定（`ci.yml`）のみで、**Rust の挙動・実装・lint 設定には変更を加えない**。Rust ファイルへの変更は D3 の doc comment 参照付け替え（コメントのみ、数箇所）に限られる。特に D5 は出荷 catalogue lint 設定と衝突しない — 同設定が ValueObject に課す機械制限は `ForbiddenMethodReceiver (&mut self)`（可変メソッド禁止）のみであり、D5 が許容する side-effect-free な `&self` 導出メソッドは lint 上もともと合法（禁止していたのは規約文書だけ）。この性質は実装 track の review scope 選定（コード層レビューの要否判断）に利用できる。
- 監査の Low findings 5 件（レビュープロンプトの LSP 例の不正確・CQS を CQRS とするラベル、README の矢印方向不統一、codex-system テンプレートの Tokio 固定文脈、`architecture-rules.json` の usecase `deny_reason` 文言の陳腐化）と改善提案 44 件は本 ADR の決定対象外であり、実装 track のタスクとして個別に扱う。
- `sotp verify usecase-purity` の機械強制の実装・強度は本 ADR では変更しない（文書の移設と強制強度の明記のみ）。

## Reassess When

- テンプレート採用プロジェクトから「アーキテクチャの人間可読な概説が無く onboarding が困難」というフィードバックが継続的に出たとき（対応候補: `architecture-rules.json` から概説ビューを自動生成する read-only view の導入。手書き文書の復活は再記述ドリフトを再導入するため最終手段）。
- 層構成の変更により、R1 マトリクスと `architecture-rules.json` では表現しきれないアーキテクチャ上の意味論（例: 層内のモジュール規律、複数 delivery の並立）が増えたとき。
- `sotp verify usecase-purity` を error へ昇格させるとき（coding-principles.md へ移設した purity 節の強制強度記述を同時に更新する）。
- 監査 Low findings / 改善提案の実装中に、本 ADR の決定と矛盾する事実が見つかったとき。
- release PR 以外に track 文脈を持たない正当な PR 形状（hotfix 直行 PR の公認等）が必要になったとき（D9 の免除列挙を見直す。列挙の際も fail-closed 既定は維持する）。

## Related

- `knowledge/conventions/no-upstream-restatement.md` — 再記述禁止原則（D1 の根拠）
- `knowledge/conventions/enforce-by-mechanism.md` — 強制手段の優先順位（D1 の根拠）
- `knowledge/conventions/type-designer-kind-selection.md` — R1 マトリクス（D1 / D4 / D5 の対象）
- `knowledge/conventions/coding-principles.md` — purity 規則の移設先（D2）
- `knowledge/conventions/adr.md` — ADR front-matter 規約（D8 の準拠先）
- `architecture-rules.json` / `deny.toml` — 層依存の機械可読 SSoT（D1）
