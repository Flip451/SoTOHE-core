---
adr_id: 2026-07-08-1020-retire-todo-marker-state-and-track-docs
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-0182uGsSwBmcuwAHkF2GHn8R:2026-07-08"
    candidate_selection: "from:[retire,A,B,C] chose:retire"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-0182uGsSwBmcuwAHkF2GHn8R:2026-07-08"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-0182uGsSwBmcuwAHkF2GHn8R:2026-07-08"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-0182uGsSwBmcuwAHkF2GHn8R:2026-07-08"
    status: proposed
---
# TODO マーカー状態管理の廃止

## Context

track 直下には SoT Chain 導入以前からの文書群（`tech-stack.md` / `product.md` / `product-guidelines.md`）が残っており、`tech-stack.md` には「未解決の TODO: マーカーが残っている間は実装をブロックする」という専用ゲート（verify tech-stack）が付属する。

これらは 3 点で現行設計と衝突する。

1. 技術選定・製品方針は ADR の担当領域であり、専用文書は ADR との二重管理（上流再記述）になる。
2. 文書内の TODO マーカーはファイル内容に埋め込まれた状態フィールドであり、「状態はファイル存在と信号から導出する」方針と矛盾する。TODO を消す行為が準備完了の宣言を代行する儀式になっており、マーカーの不在と内容の正しさは無関係。
3. マーカー依存の検査は宣言ベースであり、存在ベースで検査対象を導出する現行思想に反する。

なおテンプレートは out-of-the-box で CI が通るため、placeholder を埋めさせるオンボーディング装置としての役割も失われている。

## Decision

### D1: verify tech-stack ゲートを実装ごと廃止する

`verify tech-stack` ゲート（文書内 TODO マーカーの検査）を実装ごと廃止する。Makefile タスク、CLI の verify サブコマンド、検査実装を撤去し、以後も文書内の TODO マーカーを状態として消費する機構は作らない。

### D2: track 直下の文書群を廃止し ADR / README へ統合する

`track/tech-stack.md` / `track/product.md` / `track/product-guidelines.md` を廃止する。生きている決定内容は ADR へ昇格し（遡及コストが高い場合は grandfathered の集約 ADR 1 枚に載せる）、読者向けの製品ビジョン記述は README へ吸収する。以後、技術選定・製品方針は pre-track ADR で行う（テンプレート利用者も同様）。

### D3: 派生ビューは対象外として残す

派生ビュー（`track/registry.md`、`track/items/<id>/` 配下の `spec.md` / `plan.md` / `contract-map.md` など）は本件の対象外として残す。これらは遺物ではなく、SSoT から再生成される読み取り専用ビューである。

### D4: 廃止対象を生きた参照元に残さない

廃止後の生きた SoT・自動化・読者向け文書は、`verify tech-stack` ゲートや廃止する `track/*.md` 文書群を現行の前提として扱わない。どの参照元を更新対象にするかは spec / impl-plan 側で導出し、この ADR では参照整合性の方針だけを決定する。

## Rejected Alternatives

### A. TODO ゲートを維持・強化する

マーカー検査を精緻化しても「マーカーの不在 = 内容の正しさ」という飛躍は埋まらない。状態フィールドの形骸化は検査強度では解けないため却下。

### B. 文書は残しゲートだけ廃止する

ADR との二重管理が残り、更新されない文書が乖離の温床になり続ける。ゲートを失った placeholder はさらに形骸化するため却下。

### C. tech-stack.md を ADR への索引（ポインタ集）として残す

索引は ADR の README が既に担っており、二枚目の索引は分裂の温床。却下。

## Consequences

### Positive

- 文書内マーカーという状態表現が消え、「状態はファイル存在と信号から導出する」方針が例外なく貫かれる
- 技術選定・製品方針の SSoT が ADR に一本化され、二重管理が構造的に消える
- verify ゲート 1 本と overlay の保守対象（汎用化した track/*.md）が減り、スモークゲートも軽くなる

### Negative

- 技術スタックを一枚で一覧できる場所が無くなる（ADR 索引経由の参照になる）
- 生きている決定の ADR への昇格・仕分け作業が一度発生する

### Neutral

- テンプレート利用者のオンボーディングが「placeholder を埋める」から「必要時に pre-track ADR を書く」へ変わる

## Reassess When

- 技術スタックの一覧性への実需が出たとき（その場合も専用文書ではなく、ADR からの派生ビュー生成を検討する）
- テンプレート利用者のオンボーディングで「最初に埋める場所」の不在による混乱が観測されたとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工状態フィールドの排除
- `knowledge/conventions/no-upstream-restatement.md` — 上流の再記述禁止
- `knowledge/conventions/adr.md` — 技術選定は ADR の担当領域
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR の運用
