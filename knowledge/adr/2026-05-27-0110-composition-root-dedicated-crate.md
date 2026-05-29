---
adr_id: 2026-05-27-0110-composition-root-dedicated-crate
decisions:
  - id: D1
    user_decision_ref: "chat_segment:composition-root-design:2026-05-27"
    candidate_selection: "from:[crate-split,module-split] chose:crate-split"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:composition-root-design:2026-05-27"
    candidate_selection: "from:[facade-struct,free-functions-only,box-dyn-primary-port] chose:facade-struct"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:composition-root-design:2026-05-27"
    candidate_selection: "from:[cli-composition-only,cli-composition-plus-usecase] chose:cli-composition-only"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:composition-root-design:2026-05-27"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:composition-root-design:2026-05-27"
    candidate_selection: "from:[phased-commits-with-shim,single-big-bang] chose:phased-commits-with-shim"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:composition-root-design:2026-05-27"
    status: proposed
---
# composition root を専用 crate (apps/cli-composition) に切り出す

## Context

ADR `2026-04-30-0848-cli-via-usecase-only.md` の Rejected Alternative C は、「cli → infrastructure
依存も同時に禁止する純粋 hexagonal 構成は scope が大きいため別判断として切り出す」と明記した。
同 ADR の Reassess When にも「composition root の wiring を含む hexagonal 構造の再編が立ち上がった
とき」と書いてある。本 ADR はその「別判断」に正面から答えるものである。

### 現状の問題

- `apps/cli` が公式 composition root（`knowledge/conventions/hexagonal-architecture.md`
  の "CLI as Composition Root" 節）。責務は clap パース → infra adapter 構築（DI）→ usecase 呼び出し
  → 出力 + ExitCode とされているが、実際の DI 配線は `apps/cli/src/commands/*.rs` の execute 関数に
  分散しており、`main.rs` はディスパッチャに過ぎない。
- `review_v2` だけが非対称で、composition logic が infrastructure 層の
  `libs/infrastructure/src/review_v2/cli_composition.rs`（1507 行）に押し出されている。ファイル冒頭には
  "CLI composition root for v2 review system" と明記されており、アーキテクチャの層境界が崩れている証拠
  である。この非対称は『cli は domain 型を直接 import しない』という制約を cli 層内で満たせなかったための
  回避策として発生した。
- `apps/cli` の `may_depend_on` には `["usecase", "infrastructure"]` が含まれており、各コマンドの
  execute 関数が `infrastructure::` を直接 import できる状態が続いている。module 境界では防げず、
  将来また infra import が各コマンドに忍び込むリスクがある。

### 設計の背景

Codex（gpt-5.5 / high effort）と Claude opus が同一の briefing を受け取って独立に設計し、crate 分割の要否・配置・依存グラフ・dyn-safety 判断・review_v2 の扱いで
両者が一致した。公開 API のスタイル（facade struct 対 free function）と bin の usecase 依存の扱いで
意見が割れたが、orchestrator がそれぞれの批判を踏まえて統合判断を出した。

## Decision

### D1: composition root を専用 crate `apps/cli-composition`（crate 名 `cli_composition`）に切り出す

`apps/cli-composition` を新設し、`architecture-rules.json` に層エントリを追加する。

<!-- illustrative, non-canonical -->
```json
{
  "crate": "cli_composition",
  "path": "apps/cli-composition",
  "may_depend_on": ["domain", "infrastructure", "usecase"],
  "deny_reason": "CLI composition root owns domain/usecase/infrastructure wiring for the CLI delivery adapter.",
  "tddd": { "enabled": false }
}
```

依存グラフは次の通り。

<!-- illustrative, non-canonical -->
```
apps/cli (bin)        → [cli_composition]
apps/cli-composition  → [domain, infrastructure, usecase]
libs/infrastructure   → [domain, usecase]
libs/usecase          → [domain]
libs/domain           → []
```

`apps/cli-composition` は entry point ごとの最外殻（outermost shell）であり、再利用ライブラリでは
ないため `libs/` ではなく `apps/` に置く。将来 web / gRPC など別の entry point が追加されたときは
別の composition root（例: `apps/web-composition`）を立てる前提で、CLI 専用の性質を名前で明示する
ために `cli-` プレフィックスを付ける。

### D2: 公開 API は `CliApp` facade + `CommandOutcome` 統一戻り値。内部はコマンドグループ別モジュールに委譲し、generic interactor は composition 内部に private に閉じる

<!-- illustrative, non-canonical -->
```rust
pub struct CliApp;
pub struct CommandOutcome {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: u8,
}
impl CliApp {
    pub fn guard_check(&self, command: String) -> Result<CommandOutcome, CompositionError>;
    pub fn domain_export_schema(&self, input: ExportSchemaInput) -> Result<CommandOutcome, CompositionError>;
    // コマンドごとにメソッドを追加。内部実装はグループ別モジュール（review_v2/, track/, guard 等）に委譲。
}
```

`ReviewCycle<R, H, D>` や `ScopeQueryInteractor<D>` など dyn-safe でない generic interactor は
composition 内部に private に閉じ込める。facade の公開面には string / path / primitive / composition
自身が定義する DTO のみを出す。

`CommandOutcome` により、bin は「受け取った outcome を emit するだけ」になる。facade 全体は薄い委譲層に
留め、god-object にならないようコマンドグループ別サブモジュール（`src/review_v2/`, `src/track/`,
`src/guard.rs` 等）に実装を分散する。

### D3: bin は `cli_composition` のみに依存し、`usecase` も `infrastructure` も切る

`apps/cli` の `may_depend_on` を `["cli_composition"]` に変更する。`apps/cli/Cargo.toml` からも
`usecase` と `infrastructure` の依存を削除する。

`libs/usecase/src/lib.rs` は domain identity 型（`TrackId` / `LayerId` 等）を re-export しているが、
これは「CLI が `domain` に直接依存せず `TrackId` 等を構築する」ための意図的な正規手段
である。本 ADR の `CommandOutcome`（string ベース）公開 API では、bin はもはや `TrackId` 等の domain
identity を構築しない。この「CLI のための identity 構築」責務も composition crate 内へ
移る。その結果 bin は usecase の型を一切必要とせず、usecase 依存を完全に切れる。

### D4: `review_v2` の composition logic を infrastructure から cli-composition へ移動し、700 行制限で分割する

`libs/infrastructure/src/review_v2/cli_composition.rs`（1507 行）を
`apps/cli-composition/src/review_v2/` へ移動し、コンテキスト境界に従ってサブモジュールに分割する
（例: `shared.rs` / `scope.rs` / `run.rs` / `approved.rs` / `results.rs` / `briefing.rs` /
`commit_hash.rs` / `mod.rs`）。

移動後、`libs/infrastructure/src/review_v2/mod.rs` は adapter 型（`CodexReviewer`, `ClaudeReviewer`,
`GitDiffGetter`, `SystemReviewHasher`, `FsReviewStore`, `FsCommitHashStore`, `load_v2_scope_config`）
の公開のみ残し、composition function の re-export を止める。

### D5: 移行は 1 トラック内の段階コミット + 最終コミットで enforcement を一括フリップする

re-export shim で CI が緑の状態を維持しながら、コマンドファミリー単位でコードを段階的に移動する。
移行途中の状態（`cli.may_depend_on` がまだ `["cli_composition", "infrastructure", "usecase"]` を含む
中間状態）は main にマージしない。全コマンドの移動が完了した最終コミットで
`architecture-rules.json` / `deny.toml` / `Cargo.toml` を一括更新し、
`cli.may_depend_on = ["cli_composition"]` を固定する。

### D6: enforcement は `architecture-rules.json`（SSoT）/ `deny.toml`（手書き更新）/ `Cargo.toml` の更新で成立する

- `Cargo.toml`（workspace ルート）: workspace members に `"apps/cli-composition"` を追加する。
- `apps/cli/Cargo.toml`: `infrastructure` と `usecase` の依存を削除し `cli_composition` を追加する。
- `apps/cli-composition/Cargo.toml`: `domain` / `usecase` / `infrastructure` + `serde_json` / `thiserror` 等を宣言する。
- `deny.toml`: 手書きで layer policy を更新する（`infrastructure` の `wrappers` を `["cli"]`→`["cli_composition"]`、新規 `cli_composition` の `wrappers=["cli"]` 追加、`usecase` の `wrappers` 更新）。自動生成はされない。
- 検証は 3 系統: `cargo make deny`（cargo-deny が手書き `deny.toml` の wrappers で実依存を ban）/ `bin/sotp verify layers`（runtime で `architecture-rules.json` を読み依存を検証）/ `architecture-rules-verify-sync`（`scripts/architecture_rules.py verify-sync` が `architecture-rules.json` から期待値を計算し、手書き `deny.toml` と `Cargo.toml` workspace members との一致を検証、`cargo make ci` に含まれる）。`scripts/check_layers.py` は存在しない。

## Rejected Alternatives

### A. module 分割（`apps/cli` 内に composition モジュール、新 crate なし）

`apps/cli` 内部に `mod composition` を切るだけで新 crate を作らない案。

却下理由: `deny.toml` とコンパイル時強制は crate 粒度で機能する。module 分割では cli → infra 依存を
実際に遮断できず、将来また infra import が各コマンドの execute 関数に忍び込む。コードの見た目を整える
だけで境界を強制しない。

### B. `libs/` 配置（`libs/composition` 等）

composition root を `libs/composition` として再利用ライブラリ扱いにする案。

却下理由: composition root は entry point ごとの最外殻であり、再利用ライブラリではない。`libs/` は
複数 entry point 間での共有を含意し、composition root の per-entry-point 性に反する。

### C. 公開 API を free function のみ（facade struct なし）

string 受け取り free function を並べるだけで facade struct を持たない案（既存 review_v2 パターンの
一般化）。

却下理由: 関数の namespace が広がるにつれて scale しなくなり、one-off 配線を誘発する。ただし facade
内部の実装手段としては free function を使うことがある（D2 の内部委譲参照）。

### D. 公開 API を `Box<dyn PrimaryPort>` per command で返す

コマンドごとに `Box<dyn PrimaryPort>` を返す案。

却下理由: primary port は usecase 層の trait であり、`ReviewCycle<R, H, D>` /
`ScopeQueryInteractor<D>` が dyn-safe でない。trait object を返すには domain / usecase 型の漏洩か
wrapper trait の追加を強いられる。

### E. bin の usecase 依存を保持する（DTO 用）

`cli.may_depend_on = ["cli_composition", "usecase"]` として、bin が DTO 取得のために usecase を参照
し続ける案。

却下理由: `CommandOutcome` 統一戻り値にすれば bin は usecase の型（DTO も、usecase 経由の identity
構築も）を一切必要としない。usecase 依存を残すと boundary が string + `CommandOutcome` に純化せず、
usecase 経由の identity 構築責務が bin 側に残り続ける。依存を残す利点がないため却下する。

### F. 単一の big-bang コミットで一括移行する

全ファイルを一度に動かして最後に enforcement を固定する案。

却下理由: 巨大 diff で review が困難になり、移行中に CI 緑を保てない。re-export shim を使った段階
コミット（D5）の方が安全で検証可能。ただし移行全体を 1 トラック内に収め、中間状態を main にマージ
しない点は同じである。

## Consequences

### Positive

- cli → infra 依存をコンパイル時に遮断できる（`use infrastructure::…` が bin でコンパイルエラーになる）。
- `review_v2` の非対称が解消し、composition logic が正しい層（`apps/cli-composition`）に集まる。
- composition root が単一の最外殻に集約され、DI 配線の場所が一目でわかる。
- bin が parse + emit のみの極薄になり、テストと理解が容易になる。
- bin が `usecase` も切ることで、bin が `domain` 非依存である状態に加えて bin の依存が型レベルで `cli_composition` のみに閉じ、boundary が string + `CommandOutcome` に純化する。
- 将来 web / gRPC など別の entry point を追加するとき、`apps/web-composition` を別途立てる構造的な
  前提ができる。

### Negative

- 新 crate 追加で workspace のメンバー数が増える。
- `review_v2/cli_composition.rs`（1507 行）を含む大規模なコード移動と 700 行制限に沿った分割が要る。
- `CommandOutcome` が出力を String にバッファする。巨大出力では非効率だが、CLI 用途では許容範囲。
- composition crate が当面 CLI 整形（render 系）を持つことになる（理想的には bin が整形を担うが、
  そのためには bin への DTO 露出 = usecase 依存の復活とのトレードオフがある）。

### Neutral

- enforcement は `architecture-rules.json` / `deny.toml` / `Cargo.toml` の手動更新で成立し、既存の検証機構（`cargo make deny` / `bin/sotp verify layers` / `architecture-rules-verify-sync`）でカバーされる。新しい lint 機構は増えない。

## Reassess When

- web / gRPC など 2 つ目の entry point が追加され、composition root を entry point ごとに分ける前提を
  再評価するとき。
- `render_review_results_str` 等の CLI 整形を bin に戻し、composition が構造化 DTO を返す純化を検討
  するとき（bin への DTO 露出 = usecase 依存復活とのトレードオフがある）。
- bin が `CommandOutcome` で表現しきれない出力（usecase DTO への構造化アクセス等）を必要とし、bin の usecase 依存の再開を検討するとき。
- composition crate 自体が肥大化し、entry point 内でさらに分割が必要になったとき。

## Related

- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` — 本 ADR の直接の前提。Rejected Alternative C と Reassess When で composition root 再設計を「別判断」として明示的に先送りした ADR。
- `knowledge/conventions/hexagonal-architecture.md` — CLI as Composition Root の現行定義。本 ADR の実装後にこの節を更新して composition crate の責務を記述する。
- `knowledge/adr/README.md` — ADR 索引。
