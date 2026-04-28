---
adr_id: 2026-04-25-0353-type-designer-reconnaissance-step
decisions:
  - id: 2026-04-25-0353-type-designer-reconnaissance-step_grandfathered
    status: accepted
    grandfathered: true
---
# type-designer Phase 2 reconnaissance step — 設計開始前に baseline + type-graph で既存型インベントリを把握する

## Context

type-designer は Phase 2 の writer として ADR と spec.json から catalogue entries (kind, expected_methods, action 等) を author する。現状の internal pipeline (`.claude/agents/type-designer.md`) では「Draft → Write → baseline-capture → type-graph + contract-map → type-signals」の順で進み、**catalogue を draft する時点で既存コードベースの型インベントリ (workspace に何があるか、どの kind / partition に属しているか、命名規則がどうか) を見ていない**。

これは以下の問題を生む:

- `action: modify` vs `action: delete + add` の判断が draft 時点で根拠なく行われやすい (cross-partition migration の見落とし)
- 既存型と命名・partition がずれた catalogue が author される
- 結果として review 時に「既存にあった」「kind 違い」など低レベル指摘で round が増える

`bin/sotp track type-graph` (rustdoc ベース) と `baseline-capture` は catalogue 不要で動くため、**設計開始前に走らせて結果を Read する reconnaissance step を pipeline 1 番目に挿入**する余地がある。

関連:

- type-graph view ADR (`knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md`) — type-graph 出力仕様の決定
- TDDD multilayer extension ADR (`knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md`) — 型カタログ多層化
- `knowledge/conventions/pre-track-adr-authoring.md` — 本 ADR の起草根拠

## Decision

### D1: 既存の baseline-capture / type-graph 出力を pipeline 先頭へ前倒しして設計判断に活かす

type-designer の Phase 2 internal pipeline は元から `baseline-capture` と `type-graph` を実行しているが、これらは catalogue draft の **後** に走るため、生成されたインベントリが draft 自体の判断材料にならない。

これを pipeline の **冒頭** に移動し、生成された出力を `Read` してから draft に入る形に並べ替える。

| 順 | 旧 pipeline (現状) | 新 pipeline (本決定) |
|---|---|---|
| 1 | catalogue draft | `baseline-capture` |
| 2 | `<layer>-types.json` Write | `type-graph` |
| 3 | `baseline-capture` | `type-graph` の出力を `Read` ← 唯一の追加ステップ |
| 4 | `type-graph` + `contract-map` | catalogue draft |
| 5 | `type-signals` | `<layer>-types.json` Write |
| 6 | — | `contract-map` (catalogue 入力依存なので draft 後) |
| 7 | — | `type-signals` |

step 3 の Read 対象は `type-graph` の `--cluster-depth` 値に応じて変わる:

- `--cluster-depth 0` の場合: `track/items/<id>/<layer>-graph.md` (単一ファイル)
- `--cluster-depth ≥ 1` の場合: `track/items/<id>/<layer>-graph/index.md` と同ディレクトリ配下の per-cluster ファイル群

新たに追加されるのは step 3 の `Read` のみ。CLI コマンドの種類は変わらず、順序が変わる (`type-graph` を異なるオプションで複数回呼ぶことも選択肢として残る)。

これにより draft 段階で:

- 既存型の inventory (workspace に何が既にあるか)
- 各型の kind / partition (cross-partition migration の判断根拠)
- 命名規則 (新規 entries の命名整合)

を把握した上で kind / action 判定ができる。

レンダリングオプションの選択について:

- `--cluster-depth` などレンダリングオプションの最適値、および異なる値での複数回実行 (例: depth 0 と depth 2 を両方走らせて全体俯瞰と詳細を併用する) を採るかどうかは本 ADR では確定しない。
- 別途調査用トラックを立て、その中で type-designer に実 layer の出力を見せて最適値・呼び出し戦略を評価させる。得られた判断基準を整理し、後続 ADR / convention として固定する。

### D2: type-designer の orchestrator 向け出力を signal evaluation + Open Questions のみに整理する

`.claude/agents/type-designer.md` の Output 仕様 (lines 84-95) は現状 5 種類の section を返すよう定めている (per-layer の Entries written / Action rationale / Signal evaluation、末尾の Cross-partition migrations / Open Questions)。

しかし親 ADR `2026-04-22-0829-plan-command-structural-refinements.md` (§Consequences line 145) で orchestrator 向け責務として明示されているのは「**信号機評価** を完結して返す」ことのみ。Entries written / Action rationale / Cross-partition migrations は親 ADR の範囲を超えて実装段階で追加されたもので、orchestrator のメインコンテキストを不要に圧迫している。

これらの削除対象 section が含む情報は **orchestrator の意思決定 (back-and-forth 判断、次 phase への遷移判断、ゲート判定) に必須ではない詳細**であり、必要が生じた時点で orchestrator が catalogue ファイル (`<layer>-types.json`) / baseline / `<layer>-graph/` を直接 `Read` すれば把握できる。subagent の final message に詰め込む必要はない。

本 ADR で出力仕様を以下に整理する。

**残す**:

- per-layer の `## {layer} — Signal evaluation` — blue / yellow / red カウント + notable な yellow / red への一言
- 末尾の `## Open Questions` — ADR/spec で曖昧だった点。orchestrator が back-and-forth (ADR 修正 / spec 再検討) を判断する根拠となるため

**削る**:

- `## {layer} — Entries written` (catalogue ファイル自体が SSoT として残る)
- `## {layer} — Action rationale` (catalogue entry の `spec_refs[]` / baseline cite から辿れる)
- `## Cross-partition migrations` (catalogue 内の `delete` + `add` ペアから機械的に検出可能)

D1 で導入する reconnaissance step (`baseline-capture` / `type-graph` / `Read`) で得た既存型インベントリも、上記方針の自然な帰結として orchestrator には返さない。インベントリは `<layer>-graph/`、baseline、catalogue 自体に残っているので、orchestrator が必要なら直接読みに行ける。

## Rejected Alternatives

### A. pipeline 順序を変えず、catalogue draft 後に baseline + type-graph 出力を Read して整合性確認する (post-hoc validation)

reconnaissance step を draft 前に挿入する代わりに、現状 pipeline (draft → Write → baseline → type-graph) のまま catalogue を draft し、その後 baseline / type-graph 出力と突き合わせて整合性 (kind の不一致、partition の取り違え等) を後付けで検証する。

**却下理由**: draft 時点の判断が間違っていれば、後段の検証で rework が発生する (kind 違いだけで catalogue entries を書き直す等)。reconnaissance を先頭に置けば draft 時点で正解が見えるので、rework を回避できる。また orchestrator (Claude main session) のメインコンテキストに後段で baseline / type-graph 出力が流れ込み、D2 で削った "余計な情報" がメインコンテキストを通過することになる。

### B. reconnaissance を `/track:type-design` command 本体で実行し、結果を briefing に埋め込んで type-designer subagent に渡す

reconnaissance を subagent ではなく orchestrator (command 本体) が実行し、生成された `<layer>-graph/` の内容を briefing に埋め込んで subagent に渡す。subagent 側は briefing を読むだけで type-graph を実行不要。

**却下理由**: 親 ADR `2026-04-22-0829-plan-command-structural-refinements.md` §D4 / §Consequences で「command 本体は subagent invocation + 結果受け取りのみ」「内部 pipeline (file write / render / 信号機評価) は subagent が完結」と責務が分離されている。reconnaissance を command 本体に持たせると command が肥大化し、責務分離に反する。さらに reconnaissance 結果を briefing 経由で渡すと、orchestrator のメインコンテキストに `<layer>-graph/` の全文が流れ込むため、D2 で削った "余計な情報" を briefing 経由で再導入することになる。

### C. D2 (output sections 整理) を別 ADR に切り出し、本 ADR は D1 (reconnaissance) のみとする

ADR は 1 つの decision を記述する原則 (Nygard 式) に従い、D2 (Entries written / Action rationale / Cross-partition migrations の削除) は別 ADR で扱う。本 ADR は reconnaissance step の追加に scope を絞る。

**却下理由**: D2 (output sections 整理) は、reconnaissance step を追加する設計議論の中で「reconnaissance 結果を orchestrator に echo しない」という延長で発見された問題。D2 を別 ADR に分けて本 ADR が D1 のみになると、本 ADR 側でも「reconnaissance 結果を orchestrator に echo しない」旨を新規 D として書く必要が出てくる (D1 で reconnaissance step を導入する以上、その結果の出力扱いを規定しないと半端な決定になるため)。それを書いてもなお既存の余計な output sections (Entries written / Action rationale / Cross-partition migrations) は本 ADR の touch 範囲外として残ってしまい、「reconnaissance 結果は echo しないが、既存の余計な sections は残る」という不整合が残る。同じ ADR で pipeline (D1) と output (D2) を一貫整理する方が、type-designer の責務境界が明確になる。

## Consequences

### Positive

- catalogue draft 段階で type-designer が既存型インベントリを把握できるため、kind 判断 / action (`modify` vs `delete + add`) 判定の精度が上がり、review round 数が減る (低レベル指摘「既存にあった」「kind 違い」の発生が減る)
- 親 ADR (`2026-04-22-0829-plan-command-structural-refinements.md` §Consequences line 145) で定めた orchestrator 向け責務 (信号機評価のみ完結) と type-designer の実出力が一致する。実装と ADR の乖離が解消される
- orchestrator のメインコンテキスト消費が減る (Entries written / Action rationale / Cross-partition migrations の sections が消えるため、type-designer 1 回の invocation あたりの戻り値が小さくなる)
- 新規 CLI 追加なし。`bin/sotp track baseline-capture / type-graph / contract-map / type-signals` の既存呼び出しを順序入れ替えで再利用するだけ

### Negative

- `.claude/agents/type-designer.md` の Output 仕様を変更するため、現状 5 sections を前提とした下流処理 (もしあれば) の挙動が変わる。ただし orchestrator (Claude main session) は LLM なので、固定 section 名に依存する parser はない (人間と同じく自然言語として読む)
- type-designer の reconnaissance step が増えることで、subagent 1 回の実行時間が伸びる (ただし `bin/sotp` 呼び出し自体は元から行うので、追加分は `<layer>-graph/` の `Read` のみ)
- type-graph のレンダリングオプション選択が「調査用トラックで決定」と先送りされるため、本 ADR 採用後しばらくは type-designer がオプション判断を都度行う必要がある (調査トラック完了まで暫定運用)

### Neutral

- 既存 catalogue ファイル (`<layer>-types.json`) や baseline ファイルの形式は変更なし
- `/track:type-design` command 本体の挙動 (subagent invocation + signal 受け取り) は変更なし

## Reassess When

- 親 ADR `2026-04-22-0829-plan-command-structural-refinements.md` が superseded / deprecated になった場合 — orchestrator 向け責務の定義が変わるため D2 の根拠を再点検
- `bin/sotp track baseline-capture` または `type-graph` のコマンド仕様 / 出力形式が変わった場合 — D1 の pipeline 順序や Read 対象を更新
- 別の subagent (impl-planner / spec-designer) で「ADR 範囲を超えた余計な output sections」問題が表面化した場合 — 本 ADR の整理アプローチを他 subagent にも適用するか別 ADR で検討
- type-graph レンダリングオプション (`--cluster-depth` 等) の判断基準が調査用トラックで確定した場合 — 本 ADR の「調査用トラックで決定」の暫定運用記述を更新し、判断基準を後続 ADR / convention として固定
- type-designer が subagent ではなく別の実装形態 (例: deterministic CLI) に置き換えられた場合 — pipeline と output 仕様の前提が変わるので本 ADR を全面再評価

## Related

- `knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md` — 親 ADR。orchestrator 向け責務 (信号機評価のみ完結) と subagent 内部 pipeline 設計の SSoT
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — 上位 ADR。Phase 別 command 分解と subagent 責務分離の roadmap
- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — type-graph 出力仕様の決定 (本 ADR の reconnaissance step が活用する出力)
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — TDDD 多層化と `<layer>-types.json` カタログ構造
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 作成 lifecycle / 配置ルール (本 ADR の起草根拠)
- `knowledge/conventions/workflow-ceremony-minimization.md` — post-hoc レビュー方式 / 余計な状態の撤廃 (D2 の精神に通じる)
- `.claude/agents/type-designer.md` — 本 ADR が変更対象とする type-designer subagent 定義
