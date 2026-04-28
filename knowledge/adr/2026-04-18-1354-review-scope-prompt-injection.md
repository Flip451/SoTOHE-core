---
adr_id: 2026-04-18-1354-review-scope-prompt-injection
decisions:
  - id: 2026-04-18-1354-review-scope-prompt-injection_grandfathered
    status: accepted
    grandfathered: true
---
# review-scope.json に scope 別 briefing 注入機構を追加する — plan-artifacts scope の新設

## Status

Proposed

関連 ADR: [Review System v2: frozen scope 廃止とスコープ独立型レビュー](2026-04-04-1456-review-system-v2-redesign.md) の briefing 経路拡張として位置づける。

## Context

### §1 現状: review briefing の scope 間非対称性

現行の `/track:review` は Agent Teams で scope (`domain` / `usecase` / `infrastructure` / `cli` / `harness-policy` / `Other`) ごとに `review-fix-lead` を起動し、各 reviewer が Codex に briefing を渡して findings を収集する構造になっている。

この構造は **「どのファイルをどの scope に振り分けるか」は `review-scope.json` の patterns で制御できる一方、「その scope の reviewer にどういう観点を注入するか」はグローバルに固定** されている。すなわち:

- `domain` の reviewer には「型安全 / 不変条件 / enum-first / typestate」の観点を効かせたい
- `harness-policy` の reviewer には「permission list / hook / guardrail 整合性」を効かせたい
- plan 成果物 (`plan.md` / `spec.md` / `metadata.json` / `verification.md`) の reviewer には **「wording nits はスキップ、factual error / contradiction / broken ref / infeasibility のみ挙げる」** を効かせたい

3 つ目は特に feedback memory として繰り返し積まれてきた知見であり、ユーザーが毎回 briefing に手で付け足している。これは機械化できる。

### §2 現状: plan 成果物の review scope が存在しない

`track/review-scope.json` の `groups` には以下のみが定義されている:

```json
"groups": {
  "domain":         { "patterns": ["libs/domain/**"] },
  "usecase":        { "patterns": ["libs/usecase/**"] },
  "infrastructure": { "patterns": ["libs/infrastructure/**"] },
  "cli":            { "patterns": ["apps/**"] },
  "harness-policy": { "patterns": [".claude/commands/**", ...] }
}
```

一方 `review_operational` は `track/items/<track-id>/review.json` のみを excluded 扱いにしている。その結果:

- `track/items/<track-id>/plan.md` / `spec.md` / `metadata.json` / `verification.md` / `spec.json` / `reports/**` などの **plan 成果物は `Other` scope に落ちる**
- `Other` scope の reviewer は「code と doc が混じった残り物」を見るので、plan 成果物特有の review policy (wording nits スキップ等) を効かせられない

これは `/track:plan` のレビュー時に特に問題になる。plan 成果物は:

- 日本語 markdown 主体で、英語コードレビューのベストプラクティスが当てはまらない
- 引用 / ADR 参照 / task_refs / source の整合性が review 対象の中心
- wording 改善提案は planner の設計判断と衝突する (内部ノート系なら尚更)

### §3 現状: briefing の組み立て経路

`apps/cli/src/commands/review/codex_local.rs` は `--briefing-file <path>` で渡された markdown をそのまま prompt として Codex に渡す。briefing 生成側 (`review-fix-lead` agent) は scope 名と変更ファイル一覧からテンプレート的に briefing を組み立てるが、**scope 固有の instruction を差し込むフックは現状存在しない**。

### §4 ユーザーが期待する体験

ユーザーは繰り返し「plan review 時はこの観点で見て欲しい」「strategy docs は低い品質バーで見て欲しい」を会話中に手で注入している。これは:

1. **再現性がない** — 会話セッションが変わると同じ指示を再入力する必要がある
2. **自己 review から Codex への伝達が抜け落ちやすい** — 過去の 28-round loop に至った原因の一つ
3. **scope 単位で分岐したい** — domain と plan-artifacts で briefing の主眼が全く違う

これらは「scope ごとに briefing を追記するメタ文字列を設定可能にする」で機械化できる。

## Decision

### D1: `review-scope.json` の `groups` エントリに `briefing_file` の任意フィールドを追加する

schema (v2 のマイナー拡張、後方互換):

```json
{
  "version": 2,
  "groups": {
    "domain": {
      "patterns": ["libs/domain/**"],
      "briefing_file": "track/review-prompts/domain.md"
    },
    "plan-artifacts": {
      "patterns": [
        "track/items/<track-id>/**",
        "knowledge/adr/**",
        "knowledge/research/**"
      ],
      "briefing_file": "track/review-prompts/plan-artifacts.md"
    },
    "harness-policy": {
      "patterns": [".claude/commands/**", ...],
      "briefing_file": "track/review-prompts/harness-policy.md"
    }
  }
}
```

**設計原則:**

- `briefing_file` は **任意フィールド** (既存 scope は briefing なしで引き続き動作)
- `briefing_file` のみ提供し、inline 文字列フィールドは持たない (one way to do it — 短文でも .md を 1 枚置く)
- patterns は `<track-id>` placeholder を既存ルール通り展開する (既存 `review_operational` と同じ挙動)
- `briefing_file` の値は **workspace-relative path 文字列をそのまま保持** する (loader で内容読み込みや canonicalize は行わない — D4 の注入方式の帰結)

### D2: `ReviewScopeConfig` / `GroupEntry` に `briefing_file` パスを持たせる

domain 層 (`libs/domain/src/review_v2/scope_config.rs`):

```rust
pub struct ReviewScopeConfig {
    scopes: HashMap<MainScopeName, ScopeEntry>, // 旧: Vec<GlobMatcher>
    // ...
}

pub struct ScopeEntry {
    matchers: Vec<GlobMatcher>,
    briefing_file: Option<String>,  // workspace-relative path; 未指定なら None
}
```

`briefing_file` はパス文字列をそのまま保持するだけで、domain / infrastructure いずれも **ファイル内容の読み込みを行わない** (D4 参照)。したがって `ScopeBriefing` enum や inline/file variant の切り替えロジックは不要で、hexagonal 原則の I/O 配置問題も発生しない。

### D3: `plan-artifacts` scope を新設する

`track/review-scope.json` に以下を追加:

```json
"plan-artifacts": {
  "patterns": [
    "track/items/<track-id>/**",
    "knowledge/adr/**",
    "knowledge/research/**"
  ],
  "briefing_file": "track/review-prompts/plan-artifacts.md"
}
```

pattern 構成の意図:

- **`track/items/<track-id>/**`** — 現行 track の成果物一式 (`plan.md` / `spec.md` / `spec.json` / `metadata.json` / `verification.md` / `reports/**` ほか)。`<track-id>` placeholder は `ReviewScopeConfig::new` が現行 track ID に展開する (既存 `review_operational` と同じ挙動。ただし本 ADR 起草時点の実装では group pattern に対して placeholder 展開が未実装のため、T001 で `expand_track_id` を groups loop にも適用する必要がある — `libs/domain/src/review_v2/scope_config.rs` 参照)。`review.json` は `review_operational` で先に除外されるため `**` に含めても影響なし (classify() は operational → other_track → named scopes の順)。列挙ではなく `**` にすることで将来の成果物追加 (`design.md` / `signals/*.json` / 新しい rendered view 等) に対して future-proof
- **`knowledge/adr/**`** — ADR は planning の output (新規 / 改訂) として plan 作業と同 round で review されるべき成果物。severity policy (factual error / contradiction / broken reference) がそのまま適用できる。diff-based review なので触っていない ADR は review 対象外になる (自動フィルタ)
- **`knowledge/research/**`** — `/track:plan` が保存する研究ノート (planner 出力を `YYYY-MM-DD-HHMM-planner-{feature}.md` として verbatim copy したもの、`{feature}-codebase.md` / `{feature}-crates.md` / `version-baseline-*.md` 等の input 系も含む)。output 系は実質的に plan 成果物であり、input 系も track と 1:1 対応する作業ノートとして同じ severity policy (factual error / ADR との矛盾 / broken reference) で見るのが実運用に合う。研究ノート独自の review 観点 (情報源の時効性 / 外部 citation の妥当性) は factual error のサブカテゴリとして吸収する。diff-based review なので触っていない研究ノートは review 対象外 (自動フィルタ)

将来 track 配下に code-y なファイル (テストフィクスチャ、生成コード等) が置かれた場合は、`track/items/<track-id>/fixtures/**` のような sub-pattern を別 scope に追い出すか `review_operational` に移すかで個別対応する。初版から過剰設計しない。

`track/review-prompts/plan-artifacts.md` の初版内容 (抜粋):

```markdown
## Plan 成果物レビューの severity policy

このレビューは `track/items/<track-id>/` 配下の成果物 (`plan.md` / `spec.md` / `spec.json` / `metadata.json` / `verification.md` / `reports/**` 等)、本 track で新規 / 改訂された ADR (`knowledge/adr/**`)、および本 track の planner 出力 (`knowledge/research/**`) を対象とする。以下のみを findings として報告すること:

- **factual error**: 事実誤認 (存在しない CLI / ファイルパス / ADR 番号)
- **contradiction**: 複数箇所で矛盾する記述
- **broken reference**: `[source: ...]` / `[tasks: ...]` の参照先が存在しない
- **infeasibility**: tasks[] の依存順や workload が実装不能
- **timestamp inconsistency**: `updated_at` / commit_hash の不整合

以下は **報告しないこと**:

- wording nit (言い回しの好み、冗長さ、トーン)
- 英語 / 日本語表記の統一提案 (明示的な表記ルール違反を除く)
- planner の設計判断に対する代替案 (planning gate で既に確定済み)
```

`track/review-prompts/` ディレクトリを新設し、scope ごとの briefing md をそこに集約する。

### D4: briefing の注入経路 (ファイルパス参照方式、現行 pattern に整合)

**既存の briefing 経路:**

`apps/cli/src/commands/review/codex_local.rs` は `--briefing-file <path>` で渡された briefing をそのまま Codex prompt に流す。ラッパー (`cargo make track-local-review`) は内部で `"Read {path} and perform the task"` に変換する (`.claude/rules/10-guardrails.md` 参照)。つまり **reviewer 自身の Read tool でファイルを取りに行く** のが既存 pattern。

**scope briefing も同じ pattern で注入する:**

主 briefing の本文連結 (本文を fs::read して preamble に挿入) ではなく、**主 briefing に「scope briefing を Read せよ」という参照行を 1 本追加する** だけ。

主 briefing の最後に以下のような節を composer が emit する (scope の `briefing_file` が `Some` の場合のみ):

```markdown
## Scope-specific severity policy

このレビューの scope は `plan-artifacts` である。以下の scope 固有 severity policy を **必ず先に Read ツールで読み込み**、その方針に従って findings を選別すること:

- `track/review-prompts/plan-artifacts.md`
```

**この方式の利点:**

- **現行 pattern と同形**: briefing を「ファイルパスとして渡す → reviewer が Read する」という既存流儀に完全に揃う。ラッパー変換 (`"Read {path} and perform the task"`) と同じ発想の再適用
- **I/O が domain / loader に漏れない**: `briefing_file` の内容を読むのは reviewer sandbox 内の Read tool のみ。loader / composer は path 文字列を扱うだけ
- **trusted_root check 不要**: reviewer sandbox (`read-only`) が workspace 外の read を自然に拒否するため、loader で briefing_file の path escape を防ぐ二層 check は不要
- **ホットリロード的な挙動**: briefing md を編集して即 review 再開した場合、次の round で reviewer が最新版を Read する (composer 再合成不要)
- **実装量が最小**: composer は参照行 1 本を `push_str` するだけ。新規の fs::read も trusted_root check も enum も追加しない

**実装場所:**

`apps/cli/src/commands/review/` 配下の既存 briefing composer に、scope 名から `ReviewScopeConfig` を引いて `briefing_file` が `Some` なら参照行を append するロジックを追加する。`review-fix-lead` prompt (`.claude/agents/review-fix-lead.md`) には「Scope-specific severity policy 節が主 briefing にあれば必ずその md を Read せよ」を明示する 1 段落を追加する。

### D5: `Other` scope は briefing 対象外とする (予約名制約の帰結)

`MainScopeName::new("other")` は `ScopeNameError::Reserved` で reject される (`libs/domain/src/review_v2/types.rs` / `error.rs`)。`review-scope.json` の `groups` に `"other"` エントリを書けないため、`Other` に briefing を付ける経路はそもそも存在しない。

これは設計上の意図として維持する: `Other` は「named scope のどれにも一致しなかった残り物」という predicate-of-absence であり、固有の review 観点を持たない。もし残り物に特別な briefing が必要になった場合は、対応する named scope (例: `misc-docs`) を明示定義してそちらに patterns を寄せるのが正しいアプローチ。

## Rejected Alternatives

### A. briefing を `review-scope.json` に inline 文字列で持たせる

長文 markdown を JSON に埋めると escape (改行 `\n` / ダブルクォート) / diff / markdown lint の全方面で辛い。短文用に inline フィールドを併設する案も検討したが、「短文なら短い .md を 1 枚置けば済む」ため one way to do it 原則で却下。`briefing_file` のみ提供する。

### B. composer で briefing 内容を主 briefing に本文連結する

loader で briefing_file を fs::read して `ScopeBriefing::Inline(String)` に正規化し、composer が preamble に本文ごと連結する案 (初版ドラフトで検討していた方向)。

**Cons**:

- 現行の「ファイルパスを渡して reviewer が Read する」pattern (`codex_local.rs` + ラッパーの `"Read {path} and perform the task"` 変換) と不整合
- loader / infrastructure に fs::read と trusted_root 二層 check が増える
- `ScopeBriefing` enum (Inline / File) の切り替えロジックが domain に漏れる
- briefing md 編集後に composer 再合成が必要 (ホットリロード不可)

→ D4 のファイルパス参照方式を採用することで、これらのコストがすべて消える。

### C. briefing を `.claude/agents/review-fix-lead.md` に scope 別の case 分岐で書く

prompt 側で scope 名を見て分岐する案。
**Cons**: `review-scope.json` と `review-fix-lead.md` で scope が二重管理になる。scope 追加のたびに prompt 改修が必要。SSoT 原則に反する。

### D. scope とは別に `track/review-prompts.json` という独立ファイルを作る

scope config と briefing を別ファイルに分離する案。
**Cons**: scope を追加するときに 2 ファイル同時編集が必要。関連情報が物理的に離れる。review-scope.json の `groups` エントリに収める方が凝集が高い。

## Consequences

### Good

- **会話セッション越しに prompt 再入力が不要になる** — feedback memory に積もった運用知見を config で機械化できる
- **plan 成果物の review が専用 scope で回せる** — `Other` scope の混沌から分離できる
- **28-round loop の防止** — severity policy を briefing に埋め込むことで、reviewer が wording nit で round を伸ばさない
- **scope owner の責務が明確化** — `track/review-prompts/<scope>.md` が各 scope の review contract を明示する

### Bad / Risk

- **schema 変更** — `deny_unknown_fields` を維持するので、`briefing_file` 追加時点で v2 の schema revision を明示する (version フィールドは据え置きで、loader の serde struct に field 追加で対応)
- **briefing ファイルのメンテナンス負担** — `track/review-prompts/*.md` が腐りやすい (code は動くが briefing は古いまま、になる)
- **briefing_file の存在チェックが遅延する** — path 文字列だけ保持して reviewer の Read tool に委ねる方式の帰結として、ファイルが存在しない場合は reviewer が Read に失敗して初めて発覚する (loader 段階では検出しない)。対策は Open Questions Q3 で扱う CI lint
- **review-fix-lead agent の prompt 変更** — 「Scope-specific severity policy 節があれば必ず Read する」の追記が必要

### Migration

- 既存 `review-scope.json` は `briefing_file` なしで引き続き動作 (後方互換)
- `plan-artifacts` scope 追加時のみ `track/review-prompts/plan-artifacts.md` の初版を同時コミット

## Reassess When

- **Codex CLI の prompt 提供 API が変わった場合** — `"Read {path} and perform the task"` 変換が前提とする briefing 経路 (`--briefing-file` → ラッパー変換) が非互換変更を受けたら、D4 の注入方式を再検討する
- **`.harness/config/` 集約が実施された場合** — review-scope.json を `.harness/config/` 配下に移設する別 track が完了した時点で、`briefing_file` の相対パス解決ルールを再記述する (workspace-relative のまま維持するか、config dir 基準に変更するか)
- **scope 固有 briefing が 2-3 件を超えて肥大化した場合** — briefing md が大量の重複を抱えたら、共通 preamble の extract や `briefing_files: [...]` 複数指定への拡張を検討する
- **reviewer sandbox policy が変わった場合** — reviewer が `read-only` ではなくなった (例: workspace-write が必要な review fixture が登場した) 場合、trusted_root check の必要性が復活しうる
- **`Other` scope の振る舞いを変えたくなった場合** — 残り物への briefing 注入ニーズが生じたら、`misc-docs` のような named scope 切り出しで対応するか、`Other` への briefing 注入を許す方針転換を議論する

## Open Questions

- **Q1**: `briefing_file` の配置ディレクトリに convention / lint を課すか? reviewer sandbox が workspace 外 read を拒否するので security 的な制約は不要だが、`track/review-prompts/**` のみ許可する lint を CI で入れるか、`knowledge/conventions/**` 等からも参照可能にするかは運用判断。
- **Q2**: Empty diff の scope (変更ファイルなし) でも briefing を注入するかどうか。例えば `plan-artifacts` に該当する変更が無ければ reviewer 自体をスキップする現状挙動を維持するか、briefing を見せた上で「変更なし」と判定させるかの線引き。
- **Q3**: `track/review-prompts/*.md` の lint / 整合性チェックを CI で入れるか (broken link / 存在しない scope 名への参照を検知)。

## Rollout Plan (想定)

1. **Phase 1 (schema)**: `review-scope.json` v2 `groups.<name>.briefing_file` optional field を追加。`GroupEntry` / `ScopeEntry` に `briefing_file: Option<String>` を足すだけ (fs::read / trusted_root check なし)。
2. **Phase 2 (injection)**: `apps/cli/src/commands/review/` の briefing composer に「scope の `briefing_file` が `Some` なら主 briefing に『Read `{path}`』の参照行を append」を実装。`.claude/agents/review-fix-lead.md` に「Scope-specific severity policy 節を見たら必ず Read する」を明記。
3. **Phase 3 (plan-artifacts)**: `plan-artifacts` scope を `review-scope.json` に追加し、`track/review-prompts/plan-artifacts.md` を新規作成。既存 track でドッグフード。
4. **Phase 4 (他 scope へ展開)**: `harness-policy` / `domain` など他 scope にも段階的に `briefing_file` を整備。feedback memory から対応する briefing を抽出してファイル化。

## Future Migration: `.harness/config/` への集約

本 ADR は briefing の初期配置を `track/review-prompts/` とするが、これは暫定措置であり、将来的には `review-scope.json` ごと `.harness/config/` 配下に移設する構想がある (別 track で扱う)。移設後の想定配置:

- `.harness/config/review-scope.json`
- `.harness/config/review-prompts/plan-artifacts.md`
- `.harness/config/review-prompts/domain.md`
- ... (各 scope に対応)

`.harness/config/` が `agent-profiles.json` (provider wiring) + `review-scope.json` (scope wiring) + `review-prompts/` (scope-specific prompt wiring) の 3 点セットで「review / 実行系 capability configure の SSoT」として揃う形。

**順序の判断 (ROI 比較):**

- **移設 only**: 挙動ゼロ変更のリファクタ。`.harness/config/` の意味整理が唯一の成果
- **briefing 注入 only (本 ADR)**: feedback memory の繰り返し手動注入問題を構造的に解消、28-round loop 防止の仕組み化

→ briefing 注入の方が ROI が高いため **本 ADR を先行** させる。移設は後続 track で実施し、その際に `review-scope.json` と `track/review-prompts/` を一緒に `.harness/config/` 配下へ動かす (どうせ path の一括置換作業なので、briefing dir も相乗りで移動するだけでコストはほぼ追加されない)。

## Notes

- 本 ADR は TDDD カタログや `architecture-rules.json` には触れない。純粋に review workflow の拡張である。
- `plan-artifacts` は `Other` から切り出す形になるため、既存 track の review round 数メトリクスに影響する可能性がある (事実上 scope が 1 つ増えて並列 review が増える)。並列実行の設計は既に Agent Teams で対応済みなので追加対応は不要のはず。
