---
adr_id: "2026-07-23-0109-review-briefing-open-category-format"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
---
# レビュー指示書のカテゴリ閉列挙を半開形式へ改める

## Context

層別 reviewer briefing（`.harness/custom/review-prompts/*.md`）は全て「Report findings ONLY for the following categories」のカテゴリ閉列挙形式を採っている。

この形式の系譜は次のとおり。

- 2026-04-18（f9699693、ADR 2026-04-18-1354）: `plan-artifacts.md` で誕生。対象は計画成果物の事実検査（factual error / contradiction / broken reference / infeasibility / timestamp）であり、欠陥クラスが列挙で閉じる領域だった。文体指摘などのレビューノイズ抑制が目的で、この文脈では閉列挙は妥当に機能した。
- 2026-06-19（7bf7cef7、ADR 2026-06-18-1406 D3–D4）: 7 本の層別 briefing を新設する際、`plan-artifacts.md` の shape（"What to report" / "What NOT to report"）をそのまま踏襲した。文書検査用の閉列挙が、コード設計レビューの雛形として無検討で複製されたのがこの時点である。
- 2026-06-22（d1b2141f）・2026-07-01（76cf45d5）の改訂でも ONLY は温存され、現行に至る。

コードの設計逸脱は開集合であり、閉列挙を持ち込むと「列挙外は報告禁止」となって冒頭の役割文（role statement）の実効性を殺す。

この盲点は実走で実証された。
テンプレート利用プロジェクト（mini-repomix）で composition root に実行メソッド（driver と同一シグネチャの素通し `run`）が生成されたが、`cli_composition.md` は役割文で「must only wire」と宣言しながら、その違反を報告できるカテゴリを持たない。
invoke leak カテゴリの定義が interactor 呼び出しに限定されているため、reviewer は指示書に忠実であるほどこの欠陥を報告できない。
同一ファイル内で役割文と列挙カテゴリが機能的に矛盾している。

## Decision

### D1: コード層 briefing を半開形式に改める

対象は `domain.md` / `usecase.md` / `infrastructure.md` / `cli.md` / `cli_composition.md` / `cli_driver.md` / `harness-policy.md`。

- 「Report findings ONLY for the following categories」の文言を廃止する。
- 冒頭の役割文への違反は常に報告対象であることを明文化する。
- 既存のカテゴリ列挙は「優先カテゴリ」（探索の焦点・severity 判定の基準）へ改称し、網羅の主張を外す。

### D2: What NOT to report は全 briefing で維持する

ノイズ抑制の下限（文体・命名・体裁・閉じたゲート後の代替案提案）は 2026-04 の設計意図として有効であり、半開化後も維持する。

### D3: 文書検査系 briefing は閉列挙を維持する

`plan-artifacts.md` を典型とする事実検査系は欠陥クラスが列挙で閉じるため、閉列挙のままとする。
半開・閉の帰属判定基準は「レビュー対象の欠陥クラスが列挙で閉じるか」であり、SoT 別 briefing（adr / spec / types / impl-plan）は移行タスクでこの基準により個別判定する。

### D4: 役割文とカテゴリの矛盾検査を改訂チェックリストに加える

briefing を改訂する際、「役割文で禁じた事象が、報告可能なカテゴリを持たないままになっていないか」を確認する項目を maintainer checklist に追加する。

## Rejected Alternatives

### A. カテゴリ増補のみで対処する

今回の盲点（composition 上の実行メソッド）をカテゴリに足しても、開集合に閉列挙で追随する構造自体は変わらず、次の未列挙逸脱で同じことが起きる。

### B. ONLY を全廃して自由記述レビューに戻す

2026-04 に閉列挙を導入した動機（文体指摘等のノイズによるレビュー往復の浪費）が再発する。
plan-artifacts 系での実績を壊す理由がない。

### C. 役割文を削除して列挙に一本化する

役割文は各カテゴリの判定文脈を与えており、削除するとカテゴリ判定の精度自体が落ちる。
矛盾の解消方向が逆である。

## Consequences

### Positive

- 「役割文には反するが列挙外」という報告不能クラスが構造的に消える。
- ノイズ抑制（What NOT to report）は維持される。
- 文書検査系の実績ある閉列挙はそのまま残る。

### Negative

- 半開化した層のレビュー指摘数は一時的に増える可能性がある。
- 全 briefing の一括改訂と、reviewer 挙動の再キャリブレーションが必要。

## Reassess When

- 半開化後にレビューノイズが有意に再増したとき（優先カテゴリの表現強化か What NOT to report の増補で調整）
- reviewer が役割文違反の名目で設計趣味の指摘を乱発するとき
- SoT 別 briefing の帰属判定で基準が割れたとき

## Related

- `knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md` — 閉列挙の誕生文脈
- `knowledge/adr/2026-06-18-1406-review-prompts-relocation-per-layer-briefings.md` — 層別 briefing への複製（D3–D4）
- `.harness/custom/review-prompts/` — 対象ファイル群
- `tmp/adr/2026-07-23-0111-composition-root-pure-di-realignment.md` — 盲点を実証した欠陥の本体側 ADR
