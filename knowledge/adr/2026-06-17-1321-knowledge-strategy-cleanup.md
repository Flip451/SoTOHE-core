---
adr_id: 2026-06-17-1321-knowledge-strategy-cleanup
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-17:remove-knowledge-strategy-dir"
    candidate_selection: "from:[remove-dir,rewrite-rules-and-keep,freeze-as-archive] chose:remove-dir"
    status: proposed
  - id: D1.1
    user_decision_ref: "chat:2026-06-17:salvage-before-removal"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-17:remove-knowledge-designs-dir"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-17:remove-knowledge-schemas-dir"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-06-17:past-adr-references-not-updated-on-removal"
    status: proposed
---
# knowledge/strategy ディレクトリの整理方針

## Context

SoTOHE の開発が進むにつれ `knowledge/strategy/` は管理されなくなり、戦略文書としての必要性も薄れてきた。事実、`knowledge/strategy/README.md` 自体が 2026-03-25 以降約 3 ヶ月触れられておらず、その間に増えた中身を索引が反映できていない (15 ファイル中 6 ファイルしか掲載していない)。

## Decision

### D1: `knowledge/strategy/` ディレクトリを撤去する

`knowledge/strategy/` 配下の全 15 ファイルとディレクトリ自体を削除する。戦略文書という概念をプロジェクトのワーキングディレクトリから取り除き、SoTOHE は今後「track / ADR / convention」の 3 つで設計判断と運用ルールを完結させる。

#### D1.1: 撤去前に salvage する

各文書を読み込み、現在の SoTOHE にとって役立つ情報があれば抽出し、`knowledge/research/YYYY-MM-DD-HHMM-<topic>.md` 形式の独立 file として encode してから撤去する。既存 SoT (ADR / convention / CLAUDE.md / track/tech-stack.md など) への inline 追記は重要文書を汚染する risk があるため禁止する。

### D2: `knowledge/designs/` も同様に撤去する

`knowledge/designs/` 配下の 3 ファイル (`auto-mode-*`) も `knowledge/strategy/` と同じ方針で扱う。設計メモが track artifact (`track/items/<id>/spec.md` / `plan.md`) と重複しており、独立して管理する意義が失われている。D1.1 と同様、撤去前に salvage する。

### D3: `knowledge/schemas/` も同様に撤去する

`knowledge/schemas/` 配下の 2 ファイル (`auto-mode-config-schema.md`, `auto-state-schema.md`) も同方針。最終更新は 2026-04-01 で約 2.5 ヶ月触れられていない。D1.1 と同様 salvage 後に撤去する。

### D4: post-merge 過去 ADR への `knowledge/strategy/` 等参照は撤去時に更新しない

main にマージ済みの過去 ADR (`2026-03-30-0546-knowledge-directory-consolidation.md` など) に残る `knowledge/strategy/` / `knowledge/designs/` / `knowledge/schemas/` への参照は撤去後 dead link となるが、追従して書き換えない。ADR Lifecycle の post-merge 不変原則 (`knowledge/conventions/adr.md` §Lifecycle) に従い、過去 ADR は当時の文脈の記録として残す。現役 doc (`knowledge/conventions/adr.md` Decision Reference、`knowledge/README.md`、CLAUDE.md など SoT として運用中の文書) は本決定の対象外であり、撤去に追従して通常通り更新する。

## Rejected Alternatives

### A. 現状維持 (放置)

`knowledge/strategy/` / `designs/` / `schemas/` をそのまま残し、README ルール違反やドラフト残置も追跡対象から外す。

**却下理由**: 既に約 2.5–3 ヶ月触れられておらず、放置しても誰の参照対象にもならない。索引が機能しないまま 15+3+2 = 20 ファイルが累積し続け、新しい開発者が誤って参照する risk を温存するだけ。

### B. 運用ルールを書き直して維持

README の Files 一覧を実態に追従させ、日付サフィックス禁止ルールを再強制、ドラフトを定期的にパージ、`sotp adr suggest` の設計 ADR `2026-03-24-0930-adr-auto-derivation-design.md` を実装するか deprecate するか決める。

**却下理由**: 維持コストに対して受益者が user 自身しかおらず、品質も「正直どうでもいい」と表明されている。「戦略文書」「設計メモ」「スキーマメモ」という独立カテゴリを抱える前提が崩れており、書き直しは投資対効果が合わない。役立つ情報は ADR / convention / track artifact に既に encode され始めている。

### C. 凍結 (read-only archive 化)

`knowledge/strategy/` / `designs/` / `schemas/` を `archive/` のような場所へ一括移動し「frozen, do not edit」と明示する。

**却下理由**: 削除との実質的差分が小さく、archive/ が新たな「触らない雑多置き場」になる。git 履歴で過去版は常に参照可能なので、わざわざ working tree に凍結ディレクトリを持つ価値が低い。

## Consequences

### Positive

- `knowledge/` 配下の working tree が軽くなり、新規参照者の認知負荷が下がる (15+3+2 = 20 ファイル削減)。
- 「戦略文書」「設計メモ」「スキーマメモ」という独立カテゴリ概念がプロジェクトから消え、track / ADR / convention の 3 系統に設計判断と運用ルールが集約される。
- README ルールと実態の乖離・索引不整合・未実装 CLI (`sotp adr suggest`) への依存記述といった負債が一掃される。
- salvage プロセスで現役の知識を `knowledge/research/` の独立 file として再配置でき、既存 SoT (ADR / convention / CLAUDE.md / track/tech-stack.md) を汚染しない。

### Negative

- 過去の文脈 (vision, TODO-PLAN, roadmap, auto-mode 設計メモ等) が working tree から消え、git log を辿らないと参照できなくなる。
- salvage 判定 (役立つか否か) が主観的になり、抽出漏れによる情報損失リスクがある。
- 未実装機能 (`sotp adr suggest`) の設計 ADR `2026-03-24-0930-adr-auto-derivation-design.md` は実装基盤が消えるため、別途 deprecate or supersede の判断が必要になる。
- 現役 doc (`knowledge/conventions/adr.md` Decision Reference、`knowledge/README.md`、`CLAUDE.md` など) は撤去に追従して参照を更新するコストがかかる。
- post-merge 過去 ADR (`2026-03-30-0546-knowledge-directory-consolidation.md` 等) の `knowledge/strategy/` 等参照は dead link として残るが D4 により許容する。

## Reassess When

- 撤去後、salvage しきれなかった情報を参照する必要が頻発し、git log から逐次掘り起こすコストが無視できなくなった場合。
- プロジェクトに新しい SSoT カテゴリ (公開向けロードマップ、外部向け技術ビジョン等) が必要になった場合。
- `knowledge/` ディレクトリ全体の体系を再度見直す動機 (新規カテゴリ追加など) が出た場合。
- track / ADR / convention の 3 系統に集約しきれない種類の知識 (例: 大規模な事前リサーチ・計画メモ) を保存する場所が再度必要になった場合。

## Related

- `knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md` — knowledge ディレクトリ統合 ADR (本決定の前段階整理)
- `knowledge/adr/2026-03-24-0930-adr-auto-derivation-design.md` — `sotp adr suggest` 設計 ADR (実装未着手のまま `knowledge/strategy/` 撤去で前提が変わる)
- `knowledge/conventions/adr.md` — Decision Reference で `knowledge/strategy/knowledge-restructure-design-2026-03-20.md` を cite しており、撤去時に参照を解消する必要がある
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR lifecycle と配置ルール
- `knowledge/README.md` — knowledge 配下ディレクトリ索引 (撤去後に更新)
