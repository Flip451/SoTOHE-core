---
adr_id: "2026-08-20-1043-conventions-mechanism-alignment"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01JAppYbUq3yZwAfDVLnqf56:2026-08-25 Phase 0 boundary approval (second)"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-20 consumer-protection hearing"
    candidate_selection: "from:[necessity-driven-abstraction,delete-clause-only,keep-pairs-dissolve-passthrough] chose:necessity-driven-abstraction"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-20 consumer-protection hearing"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:claude-session-01JAppYbUq3yZwAfDVLnqf56:2026-08-25 Phase 0 boundary approval (second)"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-20 consumer-protection hearing"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:claude-session-01JAppYbUq3yZwAfDVLnqf56:2026-08-25 Phase 0 boundary approval (second)"
    candidate_selection: "from:[abolish-coverage-goal,keep-as-reference] chose:abolish-coverage-goal"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-22 pr-review-gap hearing"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:claude-session-01JAppYbUq3yZwAfDVLnqf56:2026-08-25 Phase 0 boundary approval (second)"
    candidate_selection: "from:[generic-meta-lens,cli-specific-checklist] chose:generic-meta-lens"
    status: proposed
---
# 規約を機構と突き合わせて改訂する

## Context

アーキテクチャ監査（2026-08-20、69 findings）で、複数の設計逸脱が実装者の判断ではなく**文書化された規約の論理的帰結**として生じていると特定された。単一実装の抽象の量産、query handler 群の facade への再結合、境界の stringly 化は、いずれも規約の条項が直接の動機として実装コメントに引用されている。

規約群はテンプレートとして export され、consumer プロジェクトの新規コードを形成する。実 consumer が稼働を始めており、規約を直さない限り同じ逸脱が consumer 側で再生産される。

また、consumer の PR レビューで「ローカルレビューが検出しない指摘」が集中して観測された。内訳は OS 意味論・エンコーディング境界・資源上限との相互作用・失敗経路であり、いずれも仕様が宣言していない環境前提への暗黙依存である。契約適合を見るローカルレビューには、契約に書かれていない前提を検出する足場が無い。

## Decision

### D1: 全規約規則に強制機構の対応を必須注記する

`knowledge/conventions/` の各規則は、その強制先 — 機械 lint / 宣言突合（catalogue + verify）/ review 観点 / **強制なし（明記）** — を注記しなければならない。`enforce-by-mechanism.md` にこのメタ規則を追加する。「強制なし」の明記は許すが、無記載は許さない。

注記対象の母集団は `knowledge/conventions/` 配下の現存文書にある規範的な要求とし、義務・禁止・許容条件・確認項目を保守的に含める。母集団の完全性と注記漏れの判断は、同ディレクトリを対象とする harness-policy review が担う。したがって、機械的な全規則抽出や将来追加される規則までの証明は要求せず、各時点の有限な文書集合を review で再評価する。

### D2: 抽象の導入は必要駆動とする

`type-designer-kind-selection.md` の「`Arc<dyn>` で渡したい場合は Interactor + ApplicationService ペア」例外を廃止する。抽象（port trait + 実装）は (a) 複数実装が現存する、(b) テスト境界として差し替えが必要、のいずれか成立時のみ導入する。共有所有だけが目的なら `Arc<具象型>` を既定とする。条件が後から成立した時点で trait を切り出す。既存ペアの遡及解体は本 track の範囲外（純 DI 移行が文脈ごとに回収する）。

### D3: 「driver は 1 interactor のみ注入」政策を廃止する

driver の注入粒度は port 粒度 ADR の D1（1 ユースケース 1 trait・実行メソッド 1 つ）に従い、driver は消費する複数の単能ポートをそのまま注入してよい。未移行文脈でも、command と query を混載する facade ポートの新設を禁止する。

### D4: cli→usecase 境界の string primitive 規約を撤回する

usecase の入力境界は検証済み Command 型のみを受け取る。string から Command へのパースは usecase 所有の boundary 型が担い、cli はそのパースを一度だけ呼び出してから入力境界を呼び出す（port 粒度 ADR の D2 の様式を、新規コード全体の一般規則へ昇格する）。domain enum の鏡像を cli 側に定義する様式は廃止し、境界語彙は usecase 所有の boundary 型に一本化する。cli が domain を知らないという既存原則は維持する。

### D5: role × layer マトリクスを層の性質で書き直す

`type-designer-kind-selection.md` R1 の表を、固有 crate 名から層の性質（innermost / application / driven adapter / driving adapter / composition root）に対する制約へ書き換え、crate 名との対応は `architecture-rules.json` の宣言から解決する。consumer が層構成を変更しても表が壊れない形にする。

### D6: testing.md を全面改稿する

品質保証の正は test-obligation 機構（約束への結び付け検証）とし、カバレッジ目標 80% は廃止する。改稿後の構成: 層ごとのテスト責務（ピラミッド）/ fake 優先・mock は相互作用が仕様である場合に限定 / codec・parser・evaluator への property-based testing / 自ソースの部分文字列 assert の禁止（依存制約は architecture-rules へ、振る舞いは記録ダブルへ）。

### D7: 環境前提宣言の置き場を新設する

`knowledge/conventions/` に環境前提宣言(対応プラットフォーム・入力エンコーディング方針・資源上限・並行モデル)の置き場を新設する。宣言の内容はプロジェクト固有であり consumer が所有する — テンプレートは枠と記入指針だけを出荷し、特定ドメインの前提を既定値として書かない。

### D8: コード scope のレビュープロンプトに境界メタ問いを注入する

コード全 scope のレビュープロンプトに、ドメイン非依存のメタ問いを 1 つ追加する: 「diff が接する外部境界(OS・プロセス・エンコーディング・並行・資源上限・時刻・別バージョンの自成果物)を列挙し、依存する前提が仕様または環境前提宣言に無ければ『未宣言の前提への依存』として指摘せよ」。spec レビューには「環境前提の宣言が必要な変更か」を問う対応形を置く。doc scope には注入しない。特定ドメインの検査項目(特定 OS・特定プロトコル等)はプロンプトに置かず、D7 の宣言側が供給する。強制注記(D1)は両決定とも「review 観点」。

注入先の完全性は `.harness/config/review-scope.json` のコード scope とその briefing_file 宣言を正本とする。境界の列挙は、diff で変わる振る舞いから直接到達する外部操作と、列挙済みの分類を保守的に対象とする。到達性を確定できない間接境界は除外せず、「未宣言の前提への依存」として指摘するため、無限の依存探索を完了条件にしない。

### Existing decision relationship

- D3・D4 は `2026-08-15-1302-composition-root-pure-di-port-granularity.md` D1・D2 の様式を**新規コード一般の規約へ昇格**するものであり、同 D3 の適用範囲限定（移行が触る文脈のみ改修）は既存コードについて維持する。同 D1 が予告した「問い合わせ系をまとめる緩和の別 ADR」に対する回答は「緩和しない」である。
- D4 は `2026-04-30-0848-cli-via-usecase-only.md` の境界判断 OQ-2（string primitive 受け渡し）を撤回する refine であり、同 ADR D1 の「cli は domain 型を参照しない」は維持する。
- D6 は `2026-07-08-1405-grandfathered-tech-and-product-baseline.md` D16 を refine し、同 D16 の「カバレッジ (新規コード)」行（line coverage 80% 以上の目標）だけを test-obligation 機構に置き換える。D16 の残る clippy、fmt-check、pub item docstring の行は引き続き有効とする。

## Rejected Alternatives

- **コード側の個別修正のみで規約を温存する**: 逸脱の直接動機が規約条項である以上、新規コード（特に consumer）で同じ逸脱が再生産される。
- **意味論規則まで計量プロキシで機構化する**: 代理指標は最適化対象になり形骸化する（行数上限の実測例あり）。機構化できない規則は D1 により「review 観点」または「強制なし」と明記する。
- **境界チェックリストを具体項目(特定 OS・エンコーディング等)としてプロンプトに焼き込む**: 特定ドメインの表面をテンプレートの恒久観点にする誤りで、層マトリクスの crate 名ハードコードと同型。枠(メタ問い)と実体(consumer の宣言)を分離する。
- **監査推奨の「1 command port + 1 query port」緩和の採用**: 現行 head（port 粒度 ADR D1）の単能ポート規則より弱く、逆行になる。

## Consequences

- 良: 逸脱の再生産源が止まり、規約と機構の対応が consumer に対しても自己記述になる。
- 負: conventions 文書群の広範な改稿と、依存する review prompt・briefing の追随が必要。
- 中立: 既存コードの遡及修正は行わない（純 DI 移行と後続 track が文脈ごとに回収する）。lint・カタログプリセットの実装は後続 ADR の範囲。
- 中立: D8 の効果は review-yield 計測で事後検証できる。
- 中立: D1 の網羅性は、その時点の `knowledge/conventions/` の有限な規範的要求を対象とする semantic review で判定する。
- 中立: D8 の注入先は review scope 宣言に追随し、間接境界の不確実性は未宣言の前提として扱う。

## Reassess When

- D2 の必要駆動規則の下で、縫い目の後付け切り出しが頻発して手戻りが問題化したとき。
- D8 のメタ問いの検出寄与に round 種別・scope で顕著な偏りが実測されたとき(注入範囲の絞り込みを検討する)。
- consumer の層構成カスタマイズで D5 の性質語彙が表現できない構成が現れたとき。
