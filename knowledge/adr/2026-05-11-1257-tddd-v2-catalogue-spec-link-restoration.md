---
adr_id: 2026-05-11-1257-tddd-v2-catalogue-spec-link-restoration
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-v2-catalogue-spec-link-restoration:2026-05-11"
    candidate_selection: "from:[alpha-restore-inline-fields,beta-spec-backward-link,gamma-sidecar-spec-states] chose:alpha-restore-inline-fields"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:tddd-v2-catalogue-spec-link-restoration:2026-05-11"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:tddd-v2-catalogue-spec-link-restoration:2026-05-11"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:tddd-v2-catalogue-spec-link-restoration:2026-05-11"
    status: proposed
---
# TDDD v3 カタログ: spec_refs / informal_grounds フィールドの復活 (SoT Chain ② の修復)

## Context

### §1 v3 カタログスキーマへの移行と SoT Chain ② の途絶

ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md`（v3 スキーマ ADR）は、
カタログスキーマを types / traits / functions の 3 種のエントリ群に再設計した。
この v3 スキーマは、Language / Role / Pattern 軸を schema 構造で encode するという目的のもとで設計され、
各エントリ種から `spec_refs[]` および `informal_grounds[]` フィールドが削除された。

これらのフィールドは v2 カタログの各エントリに存在し、
ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` の D1.1 で定義された
信号評価規則の入力となっていた:

| 条件 | catalogue signal |
|---|---|
| `informal_grounds[]` 非空 | 🟡 Yellow |
| `informal_grounds[]` 空 + `spec_refs[]` 非空 | 🔵 Blue |
| 両方空 | 🔴 Red |

v3 スキーマへの移行後、catalogue-spec signal の refresher は
v3 エントリに対してこのフィールドにアクセスできなくなった。
その結果、v3 エントリに対して一律 Blue を返す fallback 実装が残ったまま統合された。
これは SoT Chain ② を事実上無効化する fail-open パターンであり、
全エントリが grounding なしで Blue になるため merge gate が素通りになる。

### §2 v3 スキーマが spec_refs / informal_grounds を省いた理由

v3 スキーマ ADR（`2026-05-08-0248`）の主な設計目標は「軸混在の解消」と「schema 構造による制約エンコード」
であり、SoT Chain ② の設計はスコープ外に置かれた。v3 ADR 本文に spec_refs / informal_grounds の
廃止を明示する decision はなく、フィールドが単純に引き継がれなかった形になっている。
v3 スキーマはまだ main に未マージ（working branch 上のみ）であるため、この設計上の空白を
新 ADR で明示的に閉じることができる。

### §3 設計選択肢の比較

v3 カタログで catalogue-spec linkage を復活させる方法として以下の 3 案を検討した:

| 案 | 概要 | 主な懸念点 |
|---|---|---|
| α: v3 エントリへのインライン復活 | 3 種のエントリ種それぞれに `spec_refs[]` と `informal_grounds[]` を追加 | なし（v2 との概念的連続性が最大） |
| β: spec.json 側に backward link を追加 | spec.json の各要素から catalogue エントリを参照させる | SoT Chain の方向逆転（spec ← catalogue は SoT Chain の上流方向への参照）、spec を汚染する |
| γ: サイドカー spec_states.json の新設 | 別ファイルで catalogue-spec の対応関係を管理 | 管理ファイルの増加、catalogue エントリとの同期が複雑化 |

案 α が選ばれた理由:

- v2 カタログの概念設計（各エントリが自身の grounding を持つ）を引き継ぎ、
  SoT Chain の意味論に変更がない
- 既存の `SpecRef` / `InformalGroundRef` 型をそのまま再利用できる
- v2 で定義済みの信号評価規則（D1.1 の informal-priority rule）を変更なく継承できる
- 変更範囲が最小（スキーマ側にフィールドを追加するだけで評価・codec・変換の再利用が成立する）

### §4 関連参照

- `knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.1 — informal-priority rule
  の原典。本 ADR で v3 エントリに継承する
- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` —
  v3 カタログスキーマの定義（types / traits / functions の 3 種のエントリ群）

## Decision

### D1: v3 カタログの全エントリ種に grounding コレクションを持たせる（α 案採用）

v3 カタログの各エントリ種（types / traits / functions の 3 種すべて）に、v2 と同じ意味論の
grounding コレクション（formal な spec ref と informal な ground ref）を持たせる。

catalogue ↔ spec linkage は catalogue 側に保持する。spec.json は汚染しない（β 案却下理由）。
サイドカーには分離しない（γ 案却下理由）。

空の grounding（両コレクション空）はスキーマ上は合法だが、
commit gate および merge gate がそれぞれ検出してブロックする。
gate の動作規則は parent ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.3
の commit / merge マトリクスを継承する。

### D2: 信号評価は parent ADR §D1.1 / §D1.3 を継承する

v3 エントリに対する catalogue-spec signal の評価規則は、parent ADR
`2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.1（informal-priority rule）
および §D1.3（commit / merge マトリクス）をそのまま継承する。

| 条件 | signal |
|---|---|
| informal_grounds 非空 | 🟡 Yellow |
| informal_grounds 空 + spec_refs 非空 | 🔵 Blue |
| 両方空 | 🔴 Red |

Red は commit gate および merge gate でブロック、Yellow は merge gate のみブロック、
commit gate では警告として通過する。parent ADR の規則を v3 にも完全対称に適用する。

### D3: catalogue 表現間の変換で grounding は失われない

v3 カタログを別の catalogue 表現（v2 互換 stub など）に変換する経路が存在する場合、
spec_refs および informal_grounds は変換先に保存される。
silent drop は fail-open の温床であり、変換先での信号評価が変換元と一致する保存則を満たすこと。

### D4: catalogue の functions map のキーは自 crate prefix のみを受け入れる

catalogue の functions map のキー（function path）は、その catalogue 自身の crate prefix で始まるものだけを受け入れる。
他の crate の function path を catalogue で宣言することは禁止する。

codec の decode 段階で他 crate prefix の function path を検出した場合は、silent drop せず decode error として扱う。
silent drop は fail-open の温床であり、D2 で定めた commit / merge gate の信頼性を損なうためである。

本決定は `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` §D11 の amendment として作用する。

却下した選択肢（cross-crate function path を allow する案）:

- 他の crate の function を catalogue で再宣言するユースケースは設計上想定されておらず、運用上も使われていない
- allow すると signal 評価で他の crate の rustdoc を参照する必要が生じ、cross-layer な依存が増える
- 「allow するが signal 評価では除外する」という暗黙の運用は fail-open であり、
  parent ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.3 の commit / merge gate を裏切る

## Rejected Alternatives

### β: spec.json 側に backward link を追加する

spec.json の各要素から参照先 catalogue エントリを列挙する案。

却下理由: SoT Chain の参照方向は「ADR ← spec ← 型カタログ ← 実装」の一方向と定められており、
spec → 型カタログの方向参照は SoT Chain の逆流になる。spec.json を汚染せずに
catalogue-spec の関係を表現するためには catalogue 側でリンクを保持する必要がある（α 案）。

### γ: サイドカー spec_states.json の新設

catalogue エントリと spec 要素の対応関係を別ファイルで管理する案。

却下理由: エントリごとの grounding 情報をエントリ自身から切り離すと、
catalogue JSON を読んだだけでは grounding 状態がわからなくなる。
また spec_states.json と catalogue JSON の同期を保つ運用コストが追加される。
v2 の設計（エントリ自身が grounding フィールドを持つ）の方が
一覧性・保守性ともに優れており、α 案の採用を支持する理由が揃っている。

## Consequences

### 良い影響

- SoT Chain ② が v3 スキーマ上でも正しく動作するようになり、
  grounding なしのカタログエントリが merge gate で検出されるようになる
- fail-open（一律 Blue）パターンが解消され、merge gate の信頼性が回復する
- v2 で定義済みの信号評価規則・型を再利用するため、概念的な変化が最小限に抑えられる

### 悪い影響

- v3 カタログの各エントリ種に grounding を宣言する作業が発生する
  （既存のエントリに grounding が存在しない場合は Yellow / Red 状態から開始し、
  merge 前までに Blue へ昇格させる必要がある）

## Reassess When

- grounding 参照型（SpecRef / InformalGroundRef）の意味論が破壊的に変わった場合:
  v3 エントリへの継承戦略を見直す
- v3 カタログスキーマが再設計される場合（エントリ種の統合・分割など）:
  本 ADR の D1 で想定した 3 エントリ種構造が変わるため再評価が必要
- function path の crate prefix 制約が変わる場合（cross-workspace catalogue 等の概念が導入される場合）:
  D4 の前提（1 catalogue = 1 crate、他 crate 参照禁止）を再評価する
- grounding コレクションを型付き enum（trait grounding と type grounding を区別するような）に
  発展させたい場合: Open Questions として引き継ぐ
- v3 カタログ自身に signals サマリーフィールドを追加したい場合:
  signals ファイルとの責務分担を改めて検討する（Open Questions）

## Open Questions（フォローアップ対象、本 ADR では決定しない）

以下は本 ADR の決定範囲外であり、別途検討が必要:

- `informal_grounds[]` と `spec_refs[]` を Vec のまま維持するか、trait grounding / type grounding を
  区別する型付き enum に発展させるか
- v3 カタログ自身に `signals: ...` サマリーフィールドを追加するか
  （現在は signals ファイルが唯一の評価結果ソース）

## Related

- `knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` — parent ADR (SoT Chain ②)。
  §D1.1 の informal-priority rule を本 ADR が v3 エントリに継承する
- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — v3 スキーマ ADR。
  本 ADR が spec_refs / informal_grounds の grounding フィールドを追加する対象の v3 エントリ種を定義する
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — SpecRef / InformalGroundRef
  の元定義
- `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` — catalogue-spec-signals
  ファイルの分離方針の原典
- `knowledge/adr/README.md` — ADR 索引
