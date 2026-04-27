# 運用ドキュメント断捨離方針 — SoT 一本化と narrative 重複の解消

## Context

2026-04-27 に実施した運用ドキュメント (md ファイル群、ADR を除く) 監査で、35 件の不整合 (HIGH 12 / MEDIUM 14 / LOW 9) を検出した。drift の発生源は次の構造的特徴に集約される。

1. **同一トピックに narrative 文書が複数存在する**
   - 例: capability table が 5 か所 (`README.md` / `knowledge/DESIGN.md` / `knowledge/WORKFLOW.md` / `DEVELOPER_AI_WORKFLOW.md` / `.claude/rules/08-orchestration.md`)
   - SoT である `.harness/config/agent-profiles.json` が変わっても 5 か所すべてが追従しない
2. **SoT そのものを restate しているだけの文書がある**
   - 例: `knowledge/DESIGN.md` の Canonical Blocks (Rust 型定義) は `libs/domain/src/*.rs` のコピー
   - 例: `TRACK_TRACEABILITY.md` の Quality Gate list は `Makefile.toml` のコピー
3. **ADR で確定した事実を、narrative 文書が "migration中" / "予定" として残している**
   - 例: `knowledge/DESIGN.md §Migration Path per Track` は完了済みの `track-persistence-2026-03-11` を未完了風に記述
   - 例: `knowledge/DESIGN.md §Auto Mode (MEMO-15 Design Spike)` は実装されず置換 (`auto-cycle-replace-2026-04-10`) された設計を残存

維持コストの非対称性として、SoT (ADR / commands / agent-profiles) は immutable / レビュー必須なので drift 発生率が低いのに対し、narrative 重複は誰でも書き換えられるためコストは低いが drift しやすい。一方で誤情報の影響は新規参入者を misleading するので軽くない (古い `knowledge/WORKFLOW.md` を先に読まれると現行 workflow に到達するまでに混乱する)。

drift の根本原因は「文書を維持する努力が足りない」のではなく「**維持すべき文書の数が多すぎる**」こと。新しい ADR が採択されるたびに 5-7 ファイルを横断更新する運用は限界が近い。`knowledge/WORKFLOW.md` のような英語版 narrative や `knowledge/architecture.md` のような slim 版は責務がはっきりせず、結果としてどれが「正本」か曖昧になっている。

関連:

- `knowledge/conventions/workflow-ceremony-minimization.md` — 「維持コストが価値を上回る ceremony を廃止する」原則の出典 (本 ADR は doc 領域への拡張)
- `knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md` — `.claude/docs/` → `knowledge/` への統合 ADR (本 ADR は同精神の継続)

## Decision

### D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する

運用ドキュメントを 4 ティアに分類し、各 tier に異なる制約を設ける。

| Tier | 役割 | 文書 | 制約 |
|---|---|---|---|
| **0: SoT** (不可侵) | 設計決定 / 挙動定義 / 設定の正本 | ADR (`knowledge/adr/*.md`) / `.claude/commands/**/*.md` / `.claude/skills/**/SKILL.md` / `.claude/agents/**/*.md` / `.claude/rules/` / `.harness/config/agent-profiles.json` / `architecture-rules.json` / `Makefile.toml` / `knowledge/conventions/*.md` / `track/items/<id>/{metadata,spec,*-types,impl-plan,task-coverage}.json` / `track/tech-stack.md` | 内容を他文書に restate しない |
| **1: Entry-Point Index** (≤80 行) | 入口 (新規参入者 / Codex Cloud reviewer の最初の参照点) | `README.md` / `CLAUDE.md` / `START_HERE_HUMAN.md` / `AGENTS.md` | 入口に徹する。workflow / capability / quality gate 等の narrative を含めない (Tier 2 と二重化させない) |
| **2: Operational Narrative** (1 トピック 1 文書) | 運用ストーリー (workflow / dev env / アーキテクチャ概観) | `DEVELOPER_AI_WORKFLOW.md` / `track/workflow.md` / `LOCAL_DEVELOPMENT.md` / `knowledge/DESIGN.md` (heavy shrink 後) | 同じトピックに 2 つ目の文書を作らない。冒頭に「このトピックの SSoT は本ファイル」と明示 |
| **3: Knowledge Base** | research log / strategy notes / design spikes / schema docs / external guide policy | `knowledge/research/*.md` / `knowledge/strategy/*.md` / `knowledge/designs/*.md` / `knowledge/schemas/*.md` / `knowledge/external/*` | サイズ制約なし。陳腐化したら本人が delete する文化 |

Tier 2 の **「1 トピック 1 文書」** が本 ADR の中核。同じトピックに narrative が 2 つ目存在することを許さず、再発したら新規ではなく旧文書を merge してから削除する。

### D2: 即時削除対象 (3 ファイル) を確定する

監査で SoT との重複が明白で unique 情報がほぼゼロだった以下を **削除** する。

1. `knowledge/WORKFLOW.md` — `DEVELOPER_AI_WORKFLOW.md` と全章で重複し、capability 名 / `domain-types.json` 単数形 / Phase 2.5 表記など複数 outdated 記述を含む。英語 maintainer 向け読み物としては `CLAUDE.md` で代替可能 (本 ADR でも `CLAUDE.md` priority references の link 整理を伴う)。
2. `knowledge/architecture.md` — 自称「`knowledge/DESIGN.md` の slim 版」だが両方とも古い。slim 版を残すと「最新ではない」というメタ情報を 2 重に持つだけになる。
3. `TRACK_TRACEABILITY.md` — 内容の 80% が `track/workflow.md` と重複し、`§7 Spec Approval Status` は廃止済み機能 (`spec-approve` task は `Makefile.toml` に存在せず、`approved` 状態は `workflow-ceremony-minimization` で廃止)、`§2 Mapping Rules` の Python script API 説明は `sotp track *` Rust 化済みで stale。**§5 (registry.md Update Rules) のみ `track/workflow.md` に merge** してから削除する。

加えて、`.gitignore` 済み scratch (`repomix-output.*`) も worktree から削除する (副次対応)。

### D3: Heavy shrink 対象 (4 ファイル) のスコープを定める

削除しないが大幅に縮約する。各文書の現状サイズ → 目標サイズ:

1. **`knowledge/DESIGN.md` (~1100 行 → ~150 行)**:
   - 残す: Overview / Architecture diagram (Mermaid) / Module Structure 表 (層と責務のみ、Key Types 列は削除) / Key Design Decisions 表は ADR 索引へのリンクに置換
   - 削除: `## Canonical Blocks` 全節 (`libs/*/src/*.rs` が SoT) / `## Shell Command Guard` historical / `## Security Hardening: Rust Migration` 完了済み migration record / `## Auto Mode (MEMO-15 Design Spike)` (knowledge/designs/ または archive へ移動) / `## Domain Types Registry` (現状の `<layer>-types.json` per-layer 化を反映するか type-designer 定義に集約) / Changelog (ADR 索引が事実上の index)
2. **`README.md` (~140 行 → ~90 行)**:
   - 残す: Project pitch / SoT Chain 4 階層図 / 信号機評価 (🔵🟡🔴) 説明 / クイックスタート (新コマンド体系)
   - 削除 / 修正: capability table 削除 (`.harness/config/agent-profiles.json` リンクのみ) / `tmp/vision-v6-draft.md` 等 tmp/ 参照削除 / `/track:design` → `/track:type-design` / ロードマップは `knowledge/strategy/TODO-PLAN.md` リンクのみ
3. **`START_HERE_HUMAN.md` (~73 行 → ~60 行)**:
   - 残す: 最短 onboarding / 責務境界 / 必須レビュー・承認ポイント / 安全運用ルール
   - 削除 / 修正: 存在しない `docs/` `project-docs/` `.claude/docs/` への言及削除、実在ディレクトリのみ列挙
4. **`LOCAL_DEVELOPMENT.md` (~177 行 → ~90 行)**:
   - 残す: Host Requirements / compose セットアップ / tools-daemon / Useful Commands / Troubleshooting
   - 削除 / 修正: Git Notes 節は `track/workflow.md` 参照リンクのみに変更 / "Phase 5/6 で Rust へ移行済み" 表記は該当 ADR / track id を明示

`DEVELOPER_AI_WORKFLOW.md` は Tier 2 の workflow narrative SSoT として現状サイズを維持し、本文中の重複 capability list 削除と Python optional 統一など細部修正のみ行う。

### D4: SSoT 単一化マッピングを表で定義する

各トピックについて **どのファイルが正本か** を表で固定する。derived 文書は SSoT へのリンクのみ書き、内容を re-state しない。

| トピック | SSoT | derived のルール |
|---|---|---|
| Phase 0-3 workflow の挙動 | `.claude/commands/track/plan.md` | narrative 文書は「Phase 数 + writer 名」のみ参照、内部 step を re-state しない |
| `/track:*` コマンド一覧 | `.claude/commands/track/` (ファイル群) | `DEVELOPER_AI_WORKFLOW.md §0.4` を user-facing aggregator とし、他は記述しない |
| Capability 一覧 + provider | `.harness/config/agent-profiles.json` | `.claude/rules/08-orchestration.md` が役割説明、他は重複しない |
| Capability 役割の説明 | `.claude/rules/08-orchestration.md` | README / CLAUDE.md / WORKFLOW 系には書かない (リンクのみ) |
| Quality gates (verify task list) | `Makefile.toml` (定義) | `track/workflow.md §Quality Gates` を user-facing summary とし、他は記述しない |
| Layer 依存ルール | `architecture-rules.json` | doc では rule の言葉のみ。リスト網羅は SoT のみ |
| Branch strategy | `track/workflow.md §Branch Strategy` | `DEVELOPER_AI_WORKFLOW.md` からはリンクのみ |
| Git notes | `track/workflow.md §Git Notes` | `LOCAL_DEVELOPMENT.md` / `DEVELOPER_AI_WORKFLOW.md` からはリンクのみ |
| `metadata.json` schema | `track/items/<id>/metadata.json` (validation by Rust codec) | doc では `schema_version` 数値のみ参照可 |
| Convention 一覧 | `knowledge/conventions/README.md` (auto-generated index) | ほかの文書では individual convention 名のみ参照 |
| ADR 一覧 | `knowledge/adr/README.md` (manual index) | ほかの文書では individual ADR の path のみ参照 |
| ADR 内の決定事項 | 各 ADR (`knowledge/adr/*.md`) | narrative 文書は `Decision` のみ短く要約、Reject / Reassess は ADR 内に閉じる |

### D5: 再発防止運用ルール (5 条) を確立する

1. **新文書提案前の確認順序**: 新しい運用文書を作る前に、(1) Tier 0 SoT で表現できるか → 可なら SoT に追加、(2) 既存 Tier 2 narrative に統合できるか → 可なら統合、(3) それでも独立した文書が必要な justification は ADR で記録、の順で確認する。
2. **Tier 1 size limit を強制**: `README.md` / `START_HERE_HUMAN.md` / `CLAUDE.md` / `AGENTS.md` は **80 行以下**。超過したら narrative 化されている兆候。Tier 2 への移動か SoT への分散を検討する。将来的に line count check の CI gate 化を検討する。
3. **ADR の `Consequences` に "derived 文書の更新" を必ず含める**: 本 ADR のような大きな workflow / 命名 / 設定変更の `Consequences` セクションに「`README.md` / `DEVELOPER_AI_WORKFLOW.md` / `knowledge/DESIGN.md` 等の derived 文書を更新する」と明記する。これにより ADR 採択時に doc drift が発生しにくくなる。
4. **`tmp/` への永続的参照を禁じる**: `tmp/` は scratch (`.gitignore:77`)。永続的に参照される情報は `knowledge/` または `track/items/<id>/research/` に置く。`README.md` / `CLAUDE.md` / `WORKFLOW` 系で `tmp/...` を参照していたら anti-pattern としてレビューで指摘する。
5. **削除を奨励する文化**: doc が古いと感じたら **修正より削除を優先**。修正は SoT のみで行い、derived は削除して新しい状態を SoT 中心に再構築する。"歴史的記録" は git log / 旧 ADR で取れるので、doc 内で historical 記述を残す必然性は低い。

### D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)

本 ADR は **方針 (decision) と SSoT マッピング (D4) と運用ルール (D5)** までを範囲とし、実装は別 track 群に分割する。track 単位で文書変更の blast radius を制御し、レビュアの認知負荷を下げる。推奨 track 構成:

1. `doc-decluttering-deletes-2026-04-XX` — D2 削除のみ (`knowledge/WORKFLOW.md` / `knowledge/architecture.md` / `repomix-output.*`)
2. `track-traceability-merge-2026-04-XX` — `TRACK_TRACEABILITY.md §5` を `track/workflow.md` に merge してから削除
3. `design-md-shrink-2026-04-XX` — D3 の `knowledge/DESIGN.md` を ~150 行に縮約
4. `readme-and-entry-points-shrink-2026-04-XX` — D3 の `README.md` / `START_HERE_HUMAN.md` / `LOCAL_DEVELOPMENT.md` 縮約 + 監査 HIGH 修正 (`/track:design` → `/track:type-design` 等)
5. `orphan-stragglers-2026-04-XX` — 監査残項目 (`metadata.json schema_version` / `cargo make spec-approve` 参照削除 / `domain-types.json` 単数形置換 / `.claude/docs/` 参照削除)
6. (任意) `doc-rules-enforcement-2026-04-XX` — D5.2 の line count check を CI gate 化

## Rejected Alternatives

### A. derived 文書を auto-regenerate する仕組みを追加する

**内容**: ADR 採択時に SoT を解析して derived 文書 (capability table / quality gate list / コマンド一覧) を自動再生成する linter / generator を作る。`scripts/render_*.py` を Rust 化して `sotp doc render <topic>` のような subcommand で表現する。

**却下理由**:

- 本ハーネスは個人開発 / 小規模運用で、auto-regen 仕組みを保守するコスト (Rust generator + テスト + CI) が、文書を削除して SoT に集約するコストを上回る
- generator の出力フォーマットが narrative ごとに違う (README は宣伝寄り、CLAUDE.md は maintainer index、`.claude/rules/08-orchestration.md` は AI 向け) ため、template で扱える単純な generation にならず複雑化する
- 「文書の数自体を減らす」という根本対策をスキップして対症療法的に複雑性を増やす方向

### B. 各 narrative 文書に "last-synced ADR" メタデータを追加する

**内容**: 各 derived 文書冒頭に「このファイルは ADR `2026-04-27-0554` 時点で同期している」のようなメタデータを入れて、ADR 採択時に CI でこのメタデータと最新 ADR の比較で stale を検出する。

**却下理由**:

- 同期点を記録するだけでは drift は防げない (誰が更新するかが曖昧)
- メタデータ記述自体が新しい drift 発生源になる (人間 / agent が "今同期した" と思い込んでいるが実は同期していない)
- `workflow-ceremony-minimization.md` Rules「人工的な状態フィールドを作らない」(`Status` / `approved` フィールドと同類) に違反する

### C. CI で derived 文書の lint (用語 grep) を追加する

**内容**: ADR で確定した命名 (`/track:type-design`, `spec-designer`, `<layer>-types.json` 等) について、derived 文書で古い表記 (`/track:design`, `designer`, `domain-types.json`) が残っていないかを CI で grep する。

**却下理由**:

- 個別 lint rule の追加は短期的には有効だが、ADR 採択ごとに新しい lint rule を追加する運用が tedious になる
- lint rule 自体が SoT (ADR 内の用語) を track できない (新しい命名が出るたびに lint rule を更新する必要あり)
- 削除 + SSoT 一本化の方が long term の維持コストが低い (lint rule はゼロのまま)

ただし、本 ADR は CI lint を **完全に否定するものではない**。D5.2 の line count check や、将来的に `verify-doc-links` 拡張で SoT への broken link 検出を強化することは別 ADR で検討する余地がある。

### D. 何もしない (現状維持)

**内容**: drift は気付いた時に都度修正する。systemic な再構成はしない。

**却下理由**:

- 監査で 35 件の drift が検出されており、放置すると新規参入者の混乱コスト (古い `knowledge/WORKFLOW.md` を先に読む等) が累積する
- ADR 採択時の更新箇所が平均 5-7 ファイルあり、ADR 数が増えるほど doc 維持コストが線形に増えていく (現状で 60+ ADR)
- "気付いた時に都度修正" は実際には機能していない (本監査は 1 か月以上溜まった drift を初めて systematic に検出した)

### E. multi-language 分離を維持し英語 narrative を別 directory に集約する

**内容**: 日本語 (`DEVELOPER_AI_WORKFLOW.md`, `track/workflow.md`) と英語 (`knowledge/WORKFLOW.md`, `knowledge/DESIGN.md`) を別 directory で運用する。`docs/ja/` `docs/en/` のような構造にする。

**却下理由**:

- 現状のプロジェクトは個人 + 小規模で、日英並行運用のオーバーヘッドが価値を上回る
- 英語が必要な subagent / 外部 reviewer (Codex Cloud) は ADR / `.claude/commands/` / `.claude/rules/` (英語) で十分カバーされている
- マルチランゲージ対応が本格的に必要になった時点で別 ADR (multi-language doc strategy) で再検討する余地あり (Reassess When 参照)

## Consequences

### Positive

- **drift 削減**: 同一トピックの narrative 重複を 5 か所 → 1 か所に集約することで、ADR 採択時の更新箇所が平均 5-7 ファイル → 2-3 ファイルに削減される (見積)
- **新規参入者の混乱削減**: `DEVELOPER_AI_WORKFLOW.md` 1 ファイルが workflow narrative SSoT になることで、古い `knowledge/WORKFLOW.md` を先に読む可能性が消える
- **SoT への信頼回復**: ADR / `.claude/commands/` / `agent-profiles.json` を Tier 0 として確立し、derived は読み流し対象とすることで、各文書を読むときの認知負荷 (どれが最新か) が下がる
- **`workflow-ceremony-minimization.md` の精神を doc 領域に拡張**: 同 convention の「維持コストが価値を上回る ceremony を廃止」ルールを doc reorganization にも適用することで、convention の運用範囲が一貫する
- **knowledge/ directory consolidation の継続**: ADR `2026-03-30-0546-knowledge-directory-consolidation.md` で開始した consolidation を doc narrative にも拡張する形になり、`.claude/docs/` 残骸 (`knowledge/DESIGN.md` 内の参照) も cleanup される

### Negative

- **削除した文書の "歴史" が一時的に探しにくくなる**: `knowledge/WORKFLOW.md` の英語 narrative を参照していた habit があった場合、 git log で当時の状態を辿る手間が発生する (ただし削除 commit に明記すれば mitigation)
- **Tier 1 size limit (80 行) が運用上の制約になる**: `README.md` / `CLAUDE.md` 等が肥大化してきた時に「narrative を Tier 2 に移す or SoT に分散する」判断を都度行う必要がある。size 80 という数字はやや arbitrary
- **実装 track が 5-6 個に分割される**: 本 ADR の Decision は方針のみで、実装は track ごとに分割する (D6)。各 track の bootstrap / review / merge コストが累積する (ただし blast radius 制御の代償として許容)
- **英語 maintainer (もしくは Codex Cloud reviewer) 向けの英語 narrative が消える**: `knowledge/WORKFLOW.md` 削除で英語の workflow 概観が `CLAUDE.md` のみになる。`AGENTS.md` (Codex Cloud 向け) は維持されるので reviewer 用途は影響軽微だが、maintainer 視点では英語 narrative の選択肢が縮む

### Neutral

- **Tier 0 SoT は変更されない**: ADR / commands / agent-profiles.json / Makefile.toml / architecture-rules.json はそのまま
- **track-specific docs (`track/items/<id>/`) は影響を受けない**: 本 ADR は cross-cutting な運用 doc 群のみが scope。各 track の artifact は独立
- **`knowledge/strategy/`, `knowledge/research/`, `knowledge/designs/`, `knowledge/schemas/`, `knowledge/external/` も影響を受けない**: 本 ADR の scope は Tier 1 / 2 narrative のみ。Tier 3 knowledge base はサイズ制約も統合義務もない (memory `feedback_strategy_docs_low_quality_bar.md` のとおり)
- **既存 ADR / convention / commands / skills は変更されない**: 本 ADR は doc reorganization 専用で、設計決定や挙動定義には触れない

## Reassess When

- **Tier 0 SoT が大幅に増えた場合 (例: ADR 数が 200+ になる)**: ADR README の手動 index 維持が現実的でなくなる。auto-generate を Rejected Alternatives A で却下したが、ADR 数が一定を超えたら index 自動化は再検討する
- **multi-language 対応が本格的に必要になった場合**: 現在は日本語中心だが、外部 contributor (英語話者) 増加で英語 narrative 復活の必要が出たら Rejected Alternatives E を再評価する
- **Tier 1 size limit (80 行) が運用上きつくなった場合**: 例えば `README.md` で SoT Chain 説明だけで 80 行を超えるようになったら、limit の見直しか分割 (`README.md` + `OVERVIEW.md`) を検討する
- **ADR 採択頻度が下がり drift コストが小さくなった場合**: 本 ADR の前提 (ADR 採択時の更新コストが高い) が崩れる場合、削除した narrative の復活を検討する余地あり
- **`auto-cycle-replace` のような design spike → 別実装で置換 のパターンが頻発した場合**: `knowledge/DESIGN.md` の Auto Mode 節を archive 移動するルールが追いつかなくなる。design spike の lifecycle ガバナンスを別 ADR で扱う
- **`workflow-ceremony-minimization.md` 自体が改訂された場合**: 本 ADR の根拠 convention が変わると、Tier 0/1/2/3 の境界も見直しが必要

## Related

- `knowledge/conventions/workflow-ceremony-minimization.md` — 「維持コストが価値を上回る ceremony を廃止」原則の出典
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR を track 前段階で作成するルール (本 ADR は本 convention に従って authored)
- `knowledge/adr/README.md` — ADR 索引
- `knowledge/adr/2026-03-30-0546-knowledge-directory-consolidation.md` — `.claude/docs/` → `knowledge/` 統合 (本 ADR は同精神を doc narrative に拡張)
- `knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md` — `/track:design` → `/track:type-design` 命名変更 (本 ADR の D3 / D4 修正対象の根拠)
- `knowledge/adr/2026-04-09-2235-agent-profiles-redesign.md` — capability mapping を `.harness/config/agent-profiles.json` に集約 (本 ADR の D4 の根拠)
- `.claude/commands/track/plan.md` — Phase 0-3 の挙動 SSoT (D4 の最上位 entry)
- `Makefile.toml` — task の SSoT (D4 の Quality gates entry)
