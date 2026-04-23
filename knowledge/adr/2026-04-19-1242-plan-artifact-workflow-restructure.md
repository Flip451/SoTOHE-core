# 計画成果物ワークフローの再構築 — SoT Chain に沿ったフェーズ分離

> **状態**: Draft (ADR 化済み、推敲中)
>
> **起草日**: 2026-04-18
>
> **ADR 化日**: 2026-04-19
>
> **本 ADR の要点**:
> 1. README の SoT Chain (ADR ← 仕様書 ← 型契約 ← 実装) を厳格に守る。仕様書から型カタログへの参照は逆流として禁止
> 2. `approved` / `Status` のような状態フィールドを全廃。現状は自動で approved にされており実質的なゲートとして機能していない
> 3. ADR は「書き換え可能だが原則安定」として扱い、凍結を強制する仕組みは作らない
> 4. Phase 3 の成果物 (tasks と task_refs) を独立ファイル (`impl-plan.json`, `task-coverage.json`) に分離。`metadata.json` は track identity のみ保持、`spec.json` は Phase 3 で書き戻さない (coverage annotation は `task-coverage.json` へ)。下流で問題が検出されたときの spec 修正は正常な探索的精緻化として許容
> 5. `track/items/<id>/templates/` のような track 固有のサブディレクトリを廃案 (使用実績なし)
> 6. CI コミットゲートを「file 存在 = phase 状態」方式に再定義。optional field の検証ロジックが不要になる

## 下位決定ごとの状態

| 下位決定 | 状態 |
|---|---|
| D0: track 前段階 ADR + 3 フェーズ構成への再編 | 提案 (本 ADR の幹) |
| D1: フェーズごとの SSoT 責務分離 (SoT Chain 準拠) | 提案 |
| D2: 強制機構 (構造化参照 + 検証 CLI) | 提案 |
| D3: フェーズごとのゲート (approved 廃止) | 提案 |
| D4: フェーズごとの作成担当 | 提案 |
| D5: 形式的手順の最小化原則 | 提案 |
| D6: コミットゲートの phase 対応緩和 + 空カタログ許容 | 提案 (D0 依存) |

## 関連参照

- `README.md` §SoT Chain (参照方向の出典、本 ADR の根拠)
- `.claude/skills/track-plan/SKILL.md`
- `.claude/commands/track/plan.md` / `.claude/commands/track/design.md`
- `.claude/agents/planner.md` / `.claude/agents/designer.md` / `.claude/agents/adr-editor.md` (新設予定、D4 / 展開フェーズ 6.5)
- `.claude/rules/01-language.md:28` (`## Canonical Blocks` ルール — 本 ADR では扱わず別 ADR で再検討)
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` (型カタログスキーマの土台)
- `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` (TDDD シグナルの CI 接続と信号機評価の型カタログからの分離を扱う。本 ADR とは独立に進行可能。相互参照)
- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D9 / §D9.1 (現行の merge 時 task-completion gate `check_tasks_resolved_from_git_ref` の原典。本 ADR は tasks[] の移設に合わせて impl-plan.json 読み替えを行う、D6.2 参照)
- 関連する記憶: `feedback_no_unnecessary_stops`, `feedback_auto_cancel_permission`, `feedback_enforce_by_mechanism`, `feedback_invest_in_future_flow`
- 観測データ: `track/items/review-scope-prompt-injection-2026-04-18/review.json`

---

## 背景

### §1 観測されている 3 つの問題

#### §1.1 問題 A: 計画成果物の乖離によるレビューの長期化

`review-scope-prompt-injection-2026-04-18` の `plan-artifacts` scope は **26 ラウンド** 消費し、`zero_findings` は 3 回しか打てていない。指摘の分類:

| 原因の種類 | 件数 | 典型例 |
|---|---|---|
| canonical block の出典揺れ | 約 12 | spec T007 が ADR D3 を引用、実体は research note にある |
| scope 境界の食い違い | 約 6 | ADR D1 の schema / spec の Goal / verification の間で内容が不一致 |
| 立ち上げ時の状態記述が古い | 約 4 | 「plan-artifacts 無し」と書かれているが実ファイルには既にある |
| タイムスタンプの不整合 | 約 3 | `metadata.updated_at` が `spec.approved_at` より古い |
| 型設計の矛盾 | 約 2 | `ScopeEntry` の `Clone` 有無で research 内部にも不一致 |
| 検証ゲートの設計瑕疵 | 約 2 | `verify-doc-links` の適用範囲のずれ |

上位 2 種 (合計 18 件) は、同じ canonical block が複数の成果物に本文内で複製されていることに起因する。

#### §1.2 問題 B: `/track:plan` と `/track:design` の事前承認手続き

現行の `/track:plan` SKILL.md は「要約提示 → 承認待ち → 生成」の 2 ターン手続きを持つ。`/track:design` も同様に「Present the type design to the user for review before writing」を明示しており、層ごとに承認を取る。満足ケースでは毎回この余剰コストが発生する。

加えて、要約ベースの事前承認は実成果物と乖離しうる (レビュー対象が完成前の抽象表現になるため)。問題 A の canonical block ドリフトも、成果物の実物よりも要約を軸に議論が進むことで見逃されやすい構造に起因する。

#### §1.3 問題 C: 仕様・設計・実装計画のフェーズ混在

track の成果物生成は本来 3 つの異なる関心事 (= 3 つのフェーズ) を扱う:

| フェーズ | 成果物の種類 | 責務 |
|---|---|---|
| **仕様 (振る舞いの契約)** | spec.json | 機能が満たすべき振る舞い / 受入基準 / 制約 |
| **設計 (型の契約)** | 型カタログ (`<layer>-types.json`) | 振る舞いを実現する型構造 |
| **実装計画** | metadata.tasks[] | 実装を分解する進行マーカー (task) |

これら 3 フェーズには依存関係がある: **仕様 → 設計 → 実装計画** の順で確定するのが自然 (task は型に依存、型は振る舞いに依存)。

しかし現行の `/track:plan` はこの 3 フェーズを混在処理しており、特に `metadata.tasks[]` を型カタログより先に確定する。task は型に依存しているのに型未確定のまま分解される結果、中間成果物 (canonical block) が必要になり、問題 A の構造的な温床になっている。また `/track:design` は型カタログを後付けで生成するが、このときには task が既に固定されており、フェーズ順序が逆転している。

### §2 根本原因の分析

| 層 | 根本原因 |
|---|---|
| L0a | ワークフロー順序の逆転 (task が型より先に決まる) |
| L0b | `approved` / `Status` 状態による形骸化ゲート (自動で approved にされ、実質的なゲートになっていない) |
| L1 | SSoT (真実の唯一源) が暗黙で、各成果物の責務 (振る舞い / 型 / 実装計画など) がどのファイルに帰属するかが明文化されていない |
| L2 | 複製の許容 (canonical block を本文内に直書きするか別ファイルに保存するか、どちらも許されている) |
| L3 | 参照検証の欠如 (`sources[]` が文字列の列挙で、CI による検証がない) |

本 ADR は各根本原因をそれぞれ個別の決定で対処する:

| 根本原因 | 対処する決定 |
|---|---|
| L0a (workflow 順序逆転) | D0 (phase 再編) |
| L0b (approved 形骸化) | D1.2 / D3 (approved 廃止) |
| L1 (SSoT 宣言不在) | D1 (per-phase SSoT 責務分離) |
| L2 (複製の許容) | 本 ADR では扱わず、別 ADR で canonical block の扱いを検討 (Q14) |
| L3 (参照検証の欠如) | D2 (構造化参照 + 検証 CLI) |

### §3 設計原則

以下の原則をすべて満たす:

1. **SoT Chain の一方向依存 (README が出典)**: 下流の成果物だけが上流を参照する。逆方向の参照と層飛ばしは禁止 (詳細は D1.5)

   ```
   ADR
     ↑ ①
   仕様書
     ↑ ②
   型契約
     ↑ ③
   実装
   ```

   各番号は以下の参照に対応する:

   | 番号 | 参照元 → 参照先 |
   |---|---|
   | ① | 仕様書 → ADR |
   | ② | 型契約 → 仕様書 |
   | ③ | 実装 → 型契約 |

   以下本 ADR 内で「SoT Chain ①/②/③」と書くときはこの対応を指す。

2. **ワークフロー境界と寿命の整合**: track 横断成果物 (ADR など) のライフサイクルは track 内成果物から独立させる (詳細は D1.1 / D4)
3. **フェーズごとの SSoT**: 各フェーズは決められた成果物の責務だけを持ち、他フェーズの責務に踏み込まない
4. **ゲート方式の分離**: SoT Chain ①②③ の参照評価はシグナル (🔵🟡🔴) で段階判定、その他の整合性は binary (OK / ERROR) で判定する。人工的な `approved` 状態は作らない (詳細は D3)
5. **文書の正規化**: 同じ内容を複数の成果物に本文内で複製しない。参照は構造化する (詳細は D2.1)
6. **機構による強制** (`feedback_enforce_by_mechanism`): ルールを書くだけでは乖離は止まらない。CI とスキーマで強制する
7. **人工的な凍結を設けない**: ADR や spec を「不変」「凍結」と宣言しない。下流フィードバックで上流を修正するのが正常 (詳細は D0.1)

### §4 成果物のライフサイクル 3 層区別

| 層 | 場所 | ライフサイクルの性格 | 適用範囲 |
|---|---|---|---|
| **track 横断 (恒久的だが書き換え可)** | `knowledge/adr/`, `knowledge/conventions/`, `knowledge/DESIGN.md`, `knowledge/research/*.md` (track 横断的な分析のみ) | 書き換え可能だが原則として安定。ADR は大きな変更のときに新規作成 (D1.1) | 全 track |
| **track 内** | `track/items/<id>/` (metadata.json, spec.json, `<layer>-types.json`, impl-plan.json, task-coverage.json, plan.md, spec.md, verification.md, research/) | 各ファイルは 1 つの phase で生成される。下流で問題が検出された場合は back-and-forth で上流を修正することがある (D0.1)。track が done になったら historical 扱いに | その track のみ |
| **track 着手前 (作業記録)** | `track/items/<id>/research/<timestamp>-<capability>-*.md` | planner 作業中は更新可、track が done になったら historical 扱いに | その track の planner のみ |

**ADR の書き換え可能性 (探索的精緻化の対象)**: ADR は「書き換え可能だが原則安定」として扱う。実運用では下流 (spec / 型カタログ / 実装) からのフィードバックで ADR 自体を書き換える必要が出ることがあり、これは README §探索的精緻化ループの正常な挙動。凍結を強制する仕組みは作らない (D0.1 参照)。

### §5 用語

| 用語 | 意味 |
|---|---|
| track 前段階 | `/track:plan` 実行前の作業。必要なら ADR の作成も含む |
| 型カタログ (catalogue) | `track/items/<id>/<layer>-types.json` の群。型の SSoT |
| カタログエントリ | カタログ内の 1 つの型の JSON エントリ |
| 振る舞い契約 | spec.json の振る舞いレベルの契約 (型詳細は含まない) |
| impl-plan.json | Phase 3 で生成する実装計画ファイル。`tasks[]` と `plan.sections` を保持 |
| task-coverage.json | Phase 3 で生成する coverage ファイル。spec 要素 (in_scope / out_of_scope / constraints / acceptance_criteria) ごとの task_refs を保持 |
| 構造化参照 | 4 独立構造体 (`AdrRef` / `ConventionRef` / `SpecRef` / `InformalGroundRef`) で表現する参照エントリ (D2.1)。使用サイトごとに専用 field (`adr_refs[]` / `convention_refs[]` / `spec_refs[]` / `related_conventions[]` / `informal_grounds[]`) で保持 |
| 事後レビュー方式 | 成果物を生成してから実物をユーザーに見せて修正指示を受けるレビュー方式 |

---

## 決定

### D0: track 前段階 ADR + 3 フェーズ構成への再編

#### D0.0: フェーズ定義

| 段階 | 独立コマンド | 作成ファイル | 担当 | 参照先 | ゲート | 備考 (ファイル内容は D1.X 参照) |
|---|---|---|---|---|---|---|
| track 前段階 | (なし、`/track:plan` の外で対話 / 手動) | `knowledge/adr/*.md` | ユーザー + main 対話 / 手動 | — | — (user の手作業に判定ロジックは無い) | 詳細 D1.1。track 横断、書き換え可、探索的精緻化の対象 (§4 / D0.1) |
| フェーズ 0: 初期化 | `/track:init` (新設) | `track/items/<id>/metadata.json` | main | — | /track:plan 起動時に参照予定の ADR 存在を確認。厳密モード: ADR 未整備で /track:plan 停止 (Rejected U) | 詳細 D1.4 |
| フェーズ 1: 振る舞い契約 | `/track:spec` (新設) | `track/items/<id>/spec.json` | planner | ADR + convention (SoT Chain ①) | 仕様書 → ADR signal (D3.1) | 詳細 D1.2 |
| フェーズ 2: 型設計 | `/track:design` (既存、責務を Phase 2 専任に) | `track/items/<id>/<layer>-types.json` | designer | spec (SoT Chain ②) | 本 ADR では file/schema 存在のみ (signal は別 ADR、D3.2) | 詳細 D1.3。非型成果物 (prompt / SQL 等) は別議題 (Q14) |
| フェーズ 3: 実装計画 | `/track:impl-plan` (新設) | `track/items/<id>/impl-plan.json` + `track/items/<id>/task-coverage.json` | planner | 型カタログ + spec | task-coverage binary pass/fail (D3.3) | 詳細 D1.4。新規ファイル 2 つのみ作成、spec / metadata / 型カタログは書き換えない |
| フェーズ 4 以降 | `/track:implement` (既存) | 実装コード (既存) | 実装者 (既存フロー) | 型カタログ (SoT Chain ③、rustdoc 突合) | 実装 → 型カタログ signal (D3.4) | 既存フロー、本 ADR の改訂対象外 |

`/track:plan` は上記独立コマンドを順次 invoke する **orchestrator** (コマンド境界セクション参照)。各独立コマンドは単独でも呼び出し可能で、back-and-forth (D0.1) での元 writer 再 invoke と直結する。

#### D0.0.1: File 存在 = Phase 状態 の対応

ADR は事前条件として `knowledge/adr/` 側に存在する前提。`track/items/<id>/` 内のファイル存在だけを見れば phase 判定ができる:

| 時点 | `track/items/<id>/` に存在するファイル |
|---|---|
| /track:plan 起動前 | (なし、ADR は `knowledge/adr/` に存在) |
| フェーズ 0 完了 (初期化直後) | `metadata.json` |
| フェーズ 1 完了 | + `spec.json` |
| フェーズ 2 完了 | + `<layer>-types.json` |
| フェーズ 3 完了 (= /track:plan 完了) | + `impl-plan.json` + `task-coverage.json` |

optional field の有無を検査する代わりに、file 存在を見るだけで phase 判定できる。

#### D0.1: フェーズをまたいだ戻り作業 (探索的精緻化ループ)

ADR から実装までのすべての層は README §探索的精緻化ループと同じく **↑↓ 両方向** の遷移を持つ。下流で上流への乖離が SoT Chain 信号機評価で検出されたときは **逐次伝播** で進める:

1. 下流で上流への SoT Chain 信号機評価が **🔴** になったら、**1 つ上の層だけ**を修正する (いきなり根本原因を推定して飛ばさない)。🟡 の場合は警告ログを残して次フェーズへ進む (commit 可、merge 前解消が必要。後の手順は D3.1 / `/track:plan` 内部フロー参照)
2. その修正でさらに上の層との乖離が生じたら (新たに 🔴 が出たら)、もう 1 つ上に遷移して修正する
3. 対象層の信号機評価が改善したら、下流に遷移して作業を再開する

**例**:

- フェーズ 3 で 型契約 → 仕様書 signal が 🟡 → 警告ログを残して次フェーズへ進む (merge 前に解消必須)
- フェーズ 3 で 型契約 → 仕様書 signal が 🔴 → 型カタログを修正
- その結果、仕様書 → ADR signal が 🔴 になった → spec を修正
- spec 修正で 仕様書 → ADR signal が 🔵 に回復 → 型カタログに戻って作業再開

この逐次伝播によって、探索的な修正が最小の層に閉じる (根本原因に飛ばないことで、影響範囲を signal で段階的に特定できる)。

**改まった承認手続きは不要**: 信号機評価が自然に下流の無効化を知らせる。ADR についても「凍結された決定」ではなく、下流のフィードバックで修正されうる (§3 原則 7、D1.1)。

**修正の実行者**: 上流修正は常に **その成果物の元 writer (D4 table 参照)** を再 invoke して行う。main orchestrator が直接 Edit で修正してはならない。

理由:
- 軽微な修正でも signal 評価や下流への影響を正しく判断できるのは specialist
- main が直接編集すると 2 人目の writer が artifact に介入する形になり、D4 の「1 ファイル = 1 writer」原則が崩れる
- back-and-forth の各 step は specialist による signal 解釈 → 修正 → 再評価のループで進むべき

層ごとの実行者:

| 修正対象 | 実行者 |
|---|---|
| spec.json | planner サブエージェント |
| `<layer>-types.json` | designer サブエージェント |
| ADR (`knowledge/adr/*.md`) | adr-editor サブエージェント |

**ADR 自動書き換えの書き手の指定**: ADR の back-and-forth 修正は adr-editor サブエージェントが担う。spec / 型カタログと writer を分離することで、D4 の「1 ファイル = 1 writer」原則を ADR についても満たす (adr-editor capability の新設は展開フェーズ 6.5)。

**ADR 自動編集の判定基準**: adr-editor による自動編集の可否は以下の git 履歴ベースの機械的な基準で判定する (主観的な「decision 変更の大きさ」では判定しない):
- ADR ファイルに commit 履歴あり → auto-edit (loop 中は working tree のみ編集、コミットはしない)
- ADR ファイルに commit 履歴なし → user pause (ADR を先に commit してから再開)
- /track:plan 終了時に ADR working tree に HEAD からの diff があれば user に判断を仰ぐ (accept / revert / 手動修正 / 中止)

#### D0.2: 省略ルール

- 既存 ADR で十分なら track 前段階で新規 ADR を作らない
- 既存型のみを使う track ではフェーズ 2 は空カタログまたは `action: reference` のみでよい (空カタログの受け入れは D6.4、実装ドリフトは SoT Chain ③ で検出)

### D1: フェーズごとの SSoT 責務分離 (SoT Chain 準拠)

#### D1.1: ADR — 決定文書 (状態なし、書き換え可)

- **配置**: `knowledge/adr/*.md` (track 横断、恒久的だが書き換え可)
- **含めるもの**: 背景、決定 (D sections)、却下した代替案、影響、未解決の論点
- **含めないもの**: trait / struct / method の完全な仕様を本文内に書くこと
- **例示のみの記述**: `<!-- illustrative, non-canonical -->` コメントを必須にする
- **他成果物への参照**: ADR から track 内成果物への直接参照は禁止 (D1.5)
- **作成タイミング**: 初期作成は `/track:plan` の外 (D0.0 の「track 前段階」行参照)。back-and-forth での修正 (下流 signal 🔴 起点) は `/track:plan` 内から adr-editor サブエージェントを自動 invoke して実施 (D0.1 / D4 参照)。auto-edit は ADR ファイルに commit 履歴がある場合のみ (無ければ user pause)
- **状態フィールドなし**: `Status` 見出しは作らない。ファイルが存在し内容が読める状態が運用上の「承認」
- **改訂と探索的精緻化**: 既存 ADR を直接書き換えてよい。大きな決定変更のときは新しい ADR を作り、旧 ADR に `## Follow-up` で参照を書いてもよい (任意)。ADR も下流 (spec / 型カタログ / 実装) のフィードバックで修正されうる探索的精緻化ループの対象 (D0.1)

#### D1.2: spec.json — 振る舞い契約

- **配置**: `track/items/<id>/spec.json` (track 内)
- **含めるもの**:
  - `goal[]`, `scope.in_scope[]`, `scope.out_of_scope[]`, `constraints[]`, `acceptance_criteria[]` — 各要素は `{id: SpecElementId, text, adr_refs: Vec<AdrRef>, convention_refs: Vec<ConventionRef>, informal_grounds: Vec<InformalGroundRef>}` (Q13 [a] 確定で `SpecElementId` を付与、ref 構造体は D2.1 参照)
  - `informal_grounds[]` 非空 → 仕様書 → ADR signal が 🟡 (未永続化根拠あり、D3.1)。merge 前に formal ref (AdrRef / ConventionRef) へ昇格要
  - `related_conventions[]` — 各要素は D2.1 の `ConventionRef`。現行の「ファイルパス文字列配列」から変更。`ConventionRef.hash` の追加および `ConventionAnchor` の semantic 厳密化は Q15
  - (`domain_states` は既に型カタログに移譲済み、`knowledge/adr/2026-04-07-0045-domain-types-separation.md` 参照)
- **含めないもの**: 型の struct / method / カタログエントリの本文内複製 / `task_refs[]` (Phase 3 で `task-coverage.json` に分離) / 型カタログへの参照 field (SoT Chain 逆流、スキーマレベルで禁止、D1.5)
- **参照先**: 各要素は ADR と convention のみ参照可能。型カタログへの参照 field は存在しない
- **状態フィールドの廃止**: `status`, `approved_at` をスキーマから削除
- **トップレベルの content_hash 廃止**: spec 自体のトップレベル hash は不要 (参照元側で持つ必要があれば `SpecRef.hash` で管理、D2.1)
- **Phase 3 で書き戻さない**: coverage annotation は `task-coverage.json` が担うので、Phase 3 の作業として spec.json を書き戻す必要がない
- **下流からの修正は許容**: Phase 2 / 3 / 実装で問題が連鎖し spec 側の不備が判明した場合、spec を修正するのは正常な探索的精緻化 (D0.1)。「Phase 1 で書いたら凍結」を強制する仕組みは作らない (§3 原則 7)

#### D1.3: 型カタログ — 型の SSoT

- **配置**: `track/items/<id>/<layer>-types.json` (track 内、多層対応)
- **含めるもの**: すべての型のカタログエントリ (kind, expected_methods, expected_variants, transitions_to, implements, action, **spec_refs**, **informal_grounds**)
- **`spec_refs[]` の参照先**: spec.json 内の特定要素。各要素は `SpecRef { file, anchor: SpecElementId, hash: ContentHash }` (D2.1)。SoT Chain ② を表現
- **`informal_grounds[]`**: designer が kind 選択 / method 設計時に引いた discussion / feedback / memory / user directive 由来の未永続化根拠。非空 → 型契約 → 仕様書 signal が 🟡 (D3.2)。merge 前に formal ref (SpecRef) または ADR / convention に昇格要
- **型契約 → 仕様書 signal のみ暫定的に advisory 扱い**: このシグナルは未実装 (README ロードマップ「計画中」)。本 ADR では catalogue `spec_refs[]` の schema のみ追加し、signal 評価は後続 ADR で実装する (D3.2)。他の決定項目 (schema / phase 分離 / gate など) は本 ADR で確定
- **権威**: 型に関する全ての情報はここが唯一の真実
- **スキーマ拡張**: derive / generic bounds / visibility などを表現できない現スキーマの拡張は別 ADR (Q8)

#### D1.4: 現 metadata.json の責務分離 — identity と Phase 3 成果物を分ける

現行の `metadata.json` は (i) track identity と (ii) 実装計画 (tasks + plan.sections) を同居させており、(ii) が Phase 3 でしか確定しないため optional field 問題を生んでいた。本 ADR では以下に責務分割する:

##### metadata.json (track identity 専用、schema_version 5)

- **配置**: `track/items/<id>/metadata.json`
- **含めるもの**: `schema_version, id, branch, title, status_override (optional), created_at, updated_at`
- **含めないもの**:
  - `tasks[]` / `plan.sections` / その他の Phase 3 成果物
  - `status` フィールド — track の現在 status は `impl-plan.json` と `status_override` から on-demand で **派生** する (後述「派生ステータス」参照)。`impl-plan.json` への書き込みと `metadata.json` への status 同期という 2 段書きは原子性を保証できないため、status を identity から除外して単一ファイルへの書き込みで完結させる
- **作成タイミング**: /track:plan のフェーズ 0 (初期化、main が書く)
- **書き換え**: `branch`, `title`, `status_override`, `updated_at` のみ更新。それ以外は不変
- **後方互換**: `codec::decode` は schema_version 4 (status フィールドを持つ旧形式) を拒否する。ただし `verify-track-metadata-local` ゲートは v2/v3/v4 の legacy track を遡及適用なしで skip する (D6.1)。

##### 派生ステータス (derive_track_status)

track の status は `metadata.json` に保存された値ではなく、`impl-plan.json` の task 状態と `status_override` から `domain::derive_track_status(impl_plan, status_override)` で on-demand に算出する。算出規則:

| 条件 | 派生 status |
|---|---|
| `status_override` が Some | `status_override.track_status()` を status とする |
| `status_override` が None かつ `impl-plan.json` が存在しない | `Planned` |
| `impl-plan.json` の全 task が resolved (done / skipped) | `Done` |
| task が in_progress 状態、または resolved と unresolved が混在 | `InProgress` |
| task がすべて todo (resolved なし) | `Planned` |

`status_override` は「ユーザーが明示的に track を blocked / cancelled と判断した」という identity 注釈であり、ブランチの有無や impl-plan の状態とは独立する。そのため `status_override` のみ `metadata.json` に残す。

##### activation 不変条件

track の activation 判定は `branch` フィールドの実体化のみで決定する:

- `is_activated() ≡ branch.is_some()` — ブランチが materialized されているかどうかが活性化の唯一の基準
- `status_override` の値、派生 status の値、および `schema_version` は activation predicate の判定基準に含めない
- 根拠: branchless planning track が `status_override = blocked / cancelled` を持つケースを activation と誤判定しないため。activation の判定軸を `branch` に一本化することで、override 値の意味論的な曖昧さを排除する

##### impl-plan.json (Phase 3 で生成、新設)

- **配置**: `track/items/<id>/impl-plan.json`
- **含めるもの**: `schema_version, tasks: [{id, description, status, commit_hash}], plan: {summary, sections}`
- **用途**: 実装の進行マーカー (tasks は中断耐性)、plan.sections は section 分割ビュー
- **作成タイミング**: Phase 3 (planner が書く)
- **SoT Chain の外**: 計画用成果物、SoT 階層には属さない
- **ref field を持たない**: task は進行マーカーで SoT Chain の外。`adr_refs[]` / `convention_refs[]` / `spec_refs[]` のいずれも task 要素には設けない。必要な言及 (型カタログ / spec / convention 等、対象は問わない) は description の自然言語で記述する

##### task-coverage.json (Phase 3 で生成、新設)

- **配置**: `track/items/<id>/task-coverage.json`
- **含めるもの**: spec.json の 4 セクション (`in_scope`, `out_of_scope`, `constraints`, `acceptance_criteria`) の要素ごとに task_refs を記録する構造 (現行 spec.json の各 requirement の `task_refs` フィールドをそのまま外出し)
- **用途**: spec の要素と task の紐付けを保持。現行 `spec.acceptance_criteria[].task_refs[]` 等の機能をファイル分離で引き継ぐ
- **作成タイミング**: Phase 3 (planner が書く、impl-plan.json と同時)
- **CI 検証 (現行の `spec_coverage::verify` と同等)**:
  - **coverage 強制**: spec の `in_scope` と `acceptance_criteria` の各要素に有効な task_ref が紐付いているか
  - **referential integrity**: 4 セクション (in_scope / out_of_scope / constraints / acceptance_criteria) の requirement が持つ task_refs (optional、空なら検証対象外) について、**記述があれば** impl-plan.json の tasks[] 内 task_id を指すこと
- **前提**: 現行 spec.json の requirement 要素は `{text, sources, task_refs?}` で `text` が暗黙 ID として扱われているが、task-coverage.json から参照するには spec 要素に明示的 `id` が必要。Q13 [a] で確定 (本 ADR で `SpecElementId` を必須化、D2.1 / D1.2)

##### Phase 3 が新規ファイルのみを書く効果

- 「Phase 1 で書いた spec を Phase 3 で書き戻す」緊張が消える (writer 境界が明確: planner は Phase 1 で spec を、Phase 3 で impl-plan + task-coverage を書くが、それぞれ別ファイル)
- 通常実行で Phase 3 が spec.json / metadata.json を触らないので、型カタログの `spec_refs[].hash` (D2.1) が coverage annotation 追記で stale になる事態を回避できる
- ただし下流で問題が検出されて spec 側の修正が必要になる場合は、正常な探索的精緻化として spec.json を修正する。これは Phase 3 の「書き戻し」ではなく、back-and-forth による上流修正 (D0.1)

#### D1.5: 層をまたぐ参照の制約 (SoT Chain 方向準拠)

README の SoT Chain (ADR ← 仕様書 ← 型契約 ← 実装) を参照方向の根拠とする。SoT Chain 上では **隣接層のみ参照可** (§3 原則 1):

| 参照元 | 参照先 | 可否 | 根拠 |
|---|---|---|---|
| 仕様書 (spec.json) | ADR | ✅ 許可 | SoT Chain ① (隣接層) |
| 仕様書 | convention | ✅ 許可 | 全層共通の補助参照 (track 横断で安定) |
| 仕様書 | 型カタログ | ❌ 禁止 | SoT Chain の逆流 |
| 型カタログ | 仕様書 | ✅ 許可 | SoT Chain ② (隣接層)。本 ADR では `spec_refs[]` field で実現 (D1.3 / D2.2) |
| 型カタログ | ADR | ❌ 禁止 | 層を飛ばした直接参照。ADR 関連の問題は spec を経由して逐次伝播する (D0.1) |
| 型カタログ | convention | ✅ 許可 (schema は本 ADR スコープ外) | 全層共通の補助参照。ただし型カタログ向け `convention_refs[]` field は本 ADR では導入しない。本 ADR の型カタログスキーマには `spec_refs[]` のみ追加し、convention 参照 field は Q8 (型カタログスキーマ v2) 以降で別 ADR が扱う |
| ADR | track 内成果物 (spec / 型カタログ / metadata / research/) | ❌ 禁止 | ADR の track 横断性を保つため |
| ADR | 他 ADR / convention | ✅ 許可 | track 横断内で閉じている |
| 新規 track | 他 track の track 内成果物 | ❌ 禁止 | 他 track に踏み込むことになる。再利用したいものは ADR や convention に昇格させる |

**ADR で型例を示したいとき**: ADR 本文だけで完結させる + 例示マーカー付与 + 型カタログへのリンクは書かない。

**convention が全層共通の補助参照である理由**: convention は track 横断で全層に共通する補助情報 (coding style、workflow ルール等)。SoT Chain の ↑ 一方向依存とは別枠で、どの層からも参照してよい。

**実装層が上表に含まれない理由**: 実装 (Rust コード) は構造化参照 field (`adr_refs[]` 等) を埋め込まない。SoT Chain ③ (実装 → 型カタログ) は **rustdoc 抽出と catalogue 宣言の CI 突合** で評価する (README §参照チェーンの評価)。実装 → 仕様書 / ADR / convention も参照埋め込みメカニズムが無いため、本 table の「禁止 / 許可」判定対象ではない。実装の整合性は rustdoc 突合 signal によってのみ評価される。

#### D1.6: research note を track 内と track 横断で分ける

| 種類 | 新しい配置 | 層 |
|---|---|---|
| track 内の planner 出力 | `track/items/<id>/research/<timestamp>-<capability>-*.md` (サブディレクトリ) | track 着手前 |
| track 横断の分析 | `knowledge/research/` に残す | track 横断で安定 |

- 新規 track のみ新配置を適用、既存は遡及しない
- `knowledge/research/` というパス名は維持 (リネームの移行コストを避ける)

### D2: 強制機構

#### D2.1: 構造化参照スキーマ (4 独立構造体方式、値オブジェクトでロジック集約)

現行の文字列列挙を、参照先の層ごとに **4 つの独立した構造体** (file ベース 3 種 + 未永続化根拠 1 種) に置き換える。role 文字列タグによる discriminated union は採用しない (無闇な共通化は混沌を招く)。各使用サイトで専用 field を持ち、型で参照先の層が決まる。**各 anchor / hash / summary は値オブジェクト (newtype) で定義する**: String で扱うと比較・解決・validation ロジックが使用サイトに散在し、Q15 の semantic 厳密化時に修正箇所が広範囲に散るため。

**値オブジェクト**:

<!-- illustrative, non-canonical -->
```rust
struct SpecElementId(String);          // spec 要素の id (Q13 [a] 確定で付与)
struct AdrAnchor(String);              // loose string (Q15 で内部実装厳密化)
struct ConventionAnchor(String);       // 同上
struct ContentHash([u8; 32]);          // SHA-256
enum InformalGroundKind {              // 有限集合 → enum-first 原則
    Discussion, Feedback, Memory, UserDirective,
}
struct InformalGroundSummary(String);  // 非空一行要約
```

**4 つの ref 構造体** (独立構造体、共通 trait / enum 抽象化なし):

<!-- illustrative, non-canonical -->
```rust
struct AdrRef            { file: PathBuf, anchor: AdrAnchor }
struct ConventionRef     { file: PathBuf, anchor: ConventionAnchor }
struct SpecRef           { file: PathBuf, anchor: SpecElementId, hash: ContentHash }
struct InformalGroundRef { kind: InformalGroundKind, summary: InformalGroundSummary }
```

**配置**: `libs/domain/src/plan_ref/` (新モジュール、ref 種別ごとに 1 ファイル):

```
libs/domain/src/plan_ref/
├── mod.rs                    (再 export + module-level doc)
├── adr_ref.rs                AdrAnchor + AdrRef
├── convention_ref.rs         ConventionAnchor + ConventionRef
├── spec_ref.rs               SpecElementId + ContentHash + SpecRef
└── informal_ground_ref.rs    InformalGroundKind + InformalGroundSummary + InformalGroundRef
```

既存 `libs/domain/src/ids.rs` (entity identity 専用: `TrackId` / `TaskId` / `CommitHash` 等) は概念クラスタが異なるため分離を維持。`NonEmptyString` の流用はしない (`InformalGroundSummary` は専用 newtype で認知を区別)。

**本 ADR で決定する実装範囲**:
- 現行の `sources[]` 単一 field を廃止し、使用サイトごとに専用 field を持たせる (詳細は D1.2 / D1.3 / D2.2)
- file ベースの 3 ref (`AdrRef` / `ConventionRef` / `SpecRef`) に `anchor` field を required で導入 (reviewer が参照先内の位置を即座に判別できるため、現行 string 参照からのデグレを防ぐ)
- `SpecRef` には `hash` も required で導入。spec.json は構造化 (JSON) で anchor が示す要素 (id で特定される subtree) の境界が明確なので、hash 対象が曖昧にならず本 ADR で確定できる。ただし `SpecRef.anchor` / `SpecRef.hash` の**drift 検証** (anchor 解決 + hash 照合) は本 ADR の `sotp verify plan-artifact-refs` 対象外。後続 ADR `2026-04-23-0344` §D1.2 の `sotp verify catalogue-spec-refs` が担当する
- `AdrAnchor` / `ConventionAnchor` の validation は newtype コンストラクタ内の loose チェック (非空文字列のみ) に留める。semantic 厳密化 (heading slug / HTML marker 等) は Q15
- `AdrRef.hash` / `ConventionRef.hash` の追加は Q15 (markdown ベースで hash 対象範囲が曖昧なため、意味論ごと別 ADR で決定)
- **`InformalGroundRef` を導入して未永続化根拠を schema レベルで扱う**: 議論 / feedback / memory / user directive 由来の根拠を、file として永続化する前の段階でも構造化された形で citing 可能にする。signal 評価では「根拠は存在するが未永続化」という中間状態を 🟡 として表現し、「merge 前に formal ref (ADR / convention / research note 等) へ昇格せよ」の運用を維持する (D3.1 / D3.2)
- D1.5 で定めた参照方向制約は「どの field にどの ref 構造体が入るか」が型で決まっているため、`sotp verify plan-artifact-refs` の CI 検証は field ごとに ref 構造体の妥当性 (file 存在 / `AdrAnchor` / `ConventionAnchor` の loose validation = 非空文字列のみ / summary 非空) を検査するだけで済む (role dispatch 不要)。`AdrRef`/`ConventionRef` の semantic anchor resolution は Q15 の後続 ADR が担当。catalogue `spec_refs[]` の `SpecRef.anchor` / `SpecRef.hash` 照合は `sotp verify catalogue-spec-refs` が担う (本 ADR 対象外)

**値オブジェクトの責務境界**:

| 型 | 本 ADR でのロジック | Q15 以降 |
|---|---|---|
| `SpecElementId` | 非空文字列、ID 命名規則 (例: `IN-\d+`、`AC-\d+` 等) | 変更なし |
| `AdrAnchor` | 非空文字列のみ (loose) | heading slug / HTML marker 等の具体形式、resolution ロジック |
| `ConventionAnchor` | 同上 | 同上 |
| `ContentHash` | SHA-256 形式 (32 バイト) | 変更なし (AdrRef / ConventionRef にも再利用) |
| `InformalGroundKind` | 4 variant enum (Discussion / Feedback / Memory / UserDirective) | 変更なし (variant 追加は必要に応じて別 ADR) |
| `InformalGroundSummary` | 非空文字列 (一行要約) | 変更なし |

各値オブジェクトのコンストラクタに validation を閉じ込めることで、Q15 で semantic が厳密化されても使用サイト (CI verify / signal 評価 / schema serializer 等) に影響しない。

**SpecRef.hash の対象**: spec 要素 (id で特定される JSON subtree) の canonical serialization に対する SHA-256。serialization 規則 (key 順序、whitespace 等) は実装時に別途定める (T2 等で)。

**Q13 確定への昇格**: spec 要素への明示的 `id` 付与 (Q13 [a]) は `SpecElementId` の基盤となるため、本 ADR で **確定事項** に昇格する (これまでの「第一候補」状態からの確定)。

**本 ADR のスコープ外 (Q15 で別 ADR に委譲)**:
- `AdrAnchor` / `ConventionAnchor` の semantic 厳密化 (newtype 内部の validation 強化)
- `AdrRef` / `ConventionRef` への `hash` field 追加 (markdown ベースで hash 対象範囲が曖昧なため、意味論ごと Q15 で決定)
- ADR / convention の hash の対象 (ファイル全体か / anchored section か)
- markdown 系 anchor の resolution ロジック
- catalogue `SpecRef.hash` による drift 検出 CI gate の実装 (後続 ADR `2026-04-23-0344` §D1.2 の `sotp verify catalogue-spec-refs` が担当、D2.3 の骨格は spec.json / task-coverage.json ref 専用)

**Rust 実装の負債回避**: 4 構造体に分割し各 anchor / hash / summary を newtype で定義しておけば、Q15 で semantic が厳密化されるときも newtype の内部実装を変えるだけで使用サイトに影響しない。optional / nullable 混在や discriminated union 経由の分岐がないため `Option<String>` 債務は発生しない (後方互換は考慮せず、`feedback_no_backward_compat` と整合)。

**Q15 への設計方針ガイド (Notes)**:
- Q15 が追加する: `AdrAnchor` / `ConventionAnchor` の semantic 厳密化、`AdrRef.hash` / `ConventionRef.hash` の追加 (`ContentHash` newtype を再利用)
- convention の hash は「ファイル全体の SHA-256」が有力候補 (更新頻度が低く section 境界を気にしなくてよい)
- 構造体と値オブジェクトは引き続き独立 (enum / trait 共通化は行わない)
- `InformalGroundRef` / `InformalGroundKind` / `InformalGroundSummary` は Q15 の対象外 (file ベースでないため anchor / hash 概念が適用されない)

#### D2.2: ref 構造体一覧 (SoT Chain 準拠)

| ref 構造体 | 使用サイト (field) | 参照先の層 | field 内容 |
|---|---|---|---|
| `AdrRef` | spec.json の要素 `adr_refs[]` | ADR (track 横断) | `file` + `anchor: AdrAnchor` (loose、Q15 で semantic 厳密化) |
| `ConventionRef` | spec.json の要素 `convention_refs[]` / spec.json top-level `related_conventions[]` | convention (track 横断) | `file` + `anchor: ConventionAnchor` (loose、Q15 で semantic 厳密化) |
| `SpecRef` | カタログエントリの `spec_refs[]` | spec.json (同 track 内) | `file` + `anchor: SpecElementId` (Q13 [a] 確定) + `hash: ContentHash` (anchor が指す subtree の SHA-256) |
| `InformalGroundRef` | spec.json の要素 `informal_grounds[]` / カタログエントリの `informal_grounds[]` | (永続化されていない、file 対象なし) | `kind: InformalGroundKind` (Discussion / Feedback / Memory / UserDirective) + `summary: InformalGroundSummary` (非空一行要約) |

各 field は専用の ref 構造体のみを保持する (homogeneous)。role 文字列タグは存在しない (field 名で参照先の層が決まる)。**`InformalGroundRef` は file 対象を持たない** ため、存在自体が「未永続化根拠あり」を意味し、signal 評価で 🟡 を発火する (D3.1 / D3.2)。formal ref (`AdrRef` / `ConventionRef` / `SpecRef`) への昇格が merge の前提となる。禁止される参照パターン (SoT Chain 逆流、ADR → track 内、他 track 侵入など) は D1.5 に一覧。CI ゲートは field ごとに ref 構造体の妥当性を検査するだけで済み、role dispatch は不要。

#### D2.3: `sotp verify plan-artifact-refs` CI ゲート (骨格のみ)

新規 CLI サブコマンドの役割:
- 対象成果物: **spec.json** および **task-coverage.json** の ref field。型カタログ (`<layer>-types.json`) の `spec_refs[]` は **対象外** — 後続 ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.2 で新設された専用ゲート `sotp verify catalogue-spec-refs` が担う (責務分離原則)
- 各成果物の ref field (`adr_refs[]` / `convention_refs[]` / `related_conventions[]` / `informal_grounds[]`) を走査:
  - 各 field に入っている ref 構造体が期待どおりの型か (schema validator で強制)
  - 参照先ファイルが実在するか (file ベース ref のみ、存在チェック)
  - `AdrRef.anchor` (`AdrAnchor`) / `ConventionRef.anchor` (`ConventionAnchor`): 存在チェック + newtype コンストラクタの loose validation (非空文字列) のみ (本 ADR 範囲)。semantic 厳密化と resolution ロジックは Q15
  - `AdrRef.hash` / `ConventionRef.hash`: field 自体が本 ADR に存在しない (Q15 で追加)
  - `InformalGroundRef.kind` / `InformalGroundRef.summary` (`InformalGroundSummary`): newtype コンストラクタの validation (summary 非空、kind 有効 variant) のみ。file resolution の対象外 (未永続化根拠のため)
- **task-coverage.json の突合検証**: D1.4 で定めた 2 つの検査 (coverage 強制 + referential integrity) を担う (D3.3)
- **本文内の canonical ブロック疑惑の検出**: ADR (例示マーカーは除外) や spec フィールド内の 10 行超のコードブロックを警告 (実装詳細は Q14)
- 失敗時は CI fail

本 ADR では CLI コマンドの **存在と役割範囲**、`AdrAnchor` / `ConventionAnchor` の loose validation、および `InformalGroundRef` の newtype validation を決定。`SpecRef.anchor` / `SpecRef.hash` の解決ロジックは catalogue `spec_refs[]` に属し、後続 ADR `2026-04-23-0344` §D1.2 が導入する `sotp verify catalogue-spec-refs` ゲートが担当する (本 ADR の対象外)。`AdrAnchor` / `ConventionAnchor` の semantic 厳密化・resolution ロジック、および `AdrRef.hash` / `ConventionRef.hash` の追加は Q15 の後続 ADR に委ねる。

### D3: フェーズごとのゲート (approved 廃止)

`approved` / `Status` のような人工的な状態フィールドを廃止し、**各フェーズのゲートは SoT Chain 信号機評価 (🔵🟡🔴) または binary 検証 (OK / ERROR) の自然な判定で決まる** ことにする (§3 原則 4 のゲート方式分離に従う)。

#### D3.1: フェーズ 1 のゲート — 仕様書 → ADR シグナル

- 成立条件: spec.json の各要素の `adr_refs[]` 評価で 🔴 がゼロ、かつ `informal_grounds[]` の扱いが track 完了時に解消されていること (🟡 は commit 可・merge 不可、track 完了時にすべて 🔵 になること)
- **評価の着目点** (field ごとに独立評価、結果の合成で総合 signal を決定):
  - `adr_refs[]`: 🔵 (全 AdrRef が formal resolution 成功) / 🔴 (1 件でも file 存在・anchor validation に失敗)
  - `informal_grounds[]`: 空 → 🔵、非空 → 🟡 (未永続化根拠あり、merge 前に formal ref へ昇格要)
  - `convention_refs[]`: signal 評価対象外 (field 単位で自然に分離されるため除外ロジック不要)
- 既存の spec-signals ツールを使う (参照品質の評価)
- **失敗時の挙動**: `/track:plan` orchestrator が D0.1 逐次伝播に従って ADR 自動修正 (adr-editor 再 invoke) に escalate。ADR ファイルに commit 履歴があれば auto-edit、無ければ user pause (D0.1 参照)。再試行回数は `max_retry` (default 5、「コマンド境界」セクション参照) で制御、閾値超過で user pause

#### D3.2: フェーズ 2 のゲート — 型契約 → 仕様書 シグナル (段階的導入)

signal 評価 (型契約 → 仕様書) は README ロードマップで「計画中」であり、本 ADR の時点では未実装。本 ADR は **schema 先行** + **signal 実装は別 ADR** という段階的アプローチで扱う:

**暫定期 (signal 未実装、本 ADR 単体での状態)**:
- 成立条件: 型カタログの各エントリに **`spec_refs[]` フィールドが存在すること** (file/schema 存在チェックのみ)
- `spec_refs[]` の内容は **advisory** (書かれているが signal 評価は未実装)
- `sotp verify plan-artifact-refs` (D2.3 本 ADR 範囲) は spec.json / task-coverage.json の ref field を対象とする。型カタログの `SpecRef.anchor` / `SpecRef.hash` の drift 検証は `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.2 で新設された **`sotp verify catalogue-spec-refs`** が担う (本 ADR に基づく `plan-artifact-refs` の対象外)
- designer プロンプトで正確な `spec_refs[]` 記述を要請する

**signal 実装後 (`knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` にて確定)**:

catalogue-spec signal の設計は `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1 で確定した。
以下は同 ADR の決定内容の要約:

- **signal 色 (🔵🟡🔴) は grounding 品質のみを反映** (Phase 1 `evaluate_requirement_signal` と完全対称な informal-priority rule):
  - `informal_grounds[]` 非空 → 🟡 Yellow (informal-priority、他状態に優先)
  - `informal_grounds[]` 空 + `spec_refs[]` 非空 → 🔵 Blue
  - `informal_grounds[]` 空 + `spec_refs[]` 空 → 🔴 Red
- **`SpecRef.anchor` 解決 + `SpecRef.hash` 一致は signal 色に影響しない** — これらは信号機とは独立の binary gate (`sotp verify catalogue-spec-refs`) として分離。hash gate ERROR は全信号色で fail-closed (D1.3)
- **評価の着目点** (signal 色の決定要素、field ごとに独立評価):
  - `spec_refs[]`: 非空か空かを評価 (hash 一致 / anchor 解決の成否は信号色に影響しない)
  - `informal_grounds[]`: 空 → 🔵 / 非空 → 🟡
  - (型カタログには convention 参照 field が存在しないため除外ロジック不要)
- **merge 成立条件**: 🔴 がゼロ + hash gate OK + `informal_grounds[]` が track 完了時に解消 (🟡 は commit 可・merge 不可)
- signal 実装以降の新規 track に自動適用 (既存 track への遡及なし)

#### D3.3: フェーズ 3 のゲート — task-coverage の binary pass/fail

フェーズ 3 は 2 つのファイルを生成するが、ゲートは task-coverage.json にのみ設ける:

- **impl-plan.json (tasks 本体)**: 進行マーカーであり SoT Chain の外。スキーマ検証のみでゲート化しない
- **task-coverage.json**: **binary pass/fail** ゲート (現行 `libs/infrastructure/src/verify/spec_coverage.rs` の `spec_coverage::verify` を踏襲):
  - **pass 条件**: D1.4 で定めた 2 つの検査 (coverage 強制 + referential integrity) の両方が成立すること
  - **fail (ERROR)**: いずれか失敗で commit 不可。中間状態は設けない
  - **skip**: task-coverage.json が存在しない Phase 1/2 状態では gate 自体が発火しない (D6.1 の「file があれば検証、なければ skip」)
- **CI 検証**: 現行の `spec_coverage::verify` ロジックを `sotp verify plan-artifact-refs` に統合 (D2.3)
- **既存 README との整合**: 本ゲートは spec と task_refs の関係を評価するもので、SoT Chain ① / ② / ③ とは独立した coverage 評価

#### D3.4: 実装のゲート — 実装 → 型契約 シグナル

- 既存の rustdoc 突合シグナル (ロードマップに記載)
- フェーズ 4 以降の既存フロー、本 ADR の改訂対象外

#### D3.5: 事前承認を残す例外

以下はフェーズゲートとは別に、ユーザーの事前承認を取る:
- `git push` / `git commit` (既にガード済み)
- 外部 API 呼び出し (PR や issue の作成)
- 破壊的なファイルシステム操作
- 環境破壊 (CI 設定や lockfile の強制書き換え)

成果物の生成はここに該当しないため、すべて事後レビュー方式で扱う。

### D4: フェーズごとの作成担当

各フェーズの成果物は、そのフェーズに責任を持つ capability が直接書く。**各ファイルは 1 つのフェーズのみで生成する** (別フェーズの通常作業として同じファイルを書き戻すことを禁じる。同じ capability が別フェーズを担当すること自体は許容 — planner は Phase 1 で spec.json を、Phase 3 で impl-plan.json / task-coverage.json を書く):

| タイミング | 独立コマンド | 作成者 | 書く成果物 | 書かない成果物 |
|---|---|---|---|---|
| track 前段階 (初期作成) | (なし) | ユーザー + main の対話 | ADR | track 内成果物すべて |
| track 前段階 (back-and-forth 修正) | (`/track:plan` が自動 invoke) | adr-editor サブエージェント | ADR (下流 signal 🔴 起点の修正、ADR に commit 履歴がある場合のみ) | track 内成果物すべて |
| フェーズ 0 | `/track:init` | main | metadata.json (identity) | その他の track 内成果物 |
| フェーズ 1 | `/track:spec` | planner サブエージェント | spec.json | ADR, 型カタログ, impl-plan, task-coverage |
| フェーズ 2 | `/track:design` | designer サブエージェント | `<layer>-types.json` | ADR, spec, metadata, impl-plan, task-coverage |
| フェーズ 3 | `/track:impl-plan` | planner サブエージェント | impl-plan.json + task-coverage.json | ADR, spec, 型カタログ, metadata |

**各ファイルは 1 人の writer が 1 つの phase で生成する。別 phase の通常作業として書き戻さない**。これにより「別 writer / 別 phase の追記による責務混在」が構造的に生じない。ただし下流で問題が検出されて上流 (spec など) の修正が必要になった場合は、探索的精緻化として back-and-forth で修正する (D0.1)。**その際は該当成果物の元 writer (ADR の場合は adr-editor) を再 invoke する** (main orchestrator による直接 Edit は禁止、D0.1 参照)。ADR の auto-edit は commit 履歴がある場合のみ、無い場合は user pause (D0.1 参照)。

作成者の境界は以下の 2 層で明示する:

- **Capability → Provider routing**: `.harness/config/agent-profiles.json` が SSoT。capability (planner / designer / main 等) と実装 provider (Claude Code / Codex CLI 等) と model のマッピングを管理
- **Scope Ownership の宣言**: `.claude/agents/*.md` の subagent markdown が SSoT。各 capability が書いてよい / よくない artifact を定義 (例: planner.md に「Do not modify 型カタログ」等)

違反検出は現状 **subagent プロンプト + 人手レビュー**。config-driven な writer boundary の自動強制は Q16 で追跡。

### D5: ワークフローの形式的手順を最小化する原則 (convention 昇格)

`knowledge/conventions/workflow-ceremony-minimization.md` として確立する:

> **形式的手順を減らす**: 成果物の生成はフェーズごとのゲート (SoT Chain 信号機評価 または binary 検証) + 事後レビューを基本とする。ユーザーの事前承認はやり直しコストが高いか不可逆な action に限る (D3.5)。要約ではなく実成果物をユーザーに見せることで、レビューの情報量と速度を両立する。人工的な `approved` 状態は作らない (形骸化するため)。

### D6: コミットゲートを「file 存在 = phase 状態」方式に再定義

D1.4 で metadata.json を identity 専用に分離し、Phase 3 成果物を独立ファイル (impl-plan.json / task-coverage.json) に切り出したので、CI ゲートも「field の存在有無を条件分岐で扱う」ではなく「**該当ファイルがあればそのファイルを検証、なければ skip**」というシンプルな形になる。

#### D6.1: 各 verify-* ゲートを「file 単位」に再整理

| ゲート | 対象 | 発火条件 |
|---|---|---|
| `verify-track-metadata-local` | metadata.json の存在と identity field の schema 妥当性 (`schema_version, id, branch, title, status_override, created_at, updated_at`。`status` フィールドは identity に含まれないため検証対象外) | 常時 (track directory があれば要求)。ただし schema_version < 5 の legacy track (v2/v3/v4) は遡及適用なしで skip — 移行ポリシー参照 |
| (新) spec.json 検証 | spec.json schema 妥当性 + 各 ref field (`adr_refs[]` / `convention_refs[]` / `related_conventions[]`) の参照先整合 (ref 解決の実装は `sotp verify plan-artifact-refs`、D2.3) | spec.json が存在するとき |
| `verify-latest-track-local` (改訂) | plan.md + impl-plan.json の整合。plan.md の task 項目は impl-plan.json が存在するときのみ要求 | plan.md が render されているとき |
| (新) task-coverage.json 検証 | D1.4 で定めた 2 つの検査 (coverage 強制 + referential integrity) の両方が成立すること (現行 `spec_coverage::verify` 踏襲、実装は `sotp verify plan-artifact-refs`、D2.3) | task-coverage.json が存在するとき |

Phase 1/2 の時点では impl-plan.json / task-coverage.json が存在しないので、それらの検証は自然に skip される。条件付き field チェックのロジックが不要。

#### D6.2: commit 時ゲートと merge 時ゲートの分離

- **commit**: 以下のいずれかで block:
  - binary ERROR: schema 違反、SoT Chain 逆流、task-coverage.json が存在するのに未カバー
  - SoT Chain 信号機評価が 🔴 (仕様書 → ADR 等)
- **commit の skip 条件**: impl-plan.json / task-coverage.json が **存在しない** 状態 (Phase 1/2) は当該 binary gate が発火せず、他の gate のみ評価される
- **merge (最終 PR) の要求**:
  - binary: impl-plan.json と task-coverage.json の存在を必須化。task-coverage.json が D1.4 で定めた coverage 強制 (spec の全 in_scope と acceptance_criteria を網羅) + referential integrity の両方を満たすこと (binary pass)
  - **binary: impl-plan.json の全 task が resolved 状態** (`done` / `skipped` のいずれか) であること。`todo` / `in_progress` が 1 つでも残っていれば BLOCKED。空の tasks も BLOCKED (nullable bypass させない)。`plan/` ブランチは skip。現行 `usecase::task_completion::check_tasks_resolved_from_git_ref` (`knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D9 / §D9.1) を impl-plan.json 読み替えで踏襲 (旧 `DonePending` / `DoneTraced` / `Skipped` / `Todo` / `InProgress` 語彙は impl-plan.json schema の `done` / `skipped` / `todo` / `in_progress` に対応する)
  - SoT Chain 信号機評価: すべて 🔵 (🟡 残存は merge 不可)
  - **未永続化根拠の昇格**: spec 要素および型カタログエントリの `informal_grounds[]` が非空で SoT Chain signal が 🟡 になる場合 (D3.1 / D3.2)、merge 前に formal ref (`AdrRef` / `ConventionRef` / `SpecRef`) へ昇格して 🔵 にすること (未永続化根拠は ADR / convention / research note 等の file に書き起こしてから置き換える)

README §信号機評価 (🟡 = コミット可、マージ不可) の運用は仕様書 → ADR などの SoT Chain signal で機能する。task-coverage は binary で独立した扱い。

#### D6.3: phase ごとの commit 可否マトリクス

| 時点 | 存在するファイル | commit | merge |
|---|---|---|---|
| track 作成直後 | metadata.json | ✅ | ✗ |
| Phase 1 完了 | + spec.json | ✅ | ✗ (impl-plan.json なし) |
| Phase 2 完了 | + `<layer>-types.json` | ✅ | ✗ (同上) |
| Phase 3 途中 (未カバーの task-coverage.json あり) | + impl-plan.json + task-coverage.json (partial) | ERROR (block) | ERROR (block) |
| Phase 3 完了 | + impl-plan.json + task-coverage.json (全 in_scope + acceptance_criteria cover、referential integrity pass) | OK | OK (SoT Chain 信号機評価がすべて 🔵 かつ impl-plan.json の全 task が resolved なら、D6.2) |

**本 ADR で解消される現行方式の 2 つの問題**:
- **Phase 1/2 で placeholder タスクを挿入する必要がない**: 現行は spec.json の各 requirement が `task_refs` を持ち Phase 1 時点で空値を許さないため、暫定 task を挿入する運用が生じていた。本 ADR では `task_refs` を `task-coverage.json` に外出しし、Phase 1/2 では存在しないファイルの gate が skip されるので placeholder 不要
- **Phase 3 で spec.json の hash が動かない**: 現行は Phase 3 で spec.json に coverage annotation (`task_refs`) を追記するため spec の hash が変わっていた。本 ADR では Phase 3 で spec を触らず `task-coverage.json` を新規作成するだけなので、spec 側の hash は動かない

**`task-coverage.json` の binary 性**: `task-coverage.json` の gate は binary (OK / ERROR) のみで中間状態を持たない。ファイルが存在しつつ coverage が不完全な「途中」状態は全て ERROR として block される (D6.3 table 「Phase 3 途中」行)。半端にファイルだけ作って後で埋める運用は成立しない。

#### D6.4: 空の型カタログを受け付けるゲート修正

型カタログ (`<layer>-types.json`) のゲート (存在チェック / signal 評価 / 関連 `sotp track type-signals` など) は、**エントリが 0 件の空カタログも有効な状態として受け付ける** よう修正する。

**背景**: 既存型のみを使う track は新規型定義が不要で、空カタログまたは全 `action: reference` カタログが自然な状態。現行ゲートに「最低 1 件のエントリが必要」等の暗黙の下限がある場合、それを撤廃する。

**影響範囲**:
- `sotp track type-signals`: 空カタログでも pass
- `sotp track baseline-capture`: 空カタログを baseline として保存できる
- `verify-*-local`: 空カタログを理由とする block を撤廃

**実装ドリフト検出**: 文書化外の型変更 (catalogue に無い型が実装に追加) は SoT Chain ③ (実装 → 型契約) の reverse direction 評価で既に検出される (`libs/domain/src/tddd/consistency.rs` の `check_consistency` 内 Group 3/4 チェック、`undeclared_types` / `undeclared_traits` として実装済み)。空カタログを採用しても、実装に未宣言型が追加されれば Red として検出される。

---

## コマンド境界

### 現行と新方式の比較

```
[現行]
/track:plan       → ADR (任意) + spec + task + plan + verification を一括生成
/track:design     → 型カタログ
/track:implement  → 実装

[新] 各 phase 独立コマンド + orchestrator:
(track 前段階)    → ADR 作成 (対話または手動、/track:plan の外で先に)
/track:init       → フェーズ 0: metadata.json 生成 (main、identity のみ) (新設)
/track:spec       → フェーズ 1: spec.json 生成 (planner) (新設)
/track:design     → フェーズ 2: 型カタログ生成 (designer) (既存、責務を Phase 2 専任に)
/track:impl-plan  → フェーズ 3: impl-plan.json + task-coverage.json 生成 (planner) (新設)
/track:plan       → orchestrator: 上記 4 コマンドを順次 invoke + 結果集約 (既存コマンド、役割を再定義)
/track:implement  → フェーズ 4 (既存)
```

**各独立コマンドの役割**:
- 1 phase 1 capability 1 コマンド の明確な対応
- 単独で再呼び出し可能 (back-and-forth での元 writer 再 invoke に直結、D0.1)
- 事前確認 (上流 artifact の存在) は各コマンドが自己責任で行う

**`/track:plan` (orchestrator) の役割**:
- 事前確認: 参照予定 ADR が `knowledge/adr/` に存在するか (厳密モード、Rejected U)
- 4 コマンドを順次 invoke: `/track:init` → `/track:spec` → `/track:design` → `/track:impl-plan`
- 各 phase 完了後の gate (D3.1 / D3.2 / D3.3) を評価し、🔴 / ERROR のときは D0.1 の逐次伝播に従って元 writer を自動再 invoke
- 各 phase loop には最大再試行回数 (`max_retry`) を設定、閾値超過で user pause
- **ADR escalation 時の判定**: ADR ファイルに commit 履歴があれば adr-editor を auto-invoke (working tree のみ編集、loop 中はコミットしない)、無ければ user pause
- **終端処理**: /track:plan 完了時 (成功 / max_retry 超過のいずれも) に ADR working tree に HEAD からの diff があれば、user に diff を提示して判断を仰ぐ (accept / revert / 手動修正 / 中止)
- 最終成果物のパスと signal 状態をユーザーに報告

**`/track:plan` の引数**:
- 整数ひとつ (省略可)。最大ループ数 (`max_retry`) として使用
- 省略時は 5
- フラグ名は付けない (例: `/track:plan` / `/track:plan 3`)

### `/track:plan` 内部の流れ (orchestrator、D0.1 の逐次伝播を自動化)

```
/track:plan [max_retry]         (default 5)
  ├─ 事前確認: 参照予定の ADR が knowledge/adr/ に存在するか (未整備なら停止、Rejected U)
  ├─ /track:init を invoke      → metadata.json 生成
  │
  ├─ フェーズ 1 loop (retry ≤ max_retry):
  │   ├─ /track:spec を invoke  → spec.json 生成
  │   ├─ フェーズ 1 ゲート (D3.1): 仕様書 → ADR signal
  │   │   ├─ 🔵 → 次 phase へ
  │   │   ├─ 🟡 → 警告ログを残して次 phase へ (commit 可、merge 前解消)
  │   │   └─ 🔴 → ADR escalation (1 つ上 = ADR)
  │   │           → ADR に commit 履歴あり: adr-editor を再 invoke (working tree 編集のみ、コミットなし)
  │   │                                  → フェーズ 1 loop 先頭に戻る
  │   │           → ADR に commit 履歴なし: user pause (ADR を commit してから再開)
  │
  ├─ フェーズ 2 loop (retry ≤ max_retry):
  │   ├─ /track:design を invoke → 型カタログ生成
  │   ├─ フェーズ 2 ゲート (D3.2): 型契約 → 仕様書 signal (signal 未実装時は schema 存在のみ)
  │   │   ├─ 🔵 / 🟡 → 次 phase へ
  │   │   └─ 🔴 → /track:spec を自動再 invoke (1 つ上 = spec)
  │   │           → spec 修正後、フェーズ 1 ゲート再評価
  │   │             → 🔴 なら ADR 修正 loop に escalate (フェーズ 1 loop と同様)
  │   │             → 🔵 / 🟡 ならフェーズ 2 loop 先頭へ
  │
  ├─ フェーズ 3 loop (retry ≤ max_retry):
  │   ├─ /track:impl-plan を invoke → impl-plan.json + task-coverage.json 生成
  │   ├─ フェーズ 3 ゲート (D3.3): task-coverage binary
  │   │   ├─ OK → 完了
  │   │   └─ ERROR → /track:impl-plan を自動再 invoke (1 つ上への escalate ではなく同 phase で再生成)
  │
  ├─ 各 loop で max_retry 超過 → user pause + エスカレート
  ├─ 終端処理 (成功 / max_retry 超過いずれも):
  │   └─ ADR working tree diff を検査 (`git diff HEAD -- knowledge/adr/*.md`)
  │       ├─ diff あり → user に提示、判断を仰ぐ (accept / revert / 手動修正 / 中止)
  │       └─ diff なし → そのまま完了
  └─ 報告: 成果物のパスとゲート状態をユーザーに提示
```

事後レビュー方式: 全フェーズを自動通過させ、failure 時は逐次伝播 (D0.1) で元 writer を自動再 invoke し、閾値超過時のみユーザー介入。ADR 修正は loop 中に working tree のみ変更し、コミットは終端の user 判断後に委ねる。

---

## 却下した代替案

### A. canonical.md を新しい SSoT として導入する
**理由**: L0a (フェーズ順序の逆転) を放置する限り中間成果物は必ず要求される。対症療法に留まる。

### B. フェーズ順序は維持し、canonical block の重複だけ禁止する
**理由**: task を先に確定する限り、canonical block のような中間層が必要になる。

### C. ADR を廃止し、spec.json に決定も統合する
**理由**: ライフサイクルが異なる。ADR は track 横断、spec は track 内。

### D. 型カタログのスキーマを prompt やテンプレートまで扱えるように拡張する
**理由**: 型カタログの kind 体系は型分類に特化している。テキストまで詰め込むとスキーマ設計が破綻する。

### E. impl-plan.json の tasks に型の詳細を本文内で直接書く
**理由**: task は SoT Chain の外の進行マーカー。型の真実は型カタログだけに持たせる。

### F. 3 本の下位 ADR に分割する
**理由**: 相互依存が強く、単独で採否を決められない。repo の ADR 慣例 (1 ファイルで D1..Dn を並べる) とも不整合。

### G. `knowledge/research/` を雑居のまま維持する
**理由**: ライフサイクル 3 層区別が崩れる。D1.6 で track 内のものだけ移設する。

### H. `knowledge/research/` を `knowledge/analysis/` にリネームする
**理由**: 移行コストが過大。D1.6 でパス名は維持し、意味だけ絞る。

### I. track 内の research を単一ファイルにまとめる
**理由**: planner を再度呼んだときの版の履歴が git log にしか残らず見づらい。サブディレクトリで複数版を並べる方が可視性が高い。

### J. approved 状態を残したままシグナルも併用する
**理由**: approved が形骸化する (現状で起きている問題)。シグナルが既にゲートとして機能しており、approved は冗長。

### K. ADR を厳密に不変とする
**理由**: 実運用では ADR も書き換えられうる。凍結を強制すると不自然な形式的手順が生まれる。既存 ADR への追記も新 ADR 作成もどちらも許容する柔軟な運用にする。

### L. spec.json から型カタログへの参照を許可する
**理由**: SoT Chain の逆流。spec は型カタログの**上位**にあり、参照されるのはカタログ側。README §SoT Chain の方向性に反する。

### M. ADR を `/track:plan` のフェーズ 1 内で作成する
**理由**: ADR を `/track:plan` 内で作ると、(1) track のライフサイクルと ADR の track 横断性が混ざる、(2) spec と ADR を同じ作成者で書くと責務境界が曖昧になる。現行の慣例通り ADR は track 前段階に置くのが自然。

### N. `/track:plan` 起動時に ADR が未整備なら自動生成する
**理由**: 決定の権威が失われる。ユーザーが主導する track 前段階で作る。

### O. planner がすべての成果物を書く
**理由**: 型カタログは designer の専門 (kind 選択と TDDD シグナル連動)。兼任させると設計品質が劣化する。

### P. `track/items/<id>/templates/` サブディレクトリを導入する
**理由**: 使用実績がない。track 固有のテンプレートが必要になるケースが稀で、track 横断化 (`track/review-prompts/` 等) または spec フィールド内での吸収で事足りる。

### Q. impl-plan.json の tasks に構造化参照 field (`adr_refs[]` / `spec_refs[]` 等) を追加する
**理由**: task は進行マーカーで SoT Chain の外。構造化参照で記録する意味が薄い。現行スキーマ (`{id, description, status, commit_hash}`) で十分。

### R. `sotp verify plan-artifact-refs` で task から型カタログへの参照解決をチェックする
**理由**: task が型カタログを指すことをスキーマで強制していない。解決チェックの価値が不明確。実装 → 型カタログ (rustdoc 突合) が実質的なゲートとして機能する。

### S. Phase 3 で planner が spec.json に task_refs を書き戻す
**理由**: 「Phase 3 は新規ファイルのみを書く」 writer 境界が崩れ、coverage annotation の追記で spec.json の hash が通常フロー内で揺らぎ、下流 (型カタログ、実装) の signal が不必要に再評価される。`task-coverage.json` に分離すれば Phase 3 は純粋な新規ファイル生成になり、通常実行での spec 書き戻しが不要になる。なお、下流で問題が検出されて spec 修正が必要になる back-and-forth は別問題として許容される (D0.1)。

### T. metadata.json に tasks / plan.sections を optional field として残す (前 draft 案)
**理由**: optional field のチェックロジックが CI 側で必要になり、「field があれば検証」条件付き分岐が増える。file 分割 (impl-plan.json / task-coverage.json) にすれば「file があれば検証」の単純な形になり、writer 境界も明確。

### U. 緩和モード (ADR 未整備でも stub 自動生成で不完全な状態のまま進行を許す)
**理由**: 探索的開発と相性が良く、フロー中断を減らせる利点はある。しかし以下の懸念から本 ADR ではサポート対象外とする:
- ADR が後付けされることで「spec に合わせて decision を書く」rationalization を常態化させやすい
- 緩和モードが default 化すると SoT Chain 上流 (ADR) が先に固まる原則が形骸化する
- 一度緩和して書いた ADR を後日「厳密な decision」として書き直す運用規則の明文化が必要で、本 ADR のスコープを超える
本 ADR は厳密モードのみをサポートし、将来的に必要性が実証されたら別 ADR で緩和モード (stub ADR 段階区分、opt-in フラグ、merge 前 full ADR 強制など) を検討する。

### V. 未永続化根拠 (discussion / feedback / memory / user directive) を schema から締め出す (永続化強制)
**理由**: 議論段階のアイデアを file (ADR / convention / research note 等) に先行書き起こしてから引用する運用を強制することで、schema simplicity (file ベース 3 ref のみ) を最大化できる利点はある。しかし以下の懸念から採らない:
- WIP 中の track / spec 作成時、referring する discussion / feedback がまだ persist されていない状態で cite できず、作業フローが「先に document 化 → cite」の 2 段階に硬直する
- 既存運用の `memory — feedback_xxx` 参照パターンが折れる (移行コストが不釣り合いに大きい)
- 🟡 (commit 可・merge 前解消) の semantic を CI で保てなくなり、未永続化根拠が description 自由記述に散逸する
InformalGroundRef 導入 (D2.1) で structured ref として扱い、🟡 semantics を schema レベルで維持する方を採る。

### W. 未永続化根拠を description / text field の自由記述で吸収する (structured ref 化しない)
**理由**: schema 変更が不要で軽量ではあるが、以下の懸念から採らない:
- CI による「未永続化根拠あり → 🟡 → merge 前昇格要」の機構が働かず、reviewer の目視チェックに依存する (mechanism 不在)
- 自由記述は parse / extract の対象外で、「根拠が未永続化である」ことを track 完了時に機械的に検証できない
- description 内に ad hoc フォーマット (`[memory: xxx]`、`(see feedback XXX)` 等) が乱立する可能性があり、schema 正規化の目的 (L3 参照検証の欠如) と逆行する
InformalGroundRef (D2.1) で structured ref にすることで CI gate + 🟡 昇格要求を形式化する方を採る。

---

## 影響

### 利点

- **26 ラウンド問題の構造的緩和**: L0a / L0b を断ち、SoT Chain をスキーマ化することで、乖離の温床 (形骸化ゲート、参照の未検証、phase 順序逆転) を減らせる。ただし canonical block の重複 (L2) は本 ADR では扱わないため、別 ADR で別途対処が必要
- **SoT Chain の厳守**: README の原則通り下流→上流の一方向。CI で構造的に強制する
- **状態フィールド廃止による手続き削減**: approved の形骸化が根治する。さらに `metadata.json` から `status` を除外することで task 遷移が `impl-plan.json` への単一ファイル書き込みで完結し、2 段書きの非原子性 (impl-plan 書き込み後に metadata status 同期が失敗するパターン) が構造的に解消する
- **ADR のライフサイクル独立を保つ**: track 横断性を物理的に担保する
- **TDDD の名と実が一致する**: 型→task という自然な依存順序になる
- **metadata.json / impl-plan.json が軽量**: metadata.json は identity のみ、impl-plan.json は tasks と plan 限定で、各ファイルが単一責務
- **作成者の分業が明確**: フェーズで責務が一意に決まる
- **各ファイルが単一 writer / 単一 phase で確定**: metadata.json = フェーズ 0 の main、spec.json = フェーズ 1 の planner、型カタログ = フェーズ 2 の designer、impl-plan/task-coverage = フェーズ 3 の planner。どのファイルも別 phase で書き戻されないので hash 安定性が高い
- **Phase 1/2 での中間コミットが可能**: impl-plan.json / task-coverage.json が存在しない状態を「Phase 未到達」として自然に扱える。placeholder タスク挿入なしに Phase 1/2 状態でコミットできる
- **CI ゲートが単純化**: 「file があれば検証、なければ skip」という単純な分岐だけで済み、optional field の条件付きロジックが不要

### リスク・欠点

- **ワークフロー学習コスト**: track 前段階 + 3 フェーズを覚える必要。docs を刷新して対処する
- **実装コスト**: schema / CLI / SKILL / agent の改修、1–2 sprint 規模
- **既存 track には遡及しない**: 新旧 2 系統が一時的に並立する
- **型カタログスキーマの限界**: derive / generic bounds などは別 ADR で拡張する (Q8)
- **シグナル依存度が上がる**: approved の安全網がなくなるのでシグナル実装の品質が重要になる。`型契約 → 仕様書` シグナルは本 ADR の範囲外で、別 ADR (例: `tddd-ci-gate-and-signals-separation`) で実装される
- **advisory 期間の `spec_refs[]` 品質**: signal が未実装の間は catalogue の `spec_refs[]` 評価が検証されないため、semantic な正確性は designer のプロンプト品質に依存する。`SpecRef.anchor` / `hash` の drift 検証は後続 ADR `2026-04-23-0344` §D1.2 の `sotp verify catalogue-spec-refs` が担当 (本 ADR の `sotp verify plan-artifact-refs` 対象外)。signal 実装 + `catalogue-spec-refs` gate が揃うまで drift 防止が完結しない期間がある

### 移行

- 既存の完成済みおよび稼働中の track には遡及適用しない
- 次回の `/track:plan` 以降の invocation から新フローを適用
- 改訂対象:
  - SKILL / command / agent (ADR 生成ステップの削除、ADR 事前確認の追加、approved 廃止)
  - CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md: track 前段階 + 3 フェーズ構成を明記
  - spec.json スキーマ: `status`, `approved_at`, トップレベル `content_hash` を削除。各要素の `task_refs` も削除 (task-coverage.json に外出し)。現行の `sources[]` 単一 field を廃止し、各要素に `adr_refs: Vec<AdrRef>` / `convention_refs: Vec<ConventionRef>` / `informal_grounds: Vec<InformalGroundRef>` field を分割で持たせる (構造体は D2.1)。spec top-level の `related_conventions[]` も `Vec<ConventionRef>` に。各要素に明示的 `id: SpecElementId` を追加 (Q13 [a] 確定)
  - カタログエントリのスキーマ: `spec_refs: Vec<SpecRef>` + `informal_grounds: Vec<InformalGroundRef>` field を追加 (構造体は D2.1)
  - 値オブジェクト (`SpecElementId` / `AdrAnchor` / `ConventionAnchor` / `ContentHash` / `InformalGroundKind` / `InformalGroundSummary`) を newtype として新設 (D2.1)
  - 新モジュール `libs/domain/src/plan_ref/` を導入し、ref 種別ごとに 1 ファイル (`adr_ref.rs` / `convention_ref.rs` / `spec_ref.rs` / `informal_ground_ref.rs` + `mod.rs`) で値オブジェクト + ref struct + validation + unit tests を配置 (D2.1)
  - metadata.json スキーマ: `tasks[]`, `plan`, `status` を削除し identity field のみに (D1.4 / D6.1)。schema_version を v5 に変更。`status_override` (optional) を明示的に identity field として追加。`derive_track_status` / `is_activated` を domain に新設 (D1.4 派生ステータス / activation 不変条件)
  - `verify-latest-track-local`: impl-plan.json が存在するときのみ task 項目チェック (D6.1)
  - `verify-track-metadata-local`: identity field のみ検証 (D6.1)
  - `sotp track type-signals` / `baseline-capture` / 関連 verify: 空カタログを拒否している場合は受け入れるよう修正 (D6.4)
  - plan.md / spec.md renderer: 複数ファイルを集約する形に変更 (plan.md は metadata.json + impl-plan.json、spec.md は spec.json + task-coverage.json)
  - `usecase::task_completion::check_tasks_resolved_from_git_ref` (`libs/usecase/src/task_completion.rs`): tasks[] の参照元を `metadata.json` から `impl-plan.json` に切り替え。関連する `TrackBlobReader::read_track_metadata` port も impl-plan.json を読む形に改修 (D6.2 / 原典は `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D9)
- 新規作成:
  - `track/items/<id>/impl-plan.json` (Phase 3 成果物、D1.4)
  - `track/items/<id>/task-coverage.json` (Phase 3 成果物、D1.4)
  - `knowledge/conventions/workflow-ceremony-minimization.md` (D5)
  - `knowledge/conventions/pre-track-adr-authoring.md` (D4)
  - `sotp verify plan-artifact-refs` (D2.3、task-coverage の突合検証も担う)
  - `track/items/<id>/research/` サブディレクトリ (D1.6)
  - 独立 phase コマンド 3 つ: `/track:init` (Phase 0)、`/track:spec` (Phase 1)、`/track:impl-plan` (Phase 3)
  - `.claude/agents/adr-editor.md` (新 capability、ADR back-and-forth 修正担当、D4 / 展開フェーズ 6.5)
  - `.harness/config/agent-profiles.json` への `adr-editor` capability 追加 (provider = claude、scope = `knowledge/adr/*.md`)
- 再定義:
  - `/track:plan`: 従来の一括生成から **orchestrator** (4 コマンド順次 invoke) に役割変更
  - `/track:design`: 現行を Phase 2 専任に (責務範囲の明確化)
- 本 ADR の範囲外 (別 ADR で実装):
  - catalogue-signal (型契約 → 仕様書) の実装 — `tddd-ci-gate-and-signals-separation` ADR 等で担う
  - canonical block / `## Canonical Blocks` セクションの扱い (Q14、別 ADR で再検討)
- 廃止:
  - `cargo make spec-approve` (approved 概念の消滅に伴う)

---

## 未解決の論点

### Q1: `/adr:new` ヘルパーコマンドの要否
- [a] 作らない (現状の対話ベースで十分)
- [b] 軽量のテンプレート生成コマンド
→ 第一候補は [a]。自家試用で必要性が見えたら [b]

### Q2: 独立 phase コマンドと `/track:plan` の分担 (解決済み: 独立コマンド採用)
- `/track:init` (Phase 0)、`/track:spec` (Phase 1)、`/track:design` (Phase 2)、`/track:impl-plan` (Phase 3) を独立コマンドとして新設
- `/track:plan` は上記 4 コマンドを順次 invoke する orchestrator として再定義
- 各独立コマンドは単独で再 invoke 可能、back-and-forth (D0.1) で元 writer を直接呼べる
- 詳細: 「コマンド境界」セクション参照

### Q3: ADR の例示マーカー運用
- `<!-- illustrative, non-canonical -->` をテンプレート生成ツールで自動挿入する
- CI でマーカー付きのコードブロックを canonical 候補から除外する

### Q4: planner / designer / adr-editor が失敗したときの後始末
- main が中途半端な成果物を削除するか、ユーザーに判断を仰ぐか
- adr-editor 固有の論点: auto-edit 中に失敗した場合、working tree の未コミット変更を破棄するか保持するか (終端処理 D0.1 と一貫させるなら保持して user 判断を仰ぐ方向が自然)

### Q5: フェーズゲートの UI
- `cargo make track-status` の拡張、registry.md にフェーズ列を追加

### Q6: `sotp verify plan-artifact-refs` の実装規模
- 既存の `sotp verify doc-links` と同等規模と想定

### Q7: 展開の自家試用基準
- 2–3 track で次のフェーズに進む

### Q8: 型カタログスキーマ v2
- derive / generic bounds / visibility の表現
- 別 ADR で起草する

### Q9: track 内から track 横断への昇格手順
- track の型カタログエントリを convention や ADR に昇格させる手順
- 別 ADR で扱う

### Q10: 既存 ADR の遡及扱い
- 型詳細を本文内に書いている既存 ADR (今回の review-scope-prompt-injection の ADR D1 など)
- [a] 遡及なし (historical 扱い)
- [b] 遡及的に書き換え
→ 第一候補は [a]

### Q11: ADR 事前確認を満たさないときの UX
- 本 ADR では **厳密モードのみをサポート**する (一時停止してユーザーに ADR 作成を促す)。緩和モード (stub ADR で不完全な状態のまま進行、後から整備) は意図的にサポート対象外とする。理由は Rejected Alternative U 参照

### Q12: `型契約 → 仕様書` シグナルの仕様
- README ロードマップで「計画中」
- 本 ADR の前提。展開フェーズ内で設計する

### Q13: spec.json 要素への明示 ID 付与 (解決済み: [a] 確定)
現行 spec.json の requirement 要素は `{text, sources, task_refs?}` で、識別は `text` に依存している。task-coverage.json から参照するには明示的 `id` を要素に付与するのが望ましい:
- [a] 要素ごとに ID を付与する (例: `{id: "IN-01", text: ..., adr_refs: [...], convention_refs: [...]}`)。task-coverage / SpecRef.anchor は `id` で参照。text 変更に頑健
- [b] 要素の順序 + セクション名で参照 (例: `in_scope[2]`)。spec 編集での順序変更に脆弱
- [c] text そのものを key にする。text 変更で参照が壊れる、冗長
→ **[a] で確定** (本 ADR で確定事項に昇格、D2.1 SpecRef.anchor の基盤として必須)。展開フェーズ 1 のスキーマ分割と同時に ID を付与する

### Q14: canonical block / `## Canonical Blocks` セクションの扱い
現行ルール (`.claude/rules/01-language.md:28` / `.claude/agents/planner.md` / `.claude/skills/track-plan/SKILL.md` L406-416) では planner 出力の一部を canonical block として verbatim English で保存する。L2 (複製の許容) の根本原因にもなっているが、本 ADR では **扱いを決定しない** (scope 外):
- 本 ADR の phase 別 SSoT 分離 (D1) により各成果物の SSoT は明確化された
- しかし canonical block が扱っていた非型コンテンツ (prompt / SQL / schema 断片 / アルゴリズム実装方針など) の配置先は別議題
- 「canonical block を廃止するか」「縮小して存続させるか」「役割を再定義するか」は別 ADR で代替案と共に検討する
- 本 ADR の決定との整合: 型関連は型カタログ (D1.3) が SSoT、決定は ADR (D1.1) が SSoT として既に置き換え可能

### Q15: AdrAnchor / ConventionAnchor の semantic 厳密化 + AdrRef / ConventionRef の hash 追加

本 ADR D2.1 で 4 独立構造体 (`AdrRef`, `ConventionRef`, `SpecRef`, `InformalGroundRef`) を導入、file ベース 3 ref に `anchor` field を required で配置する (reviewer が参照先の位置を即座に判別できるため、現行 string 参照からのデグレを回避)。`InformalGroundRef` は file 対象を持たず anchor / hash の議論対象外。ただし `AdrAnchor` / `ConventionAnchor` は newtype 内部で loose validation (非空文字列のみ) に留め、semantic 厳密化と `AdrRef.hash` / `ConventionRef.hash` の追加は Q15 の後続 ADR で行う (markdown の semantic は対象境界が曖昧で別途議論が必要なため)。論点:

- **AdrAnchor / ConventionAnchor の semantic** (markdown ファイル):
  - heading text そのもの (`D0`, `D1.1`)
  - GitHub-style slug (`d0`, `d1-1`)
  - 独自 HTML コメント marker (`<!-- anchor: d0 -->`)
  - 選択した semantic に合わせて newtype コンストラクタの validation を強化

- **hash field の追加と対象範囲** (markdown ファイル):
  - convention: ファイル全体の SHA-256 が有力候補 (更新頻度が低いため section 境界を気にしなくてよい)
  - ADR: ファイル全体 / anchored section 未定
  - markdown の「セクション」境界をどう抽出するか (heading から次の同 level heading まで?)

- **ADR / convention の半構造化問題**:
  - markdown は自由形式で、厳密な「セクション」概念が曖昧
  - 現行 ADR で慣用的な `## Dx: ...` / `### Dx.y: ...` 規約をどこまで formalize するか

**本 ADR スコープ外**。別 ADR で以下を含めて設計:
- `AdrAnchor` / `ConventionAnchor` の内部 validation 厳密化 (field 自体は本 ADR で追加済み、newtype の内部実装のみ変更する形)
- `AdrRef` / `ConventionRef` への `hash` field 追加 (required で導入、`ContentHash` newtype を再利用、後方互換は不要)
- markdown 系 anchor の resolution ロジック (anchor → markdown 内位置)
- hash アルゴリズムと対象範囲の定義 (markdown ファイルの section 境界)
- markdown ベース成果物 (ADR / convention) での section 境界抽出ロジック
- `sotp verify plan-artifact-refs` の `AdrRef` / `ConventionRef` 側 semantic anchor resolution 実装 (`AdrAnchor` / `ConventionAnchor` の loose validation は本 ADR で有効; `SpecRef` 側の resolution は後続 ADR `2026-04-23-0344` §D1.2 の `sotp verify catalogue-spec-refs` が担当)

### Q16: Writer 境界の自動強制
D4 で「各 phase の成果物はその phase の元 writer だけが書く」と原則を定めたが、現状の違反検出は **subagent プロンプト + 人手レビュー** のみで、自動強制機構が無い。論点:

- **宣言の SSoT**: `.claude/agents/*.md` の subagent markdown で Scope Ownership を宣言
- **routing の SSoT**: `.harness/config/agent-profiles.json` が capability → provider を管理
- **自動強制の候補**:
  - Hook (write-time): 特定 capability が禁止 path へ write しようとしたら block
  - CI gate (post-commit): commit diff を走査し、writer 境界違反を検出
  - agent-profiles.json への拡張: `writer_scope` field を追加し、各 capability の書き込み可能 path を宣言する形で hook / CI 検証に使う
- **スコープ**: これらの自動強制は本 ADR の範囲外 (prompt + 人手で運用開始)。自家試用で違反頻度が問題になったら別 ADR で設計
- **判断基準**: 運用ログで「writer 境界違反」が 1 track あたり数件超える場合に自動化を優先、そうでなければ prompt 運用を継続

---

## 展開計画

各展開フェーズは 2–3 track で自家試用する。

### 展開フェーズ 0: ルールと convention の明文化

- SKILL / command / agent の刷新 (ADR 生成ステップの削除、ADR 事前確認の追加、approved の削除)
- `workflow-ceremony-minimization.md` / `pre-track-adr-authoring.md` の新規作成
- CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md に track 前段階 + 3 フェーズを明記
- D1.6 の research 再配置

### 展開フェーズ 1: スキーマ分割 + CI ゲート再整理

- spec.json から `status`, `approved_at`, トップレベル `content_hash`, 各要素の `task_refs` を削除
- spec.json の各要素に明示的 `id` フィールドを追加 (task-coverage.json からの参照基盤、Q13)
- spec.json を構造化: 各要素の `sources[]` 単一 field を廃止し `adr_refs: Vec<AdrRef>` / `convention_refs: Vec<ConventionRef>` / `informal_grounds: Vec<InformalGroundRef>` に分割。top-level `related_conventions` も `Vec<ConventionRef>` に
- カタログエントリのスキーマに `spec_refs: Vec<SpecRef>` + `informal_grounds: Vec<InformalGroundRef>` を追加
- 値オブジェクト newtype 新設: `SpecElementId` / `AdrAnchor` / `ConventionAnchor` / `ContentHash` / `InformalGroundKind` / `InformalGroundSummary` (D2.1)
- 新モジュール `libs/domain/src/plan_ref/` (ref 種別ごとに 1 ファイル構成) を導入
- metadata.json スキーマを identity のみに縮小 (`tasks[]`, `plan` を削除) (D1.4)
- `impl-plan.json` schema 新設 (tasks + plan.sections) (D1.4)
- `task-coverage.json` schema 新設 (spec 4 セクション (in_scope / out_of_scope / constraints / acceptance_criteria) の要素ごとの task_refs、現行 `spec.rs` の `task_refs` を外出し) (D1.4)
- `verify-latest-track-local` を「impl-plan.json があれば task 項目チェック」に改訂 (D6.1)
- `verify-track-metadata-local` を identity のみ検証に (D6.1)
- plan.md renderer を metadata.json + impl-plan.json の集約に変更
- spec.md renderer を spec.json + task-coverage.json の集約に変更
- schema validator の更新
- `track/items/<id>/research/` サブディレクトリ対応

### 展開フェーズ 2: `sotp verify plan-artifact-refs` の実装

- 各 ref field の参照先整合 CLI: spec.json の `adr_refs[]` / `convention_refs[]` および task-coverage.json の ref field が対象。catalogue `spec_refs[]` の `SpecRef.anchor` / `SpecRef.hash` 検証は対象外 (後続 ADR `2026-04-23-0344` §D1.2 の `sotp verify catalogue-spec-refs` が担当)
- task-coverage.json の coverage 強制 + referential integrity 検査 (現行 `spec_coverage::verify` 踏襲)
- `cargo make ci` への組み込み
- **catalogue-signal (型契約 → 仕様書) は本 ADR スコープ外**。別 ADR (例: `tddd-ci-gate-and-signals-separation`) で実装する。signal 実装までは catalogue の `spec_refs[]` は semantic 面で advisory 扱い (schema / drift 検証は後続 ADR で有効)

### 展開フェーズ 3: レビューア briefing の刷新

- 内容 fidelity の検証を削除 (レビューラウンド数の削減)
- フェーズ別のレビューア briefing を追加

### 展開フェーズ 4: フェーズ 0 / 1 独立コマンドの新設

- `/track:init` (Phase 0) と `/track:spec` (Phase 1) を新設
- それぞれ単独で invoke 可能、事前確認 (ADR 存在 / metadata.json 存在) は各コマンドが自己責任で行う
- SKILL.md / command.md / planner agent を刷新 (Phase 1 は planner に writer 委譲)
- 2–3 track で自家試用

### 展開フェーズ 5: フェーズ 2 ワークフローの刷新

- `/track:design` を Phase 2 専任に (現行の責務を明確化)
- designer エージェントに書き込み権限を付与
- signal 評価は後続 ADR (D3.2)

### 展開フェーズ 6: フェーズ 3 独立コマンドの新設

- `/track:impl-plan` を新設
- impl-plan.json + task-coverage.json の生成を Phase 2 の後に分離 (planner が書く)

### 展開フェーズ 6.5: `/track:plan` orchestrator への再定義

- 従来の `/track:plan` を 4 コマンド (`/track:init` → `/track:spec` → `/track:design` → `/track:impl-plan`) を順次 invoke する orchestrator に再定義
- ADR 事前確認と各 phase ゲート評価を orchestrator 側で担当
- **D0.1 逐次伝播の自動化**: 各 phase gate で 🔴 / ERROR 検出時に元 writer を自動再 invoke、ADR まで遡った場合は adr-editor サブエージェントを自動 invoke して ADR 自動編集 (ADR に commit 履歴がある場合のみ、無ければ user pause)。loop 中は working tree のみ変更し、コミットはしない
- **終端判断**: `/track:plan` 完了時に ADR working tree に HEAD からの diff があれば user に判断を仰ぐ (accept / revert / 手動修正 / 中止)
- **再試行制御**: `/track:plan` は整数ひとつの positional 引数で `max_retry` を受ける (省略時 5、フラグ名なし)。各 phase loop で閾値超過 → user pause
- **adr-editor capability の新設**: `.claude/agents/adr-editor.md` を新規作成 (scope: `knowledge/adr/*.md` のみ書き込み可、他禁止)。`.harness/config/agent-profiles.json` に `adr-editor` capability を追加 (provider = claude、model は planner と同等以上を推奨)
- 2–3 track で自家試用

### 展開フェーズ 7: フック強制 (任意)

- 自家試用完了 3 track 以上
- 具体的に何を hook で強制するかは別 ADR (canonical block 扱い、catalogue-signal 等) の進捗を見てから決定

---

## 一括 track 化するときのタスク順序案

| Task | 展開フェーズ | 対象決定 |
|---|---|---|
| T1: ルール + convention の明文化 (approved 廃止 / research 再配置 / 手順最小化) | 0 | D1.6, D5, 状態廃止 |
| T2: スキーマ分割 + CI ゲート再整理 (spec 構造化参照 + カタログ sources + metadata identity 化 + impl-plan.json / task-coverage.json 新設 + verify-*-local を file 存在ベースに改訂 + renderer 集約化 + 空カタログ許容 + `check_tasks_resolved_from_git_ref` の impl-plan.json 読み替え) | 1 | D1.4, D2.1, D2.2, D6.1-D6.4 |
| T3: `sotp verify plan-artifact-refs` (task-coverage 突合検証。catalogue-signal は別 ADR で) | 2 | D2.3 |
| T4: レビューア briefing の刷新 | 3 | 展開フェーズ 3 |
| T5: `/track:init` + `/track:spec` 新設 (Phase 0/1 独立コマンド) | 4 | D0.0 (フェーズ 0/1 行), D3.1, D4 |
| T6: `/track:design` 責務刷新 (Phase 2 専任) | 5 | D0.0 (フェーズ 2 行), D3.2, D4 |
| T7: `/track:impl-plan` 新設 (Phase 3 独立コマンド) | 6 | D0.0 (フェーズ 3 行), D4 |
| T7.5: `/track:plan` orchestrator 再定義 (4 コマンド順次 invoke + D0.1 ループ自動化 + adr-editor capability 新設 + ADR 自動編集 (git 履歴ベース判定) + 終端 diff 判断 + `max_retry` 整数引数、default 5) | 6.5 | コマンド境界, D0.1, D4 |
| T8: フック強制 (任意) | 7 | 全 D |

---

## 補足

- 本 ADR は track 作成手順の構造改善を扱う
- `feedback_enforce_by_mechanism` / `feedback_no_unnecessary_stops` と整合する
- repo の ADR 慣例 (1 ファイルで D1..Dn を並べる) と整合する
- README.md の SoT Chain 記述を本 ADR の構造的根拠とする
- `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` は TDDD シグナルの CI 接続と信号機評価の型カタログからの分離を扱う別 ADR。本 ADR とは責務が独立 (本 ADR は phase 分離 + SSoT + CI 存在ゲート、先行 ADR はシグナル基盤の配置とゲート接続) で、並行進行可能
- **signal 実装の責務分担**: 本 ADR は catalogue に `spec_refs: Vec<SpecRef>` schema を追加し、値オブジェクト (`SpecElementId` / `ContentHash`) と共に `sotp verify plan-artifact-refs` で **spec.json の `adr_refs[]` / `convention_refs[]`** の schema・file 存在・loose anchor 検証 + **task-coverage.json の coverage 強制および referential integrity** (spec element ID が spec.json に存在するか、task ID が impl-plan.json に存在するか) を提供するところまでを扱う。`AdrRef` / `ConventionRef` は hash field を持たないため hash drift 検証は本 ADR では不可能。spec 側も `adr_refs: Vec<AdrRef>` / `convention_refs: Vec<ConventionRef>` + newtype (`AdrAnchor` / `ConventionAnchor`) を導入。catalogue の `spec_refs[]` anchor/hash drift 検証は `sotp verify plan-artifact-refs` の対象外であり、後続 ADR `2026-04-23-0344` §D1.2 で導入する `sotp verify catalogue-spec-refs` が担当する。signal 評価 (型契約 → 仕様書、README ロードマップの「計画中」項目) は本 ADR スコープ外で、上記別 ADR または後続 ADR が担当する。signal 実装前は `spec_refs[]` の semantic 品質は未検証

---

## 付録 A: 観測データの出典

- 26 ラウンドのレビュー詳細: `track/items/review-scope-prompt-injection-2026-04-18/review.json`
- 現行フロー:
  - `.claude/rules/01-language.md`
  - `.claude/skills/track-plan/SKILL.md`
  - `.claude/commands/track/plan.md`
  - `.claude/commands/track/design.md`
  - `.claude/agents/planner.md`
  - `.claude/agents/designer.md`
- README.md §SoT Chain (本 ADR の構造的根拠)

## 付録 B: 期待効果の試算

| 指摘の種類 | 件数 | 新方式での扱い |
|---|---|---|
| citation の出典ずれ | 約 12 | D2.3 の CI ゲートで事前検出 |
| scope 境界の食い違い | 約 6 | D1 + D2.3 の CI ゲートで事前検出 |
| 立ち上げ時の状態記述が古い | 約 4 | D1.1 の `Status` 見出し廃止 + D6.3 の phase 判定で自然消滅 (状態記述を file 存在で判定) |
| タイムスタンプの不整合 | 約 3 | 本 ADR の `approved_at` 削除 (D1.2) で自然消滅 |
| 型設計の矛盾 | 約 2 | 型カタログスキーマ v2 (Q8) で根治 |
| 検証ゲートの設計瑕疵 | 約 2 | レビューアの本質的な論点 |

約 25 件が CI 側 / 本 ADR の構造変更で解消し、レビューアは 2–4 件に集中する (残る関心事は Q8 型カタログスキーマ v2 + 検証ゲートの本質的論点)。予想ラウンド数: 26 → 2–3 程度。

## 付録 C: フェーズ順序の代替検討

| 順序 | 採否 | 理由 |
|---|---|---|
| **track 前段階 ADR → 契約 → 型 → 実装計画** (採用) | ✓ | SoT Chain ①②③ を厳守、ADR のライフサイクル独立を保つ |
| track 内 ADR → 契約 → 型 → 実装計画 | ✗ | ADR のライフサイクルが混ざる |
| 契約 → 型 → 実装計画 (逆流許可) | ✗ | SoT Chain の逆流 |
| 決定 → 契約 → 実装計画 → 型 | ✗ | 型が task より後になると依存逆転 |
| 決定 → 型 → 契約 → 実装計画 | ✗ | spec が型依存の記述を許すと振る舞い責務が曖昧になる |

採用案は「上位の成果物が下位の成果物に制約を与える一方向依存」を満たす唯一の配置。

---

## Follow-up (2026-04-22)

**トリガー**: PR #107 の Codex Cloud レビューが、`TransitionTaskUseCase::execute` および `AddTaskUseCase::execute` における非原子的な 2 段書き (impl-plan.json 書き込み → metadata.json の status 同期) を P0 として指摘。第 2 書き込みが失敗すると 2 ファイルが不整合状態に陥る。

**解決策**: `metadata.json` から `status` フィールドを完全に除去し、schema_version を v4 → v5 (identity-only) に更新。task の状態遷移は `impl-plan.json` への単一ファイル書き込みで完結する。track の現在 status は `domain::derive_track_status(impl_plan, status_override)` で on-demand に算出する。`status_override` は「ユーザーによる blocked / cancelled の意図的注釈」であるため identity に残す。

**影響**:

- task 遷移が単一ファイル原子書き込みになり、2 段書き由来の不整合が構造的に解消する
- `derive_track_status` 関数を domain に新設 (算出規則は D1.4「派生ステータス」参照)
- `is_activated() ≡ branch.is_some()` を activation の唯一の判定基準として明文化 (D1.4「activation 不変条件」参照)
- v4 (status フィールドを持つ旧形式) の metadata.json は `verify-track-metadata-local` でスキップされる (既存 track への遡及適用なし — 移行ポリシー参照); v5 への書き換えなしに decode しようとする `codec::decode` は v4 を拒否するが、verify ゲート自体は legacy track を skip する
- `bin/sotp` CLI の `track-transition` 等の task 遷移コマンドは impl-plan.json のみを読み書きするよう変更済み

**本 ADR の変更箇所**: §D1.4 metadata.json 定義の「含めるもの」「含めないもの」「書き換え」の更新、「派生ステータス」「activation 不変条件」各サブセクションの新設、§D6.1 `verify-track-metadata-local` 行の更新、移行セクション metadata.json スキーマ行の更新、利点セクション「状態フィールド廃止」記述の補完。

**2026-04-23 Correction**: 本 Follow-up で追加した `check_impl_plan_presence` invariant は、`/track:init` が branch create する設計 (ADR 2026-04-22-0829 §D3) および Phase 0-2 の自然な progression (metadata → spec → 型カタログ → impl-plan) と衝突することが実運用で判明したため revert した。`derive_track_status` の「`impl-plan.json` 不在 → Planned」graceful fallback が semantic の正しさを既に担保しており、invariant は redundant な defensive check であった。関連する code 削除 (libs/domain/src/impl_plan.rs::check_impl_plan_presence / ImplPlanPresenceError、libs/infrastructure/src/verify/latest_track.rs + apps/cli/src/commands/make.rs の call site) は track `catalogue-spec-signal-activation-2026-04-23` の T025 で実施。
