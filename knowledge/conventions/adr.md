# Convention: Architecture Decision Records (ADR)

## Rule

設計判断は `knowledge/adr/` に ADR として記録する。

## When to Write an ADR

- 技術選定（クレート、ツール、フレームワーク）
- アーキテクチャ変更（層構造、依存方向、モジュール分割）
- ワークフロー変更（Phase 戦略、トラック運用方針）
- 却下した選択肢が将来再検討される可能性がある判断

## Format

- Nygard 式 + Rejected Alternatives + Reassess When
- 採番: `YYYY-MM-DD-HHMM-slug.md`
- テンプレートと索引: `knowledge/adr/README.md`
- 機械検証可能な decision メタデータは YAML front-matter (下記) で encode する

## YAML front-matter

各 ADR ファイルは MD body の前に YAML front-matter ブロックを置く。`bin/sotp verify adr-signals` がこの front-matter を読み取って各 decision の根拠 trace と lifecycle を検証する。

front-matter の起点は `2026-04-27-1234-adr-decision-traceability-lifecycle.md` D1-D3。

### Schema

top-level key は以下 2 つのみ (`deny_unknown_fields`):

- `adr_id` (required, non-empty string): スラグ識別子。通常はファイル名から `.md` を除いたもの。
- `decisions[]` (optional list, default empty): 各 decision のメタデータ配列。本文の `### D<n>` 1 つにつき 1 entry を対応させる。

`decisions[]` 各 entry のフィールド:

| field | type | 必須条件 | 意味 |
|---|---|---|---|
| `id` | non-empty string | 必須 | decision 識別子 (`D1`, `D2`, ...; grandfathered legacy では `<file-stem>_grandfathered` 等)。同一 ADR 内で unique であれば良い。 |
| `user_decision_ref` | string | optional | ユーザー明示承認の参照 (chat segment ref, approval marker 等)。値が non-null なら 🔵 Blue。 |
| `review_finding_ref` | string | optional | review process で発見された根拠の参照。値が non-null かつ `user_decision_ref` 未設定なら 🟡 Yellow。 |
| `candidate_selection` | string | optional | `## Rejected Alternatives` で評価した候補からの選択 (例: `"from:[A,B,C,D,E] chose:A"`)。 |
| `status` | string | 必須 | `proposed` / `accepted` / `implemented` / `superseded` / `deprecated` のいずれか。それ以外は parse 時に reject。 |
| `superseded_by` | string | `status: superseded` のとき必須、他では禁止 | 後継 decision への参照 (`<adr-slug>.md#<id>` 形式)。`null` も禁止 (raw key-presence check)。 |
| `implemented_in` | string | `status: implemented` のとき必須、他では禁止 | 実装を identify する non-empty な commit hash / reference。`null` も禁止 (raw key-presence check)。 |
| `grandfathered` | bool | optional | `true` のとき verify-adr-signals の Red/Yellow 判定対象から除外 (D4 exemption)。 |

### Status と typestate dispatch

`status` の 5 値はそれぞれ domain typestate variant に dispatch される:

| `status` 値 | 対応 typestate | 補足 |
|---|---|---|
| `proposed` | `ProposedDecision` | 起草直後 (実装着手前) |
| `accepted` | `AcceptedDecision` | review 完了、実装可能 |
| `implemented` | `ImplementedDecision` | 実装完了 (`implemented_in` 必須) |
| `superseded` | `SupersededDecision` | 後続に置き換え (`superseded_by` 必須) |
| `deprecated` | `DeprecatedDecision` | 置き換えなしで廃止 |

dispatch の実体は `parse_adr_frontmatter`(`libs/infrastructure/src/adr_decision/parse.rs`) の status-string match で行われる。

### decision 個別 status とファイル全体 status の関係

`pre-track-adr-authoring.md` はファイル全体の `## Status` 見出し (`Proposed` / `Accepted` / 等の summary status) を**禁止**している。本 convention の `decisions[].status` はそれとは別 axis で、**decision 粒度** の lifecycle を表す:

- ファイル全体の summary status は形骸化しやすく (実装が進んでも誰も更新しない)、機械検証もできなかった → 廃止。
- decision 個別 status は機械検証可能 (`verify-adr-signals` が typestate dispatch を通じて評価) であり、partial supersession (同一 ADR 内の D1 だけ superseded、D2 は active) や implementation tracking (`implemented_in` で commit を指す) を encode できる → 追加。

`workflow-ceremony-minimization.md` の「人工的な状態フィールドを作らない」原則とも衝突しない:

- 同原則の対象は **形骸化する file-level / summary-level の状態フィールド** (実成果物と乖離するもの)。
- decision 個別 status は機械検証で schema 整合性が CI で確認される (例: `status: implemented` なら `implemented_in` 必須 / 非 empty、欠落すれば parse 失敗) ため、形骸化リスクが構造的に低減されている。ただし `implemented_in` に記載した commit hash の実在や `superseded_by` の参照先の解決は現 CI では検証しない。

### Grounds requirement

各 `decisions[]` entry は次のいずれかを満たさなければならない:

1. `user_decision_ref` が non-null (Blue), または
2. `review_finding_ref` が non-null (Yellow), または
3. `grandfathered: true` (exempt)

3 つすべてに該当しない decision は 🔴 Red と評価され、`cargo make verify-adr-signals` が non-zero exit して CI を block する。

### grandfathered 用途

`grandfathered: true` は次の場合に限定して使う (D4):

- 対象 ADR が本 front-matter フォーマット採択前に作成されたもの
- 根拠 trace フィールド (`user_decision_ref` / `review_finding_ref`) を遡って確定するコストが高い

`grandfathered` な decision は verify-adr-signals の signal evaluation 対象から除外され、CI block を起こさない。新規 ADR では使用せず、必ず `user_decision_ref` または `review_finding_ref` を埋める。

### Fail-closed 振る舞い

`bin/sotp verify adr-signals` は `knowledge/adr/` 配下の全 `*.md` を走査する。front-matter のない `.md` ファイルは **即座に fail** する — `parse_adr_frontmatter` が内部で `MissingAdrId` を返し、filesystem adapter がそれを read error として伝播させる (新規 ADR が未記入のまま slip-through するのを防ぐ fail-closed 設計, CN-04)。

新規 ADR を追加する際は必ず先に front-matter を入れる。`grandfathered: true` は本 front-matter フォーマット採択前の既存 ADR に遡って back-fill する場合にのみ使用できる (`grandfathered 用途` セクション参照)。新規 ADR では必ず `user_decision_ref` または `review_finding_ref` を埋める。

### 例

```yaml
---
adr_id: 2026-04-27-1234-example
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-XXX:2026-04-27"
    status: accepted
  - id: D2
    review_finding_ref: "RF-12"
    status: proposed
---
# ADR title

(MD body は従来通り)
```

## ADR vs Convention

| | ADR | Convention |
|---|---|---|
| 問い | 「なぜこうした？」 | 「これからどうする？」 |
| 時制 | 過去形 | 現在形 |
| 寿命 | 永続（superseded でも残る） | 現行ルールのみ有効 |

Convention から関連 ADR にリンクするには `## Decision Reference` セクションを追加する。

## Lifecycle: pre-merge draft vs post-merge record

ADR が `main` にマージされているかで扱いが変わる:

- **Pre-merge (current working branch / open PR)**: ADR はまだ draft。 レビューや実装で欠陥・矛盾・見落としが判明したら **同じファイルを直接編集** して構わない。新 ADR で supersede する必要はない。pre-merge の段階で design を整える目的に沿う。
  - 判定: `git log main -- <adr-file>` が empty (当該 ADR が main に存在しない) なら pre-merge 扱い
- **Post-merge (merged to `main`)**: ADR は永続 record として不変。semantic content の変更は新 ADR で supersede または refinement する。既存 ADR に許容される編集は (1) typo 修正、(2) broken cross-reference 修正、(3) newer ADR への back-reference 追加 のみ。
  - 新 ADR は `## Related` で旧 ADR を参照。旧 ADR は当時の decision の歴史 record として残す。

この使い分けは `.claude/agents/adr-editor.md` の editing rules にも反映されている。

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR テンプレート・索引
- [knowledge-restructure-design-2026-03-20.md](../strategy/knowledge-restructure-design-2026-03-20.md) — 元の設計メモ
