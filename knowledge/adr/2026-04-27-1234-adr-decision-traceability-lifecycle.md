# ADR decision の根拠 trace 信号機評価と decision 個別 lifecycle 管理 — MD + YAML front-matter フォーマット採用

## Context

2026-04-25 に実施した `type-designer-tuning-2026-04-25` トラックにおいて、orchestrator が agent briefing 経由で未承認の仕様 (auto-cleanup を default 出力に限定する) を ADR D2 に独断で追加し、その内容が spec / impl-plan まで伝播してユーザー指摘で発覚する事象が起きた。この事象の構造的原因は、orchestrator が briefing を free-form text として記述することで、ユーザー未確認の decision や judgement を密輸する余地がある点にある (WF-67、agent briefing アンチパターン)。

WF-67 は briefing 側の対策 (機械生成 / static template / SSoT pointer 化) を扱うが、それとは別の防衛線として、ADR 自身の decision の正当性を機械的に検査し、decision 個別の lifecycle を追跡する仕組みが求められた。

既存の spec → ADR 信号 (`spec.json` の `adr_refs[]` / `informal_grounds[]`) は spec が ADR を参照しているかを測る forward link であり、ADR decision の根拠が user 承認または review process に遡れるかを測る back-trace とは直交する。両者を組み合わせることで spec / ADR の信頼性網羅と lifecycle 管理が完成する。

なお、`knowledge/conventions/pre-track-adr-authoring.md` は ADR ファイル全体の `## Status` 見出しを禁止しているが、本 ADR の D2 で導入する decision 個別の `status` フィールドはその禁止と **axis が異なる** (ファイル全体の承認状態ではなく decision 粒度の lifecycle)。また、`knowledge/conventions/workflow-ceremony-minimization.md` の「人工的な状態フィールドを作らない」原則は、形骸化して実成果物と乖離する file-level / summary-level の状態フィールドを対象としており、機械検証可能な decision 粒度の lifecycle field とは目的・粒度が異なる。

関連:

- WF-67 (agent briefing アンチパターン) — orchestrator が free-form briefing で未承認 decision を密輸する構造的リスクと、その briefing 側の構造的対策 (機械生成 / static template / SSoT pointer 化)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR ファイル全体の `## Status` 禁止
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工状態フィールド廃止原則

## Decision

### D1: ADR 各 decision に「根拠 trace」を attach し信号機 (🔵🟡🔴) で評価する

ADR の各 decision (`D1`, `D2`, ...) に対して、その根拠を示すフィールド (`user_decision_ref` または `review_finding_ref`) を YAML front-matter に持たせ、次の信号機で評価する。

| 信号 | 根拠タイプ | 条件 |
|---|---|---|
| 🔵 青 | `user_decision_ref` あり | ユーザーが明示承認した (chat segment ref / approval marker 等) |
| 🟡 黄 | `review_finding_ref` あり | review process で発見 → ユーザー未明示だが defensible な根拠がある |
| 🔴 赤 | 根拠なし | orchestrator 独断 — ユーザー未承認、review 由来でもない |

🔴 赤の decision は `bin/sotp verify adr-signals` で CI block 対象とする。`grandfathered: true` を付けた decision はスキップする。

この信号機評価は WF-67 (briefing 側の対策) の補完となる ADR 側の safety net であり、briefing 経由で独断 decision が混入した場合でも CI で検出できる。

### D2: ADR 各 decision に「個別 status」を持たせ lifecycle を encode する

ADR ファイル全体の status (`## Status` セクション、`pre-track-adr-authoring.md` で禁止) とは別 axis として、decision 粒度の lifecycle を YAML front-matter の `status` フィールドで管理する。

decision 個別の `status` 値:

| 値 | 意味 |
|---|---|
| `proposed` | 起草直後 (実装着手前) |
| `accepted` | review 完了、実装可能 |
| `implemented` | 実装完了 (`implemented_in: <commit>` で commit ref 付帯) |
| `superseded` | 後続 ADR の decision に置き換えられた (`superseded_by: <ADR>.md#<id>`) |
| `deprecated` | 廃止 (置き換えなし) |

この仕組みにより次が可能になる:

- **partial supersession**: 同一 ADR ファイル内の D1 だけ `superseded`、D2 は `implemented` という状態を 1 ファイルで表現できる (現行のファイル単位 status では不可)
- **implementation tracking**: どの decision がいつ・どの commit で実装されたか ADR 内で追跡できる
- **deprecation tracking**: 置き換えなしで廃止された decision を encode できる

### D3: ADR フォーマットは MD body + YAML front-matter を採用する

ADR ファイルに YAML front-matter ブロックを追加し、narrative 本文 (MD body) を維持したまま `decisions[]` メタデータを機械パース可能な形で encode する。

front-matter で encode するフィールド (構成):

- `adr_id`: ADR のスラグ識別子
- `decisions[]`: decision ごとのメタデータ配列
  - `id`: `D1` / `D2` 等の decision 識別子
  - `user_decision_ref`: ユーザー明示承認の参照 (type: `chat_segment`, `approval_marker` 等)
  - `review_finding_ref`: review finding の参照
  - `candidate_selection`: 候補 A/B/C からの選択を encode
  - `status`: `proposed` / `accepted` / `implemented` / `superseded` / `deprecated`
  - `superseded_by`: 後続 ADR の decision への参照 (`<adr>.md#<id>` 形式)
  - `implemented_in`: 実装 commit hash
  - `grandfathered`: `true` の場合、verify および status 強制の対象外

MD body は今まで通りの narrative を維持する。

<!-- illustrative, non-canonical -->
```yaml
---
adr_id: 2026-04-27-1234-adr-decision-traceability-lifecycle
decisions:
  - id: D1
    user_decision_ref: { type: chat_segment, ref: "2026-04-25T03:50:00Z" }
    status: proposed
  - id: D2
    user_decision_ref: { type: chat_segment, ref: "2026-04-25T04:30:00Z" }
    status: proposed
  - id: D3
    user_decision_ref: { type: chat_segment, ref: "2026-04-25T04:35:00Z" }
    candidate_selection: { from: [A, B, C, D, E], chose: A }
    status: proposed
  - id: D4
    user_decision_ref: { type: chat_segment, ref: "2026-04-25T05:10:00Z" }
    status: proposed
---
```
<!-- illustrative, non-canonical -->

採用理由:

- MD body は narrative を維持 (人間が読む永続 record としての ADR の本質を損なわない)
- YAML front-matter で機械パースに必要な metadata を encode → `bin/sotp verify adr-signals` は YAML パース 1 発で取得
- 既存 ADR の MD フォーマットを破壊しない (front-matter 追加のみで migration 可能)
- diff レビューで MD body と metadata の変更が分離して見える
- 決定候補比較は `## Rejected Alternatives` を参照

### D4: 既存 ADR には `grandfathered: true` を付けて gradual に back-fill する

`bin/sotp verify adr-signals` を導入する時点で既存 ADR がすべて 🔴 となり CI が全落ちするのを避けるため、既存 ADR の decision には `grandfathered: true` を付けて verify をスキップする。その後、トラック単位で back-fill (根拠 trace フィールドを調査して記入) し、`grandfathered: true` を外して信号評価対象に組み入れる。

`grandfathered: true` は次の条件を満たす場合に使用する:

- 対象 ADR が本 ADR 採択前に作成されたもの
- front-matter の根拠 trace フィールドをさかのぼって確定できない / コストが高い

`grandfathered: true` の解除は強制しないが、ADR が有効な decision を持つ限り back-fill を奨励する。

## Rejected Alternatives

### A. sidecar JSON ファイル方式

**内容**: ADR MD ファイルはそのまま維持し、`<adr-slug>.json` という sidecar ファイルに decision メタデータを格納する。

**却下理由**:

- ADR ファイルと sidecar が分離することで、どちらかだけ更新されて乖離するリスクが生まれる
- `git log`, `git diff` での追跡が 2 ファイルに分散し、レビューコストが増す
- ADR への参照 (`knowledge/adr/<slug>.md`) で sidecar も合わせて参照しなければならない暗黙のルールが発生する

### B. MD body のみ (構造化 table 形式)

**内容**: front-matter を使わず、MD body の中に構造化 table (`| decision | user_decision_ref | status | ...`) を書き、テキスト解析で機械パースする。

**却下理由**:

- MD の table は書式ルールが strict でなく、パーサーが脆弱 (セル内の `|` エスケープ、複数行値など)
- decision table をパースするために ADR ごとにフォーマットが少しずつ異なるリスクがある
- narrative (読み物) と machine-readable data が同一フォーマットに混在し、diff レビューが見づらい

### C. JSON 専用ファイル方式

**内容**: ADR を MD ではなく JSON ファイル (`<adr-slug>.json`) として管理し、`narrative` フィールドに文章を格納する。

**却下理由**:

- ADR の本質は「人間が読む永続 record」であり、JSON は読み物として不適
- GitHub のファイルビューや通常のテキストエディタで読みにくい
- 既存 ADR 60+ 件の移行コストが非常に高い

### D. XML / HTML 形式

**内容**: ADR を XML または HTML ファイルとして管理し、要素属性で decision メタデータを encode する。

**却下理由**:

- XML / HTML は ADR のような narrative ドキュメントとして日常的に書かれない (記述コストが高い)
- JSON / YAML と比べて冗長でレビューがしづらい
- 既存エコシステムとの親和性が低い

### E. agent self-check のみで mitigation する案

**内容**: adr-editor / orchestrator が briefing 実行後に「自分が未承認 decision を加えていないか」をセルフチェックする仕組みだけで密輸を防ぐ。

**却下理由**:

LLM の self-check は over-confidence と blind spot の問題から信頼できない (WF-67 の発生源となった型設計チューニングトラックでも adr-editor は独断追加を自覚していなかった)。CI / verify による機械的検出が構造的解決であり、self-check は補助にしかならない。

## Consequences

### Positive

- **orchestrator 独断の構造的検出**: 🔴 赤 decision が CI で block されることで、briefing 経由の未承認 decision 混入が自動検出できるようになる
- **WF-67 との二重防御**: briefing 側の構造化 (WF-67) と ADR 側の信号機評価 (本 ADR) が補完し合い、密輸リスクを二層で防ぐ
- **decision 粒度の lifecycle 管理**: どの decision がいつ実装されたか / superseded されたかを ADR 内で追跡でき、implementation tracking と partial supersession が可能になる
- **既存 ADR との後方互換性**: `grandfathered: true` による gradual back-fill で、既存 ADR 60+ 件を一括移行せずに段階的に対応できる
- **narrative 維持**: MD body はそのままのため、人間が ADR を読む体験は変わらない

### Negative

- **front-matter 記述コストの増加**: 新規 ADR 作成時に front-matter 記述が必要になる (将来 adr-editor が半自動化されるまではコストが残る)
- **back-fill 作業コスト**: 既存 ADR の `grandfathered: true` 解除は各 decision の根拠をさかのぼる作業が必要で、ADR 数が多いほどコストが大きい
- **GitHub レンダリングで front-matter が非表示**: GitHub は YAML front-matter をレンダリング時に隠す。front-matter の内容を確認するには raw 表示が必要になる

### Neutral

- **MD body フォーマットは変わらない**: 既存の Nygard 式 + Rejected Alternatives + Reassess When 構成はそのまま維持される
- **ADR ファイルの外見は変わらない**: front-matter が追加されるだけで、ADR のファイル数・配置・命名規則は変更されない
- **`## Status` 禁止ルールとの整合**: `pre-track-adr-authoring.md` の「`## Status` セクション禁止」は ADR ファイル全体の承認状態を対象とするルールであり、本 ADR の decision 粒度の `status` フィールドはそのルールの適用外

## Reassess When

- **briefing 機械生成 (WF-67 Phase B) が完成した場合**: orchestrator が briefing を手書きしなくなれば、🔴 赤 decision の発生頻度が大幅に減る。信号機評価の厳格さ (🔴 block) の意義を再評価する余地が生まれる
- **既存 ADR back-fill が一定割合完了した場合 (例: 80% 以上)**: `grandfathered: true` をスキップする仕組みの必要性が薄れ、verify ルールを単純化できるかを検討する
- **将来別 ADR で front-matter の field 構成が大きく変わった場合**: 本 ADR の D3 で示した field 構成が後続 ADR で変更された場合、本 ADR の Consequences / format 説明との整合を確認する
- **`bin/sotp verify` サブコマンド群が大幅に増えた場合**: verify 単体のビルド・テスト・保守コストが上がった時点で、verify サブコマンドの設計方針 (統合 vs 分離) を別 ADR で見直す
- **ADR 数が 200+ になった場合**: back-fill の規模が急増した場合、`grandfathered` 一括解除ではなく優先度付き back-fill の仕組みが必要になる可能性がある

## Related

- `knowledge/conventions/pre-track-adr-authoring.md` — ADR ファイル全体の `## Status` 禁止ルール (本 ADR の D2 decision 個別 status との axis 分離の根拠)
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工状態フィールド廃止原則 (本 ADR の decision 個別 status がこの原則と異なる axis であることの根拠)
- `knowledge/conventions/adr.md` — ADR 基本ルール (Nygard 式フォーマット / lifecycle)
- `knowledge/adr/README.md` — ADR 索引
- `knowledge/adr/2026-04-27-0554-doc-reorganization.md` — style sample として参照した最近の ADR
