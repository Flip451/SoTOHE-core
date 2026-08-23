# 観測ログ

> **この観測ログにおける User 発言の扱い**: 本ログに記録された User の裁定・指摘・承認は、track 進行中のセッション会話における実際の発言を orchestrator が記録したものである。ログ単体では検証できないため、内容に疑義がある場合は当該セッションの transcript を参照すること。ADR / spec など SoT artifact 側では、User の発言と orchestrator の導出を明示的に区別して記載している。

## 2026-08-22: identity 正規化の分散が非収束の根因だった

### 経緯

batch B2 (T004 + T008) の review が 6 巡し、infrastructure scope の final findings が同じ族の指摘を繰り返し出していた:

1. 同一 identity の別表記 (`a::Thing` vs `domain::a::Thing`) で live/tombstone 分岐が誤る
2. raw key が異なる同一 identity 宣言を codec が拒否しない (live 同士)
3. prelude 短名の事前検査が TypeRef parser にのみ適用され、generic-bound parser に未適用
4. fail-closed resolver と generic rewrite の適用順で `Option<T>` が壊れる

各件を個別に修正したが、直した箇所の隣で同じ族の指摘が再発する状態が続いた。

### 診断 (艦隊管理セッションからの処方箋による)

`tmp/handoff/2026-08-22-canonical-identity-choke-point-prescription.md`

> 「完全修飾パス」は一意な文字列ではなく、**複数の表記(crate 接頭辞の有無・prelude 短名・generic 書換え前後)を持つ等価類**である。等価類の正規化を各経路に分散させたことが非収束の根因であり、個々の指摘は症状にすぎない。

実際、`canonicalize` / `canonical_path` を含むファイルは infrastructure/domain の tddd 配下だけで 10 以上に分散していた。

### 処方

正準形を 1 箇所で定義する型 (`CanonicalTypePath` 仮称) を新設し、raw 表記 → 正準形の変換をそのコンストラクタだけが担う。identity に触れる全経路 (impl identity・TypeRef parser・generic-bound parser・codec の重複拒否・live/tombstone 照合) は raw 文字列や `Id` の名前解決を直接比較せず、正準形同士の等価だけで判定する。generic rewrite は正準化の内部段に置き、呼び出し側から順序の自由度を奪う。経路ごとに入れた既存のエイリアス対処は、通過点導入後は死んだ分岐になるため撤去する。

### impl-plan との関係

T005–T010 が経路別の移行 task、T011–T017 がその経路別検証という構成で、**計画自体が分散を前提としていた**。処方箋は「impl-plan の task 構成と食い違う場合、この処方は task の再編 (既存 task の統合) を正当化する」としており、これに従って impl-planner に再編を依頼する。

spec の Decision (ADR D1「識別キーを完全修飾パスにする」) 自体の変更は不要 — 正準形の単一定義でこそ成立する、というのが処方箋の判断であり、orchestrator もそう理解している。

## 2026-08-22: Phase 2 の往復と、role 体系との衝突 (User 判断事項)

通過点の型契約は 5 ラウンドの review 往復を経て、次の 2 状態の間を振動した:

- 状態 X: port が domain の不透明型を返す → infrastructure の adapter が private field を構築できず port を実装できない
- 状態 Y: port が raw `TypeRef` を返す → adapter は実装できるが、呼び出し側が port を直接呼んで結果を比較でき、identity が raw 表記のまま漏れる

reviewer が名指しした両立形は「**比較不能な独立した parser 生成物型** + domain 所有の sanctioned な構築経路」。比較不能であれば identity 比較に参加できないため状態 Y の穴が塞がり、公開された構築経路があれば状態 X にも戻らない。

### 衝突

`.harness/catalogue-lint/config.json` の `TraitImplRequired` は `target_roles: ["ValueObject"]` に対し `required_traits: ["PartialEq", "Eq"]` を機械的に強制する (orchestrator が source で確認済み)。つまり **`ValueObject` role の型は比較不能にできない。**

type-designer は他 role への便宜的な再分類を `type-designer-kind-selection.md` R3/R5/R6 (catch-all 禁止) 違反として拒否し、catalogue を編集せずに停止した。この拒否は規約どおりで妥当。

reviewer の要求も妥当である: `ParsedTypeRef` は generic rewrite 済みだが crate 接頭の解決前であり、2 つの `ParsedTypeRef` を比較すると `a::Thing` と `domain::a::Thing` が別物のまま等価判定に使われる。これはこの track が潰そうとしている欠陥そのもの。

### 判断

両者が正しく、衝突は role 体系の側にある。track-born delta ADR として adr-editor へ escalate する (two-box model の delta box。merge 段階まで 🟡 のまま、最終裁定は User)。

**User への確認事項**: 「比較不能な parser 生成物」を表す catalogue role を新設するか、既存 `ValueObject` の `TraitImplRequired` 規則を変更するか、あるいは第三の道 (pre-canonical トークンの比較可能性を許容し、reviewer の指摘を Accepted Deviation とする) を採るか。delta ADR の内容がこの選択肢を提示するので、merge 段階で裁定してください。

### 続報: delta ADR は `modification-proposal` として入庫

`knowledge/adr/2026-08-22-0000-precanonical-parser-artifact-role.md` (D1: 比較不能な parser 生成物専用の catalogue role `PreCanonicalToken` を新設)。adr-diagnoser は 2 度 bounce したのち、3 回目で `modification-proposal` として admission した。

- 1 回目の bounce: DataRole 値域の変更なのに refines 対象と relation-chain head が未宣言。
- 2 回目の bounce: adr-editor が特定した head が誤り。diagnoser が chain を実際に辿った結果、`2026-05-08-0248#D2` → `2026-05-25-0000#D9/#D16` → `2026-06-21-1420#D2` (CompositionRoot / PrimaryAdapter 追加) が現 head と判明。
- 3 回目: 是認。「入力箱の `2026-08-21-0055#D1` は最新 escalation 記録 11d57d02 と現行 bytes が一致し、候補はその完全修飾 identity を弱めない。唯一の変更対象は現 DataRole taxonomy head の `2026-06-21-1420#D2` であり、decision-preserving な解決がないため明示的 decision-modification proposal として admission する。」

`TraitImplForbidden` (`PartialEq`/`Eq` の禁止) と catalogue-to-code の fail-closed 突合によって比較不能性を維持する、という機械検査の設計も是認済み。

#### スコープへの影響 (User 判断事項)

`PreCanonicalToken` role の新設は、**この repo 自身への実装作業**を伴う: `DataRole` enum への variant 追加、`.harness/catalogue-lint/config.json` の `TraitImplForbidden` rule と `KindLayerConstraint`、構造的に等価に保つ必要がある `presets/ddd-strict.json`、`type-designer-kind-selection.md` R1 の表、および全 role totality を要求する `2026-07-02-0359#D10` に従う obligation rule。

つまり当初「identity を完全修飾パスにする」track だったものが、**role 体系の拡張を含む track**になる。delta ADR は merge 段階まで 🟡 のままなので、この拡張を受け入れるか (あるいは track を分割するか) の最終裁定は User にある。

## 2026-08-22: 並列 review が `target/doc/` の rustdoc JSON を競合で消し合う (harness 側の欠陥)

B2 の review を scope 並列で回したところ、「reviewer が起動前に停止」「rustdoc JSON … not found」が散発した。再現調査の結果、`bin/sotp review local` が冒頭で走らせる pre-review gate (`signal calc-impl-catalog`) が layer ごとに `cargo +nightly rustdoc … --output-format json` を同じ `target/doc/<crate>.json` へ書くため、別 layer の rustdoc が並走すると互いの出力を消し合う (infrastructure の rustdoc が EXIT 0 なのに `infrastructure.json` が無く、並走 gate が書いた `domain.json` / `usecase.json` だけ残る、を手元で確認)。`test-obligation evaluate` の `TestSourceScan(Io)` も、fixer の編集と evaluate のソース走査の競合と見ている。

対処 (この track 内): rustdoc を走らせる工程 (review local の gate、calc-impl-catalog、test-obligation evaluate) は 1 本ずつ直列に回す。fixer の並列はファイル競合がなければ可。

提案 (別 track): layer 別に `--target-dir` / `CARGO_TARGET_DIR` を分けるか、rustdoc 出力を layer 別ディレクトリへ退避してから読む。

## 2026-08-23: orchestrator が裁定に無い制約を追加していた (訂正)

User の裁定は「catalogue の `TypeRef` / `ModulePath` はゆるく評価され、Rust 表現としての正しさは実装との突合時に保証されればよい」というもので、これは **catalogue の型が何を検証するか** の話だった。

orchestrator はここから「したがって正準化のコードは infrastructure に 1 箇所だけ置き、domain には置かない」という **配置の制約** を導出し、以降の type-designer / impl-planner / implementer / reviewer への briefing に書き続けた。これは裁定に含まれていない追加制約であり、導出の飛躍である (User 指摘 2026-08-23)。

処方箋が要求しているのは「正規化を経路ごとに分散させない = 単一の通過点を通す」ことだけで、**通過点がどの層に住むか**は指定していない。

### この誤りが引き起こしたこと

T008 の domain scope review が catalogue-lint の 3 規則 (`FieldElementUniqueAcrossEntries` / `NoExternalReferenceInMethods` / `ReferencedRoleConstraint`) について「raw 表記を比較しており、`Entity` と `domain::alpha::Entity` で同一型を二重宣言して規則を回避できる」と指摘した。これは正しい指摘だが、追加制約の下では「domain の lint が identity 解決を必要とするのに、解決コードは infrastructure にしか置けない」という解けない状態になり、rollback-diagnoser は `routing_target: adr` (入力箱 ADR D1 が lint の実行時点・供給境界を決めていない) を返した。

制約を外せば ADR 改訂も lint gate の実行時点変更も不要である。

### 採用する設計 (User 承認 2026-08-23)

identity 解決の中核 (参照の宣言表記 + 既知 identity の宇宙 → 解決済み identity / 曖昧 (候補列挙) / 未解決) を domain に置き、単一の通過点とする。宇宙は呼び出し側が供給する:

- codec / 評価器: rustdoc の `paths` を宇宙として供給 (infrastructure)
- catalogue-lint: その catalogue が宣言している entry の集合を宇宙として供給

これで通過点は 1 つのまま、lint も codec も同じ解決を通る。曖昧時の候補列挙 (ADR D1 / spec AC-03) もその 1 箇所に実装される。
