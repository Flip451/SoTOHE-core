# TDDD 信号機評価の CI ゲート接続と宣言/評価結果ファイル分離

## Status

Accepted (2026-04-19)

## Context

Stage 2 (TDDD) シグナル評価は `<layer>-types.json` に宣言と評価結果が同居しており、評価結果は `sotp track type-signals` の手動実行でしか更新されない。CI (`verify-spec-states-current-local`) は**格納済み信号値を読むだけ**で、現在のコードとの乖離は検出できない。

### 問題 1: 手動再計算への依存

開発者が `sotp track type-signals` を実行し忘れると、Red 信号が埋め込まれたコードが commit される。CI は stored signals を正としか見ないため、stale な値で Blue のまま通過するケースがある。事前条件の再計算を**開発者の手動操作に依存しない仕組み**に置き換える必要がある。

### 問題 2: 評価結果が review code_hash を変動させる

`<layer>-types.json` は review の scope 分類で通常の「コード」として扱われるため、評価結果だけ更新した commit でも `code_hash` が変動する。結果として、実装の変更がなくても reviewer の再レビューがトリガーされる。評価結果は「生成物」であり、authored コンテンツとは扱いを分離すべきである。

### 問題 3: 宣言と評価結果の境界が曖昧

1 ファイル同居のため、PR diff では「人間が書いた宣言の変更」と「機械が生成した評価結果の変更」が混ざり、review の論点が拡散する。境界を物理ファイル単位で明確化することで、「何を人間がレビューすべきか」が明示される。

### 先行事例

最も近い先行事例は `strict-signal-gate-v2-2026-04-12` (PR #93, Accepted)。ADR 先行 + fail-closed 真理値表事前確定 + ヘキサゴナル層分離のパターンを踏襲する。`check_type_signals` (strict: bool) は本 ADR の決定を実装する基盤となる。

### Fail-closed 原則

SoTOHE-core は全ゲートで fail-closed を設計原則とする。本 ADR の追加ゲートも以下を遵守する:

- 評価結果ファイルの読取不能 → fail-closed で error
- シンボリックリンク検出 → fail-closed で error (既存 D4.3 ガード流用)
- pre-commit で必要な前提 (toolchain 等) が不在 → fail-closed で commit ブロック

## Decision

### D1: `<layer>-types.json` の分離

`<layer>-types.json` を以下の 2 ファイルに物理分離する:

- **宣言ファイル** `<layer>-types.json`: 人間が書く `type_definitions` のみを保持する。評価結果 (`signals`) フィールドは持たない。
- **評価結果ファイル** `<layer>-type-signals.json` (新規): `sotp track type-signals` の出力のみを保持する。フィールド構成: `schema_version: 1`, `generated_at` (ISO 8601 UTC 生成日時), `declaration_hash` (宣言ファイルのバイト列 SHA-256 hex), `signals` (配列)。宣言ファイルの fingerprint を記録し、stale (乖離) を検出可能にする。

対象は Stage 2 (`<layer>-types.json`) のみ。Stage 1 (`spec.json` の `signals: {blue, yellow, red}` 同居) は本 ADR の scope 外とし、別 ADR で扱う。

### D2: pre-commit 時の自動再計算

`dispatch_track_commit_message()` (`apps/cli/src/commands/make.rs`) の guarded commit パスに、評価結果の自動再計算ステップを追加する。

`/track:commit` 経路の順序:

1. **(新規)** 評価結果の再計算 — active track の全 `tddd.enabled` layer に対し `sotp track type-signals` 相当の処理を実行
2. CI (`cargo make ci`)
3. Review guard (`sotp review check-approved`)
4. Commit from file
5. `.commit_hash` 永続化

再計算を CI より前に置く理由: D5 で追加する stale 検出が `cargo make ci` 内部の `verify_from_spec_json` から呼ばれる。CI より後に再計算を置くと、stale 状態で CI がブロックされてしまい、再計算が走る前に commit フローが中断される。再計算を先に実行することで、CI が評価結果ファイルを検証する時点で常に最新状態が保証され、「pre-commit を通過した commit は stale にならない」という D5 の前提が成立する。

Makefile の `ci-local` dependency に追加しない。その場合 CI 内部での再計算となり、Docker image への nightly toolchain 要求が増える。commit パスはホスト上で走るため、既存の nightly 前提を変えない。

新規の pre-commit 実装では `architecture-rules.json` が不在の場合も fail-closed (commit をブロック) とする。既存の `sotp track type-signals` CLI に存在する legacy fallback (architecture-rules.json 不在時に synthetic domain binding で継続) は本 ADR の fail-closed 原則と相容れないため、pre-commit 配線には適用しない。

### D3: pre-commit 判定ポリシー (Red / Yellow / Blue)

評価結果による commit 可否の切替は以下:

- **Red**: pre-commit で commit をブロックする。fail-closed。Red になった型名を stderr に出力し、`/track:design` で宣言を更新するよう誘導する。`tmp/track-commit/commit-message.txt` は**保持する**(次回再試行の副作用防止)。
- **Yellow**: pre-commit は warning を stderr に出して commit を許可する。merge gate (`check_strict_merge_gate`) は strict=true のため Yellow をブロックする (既存挙動を維持)。
- **Blue**: pre-commit / CI / merge gate すべて通過。

### D4: 評価結果ファイルを review scope から除外

`track/review-scope.json` の `review_operational` に `track/items/<track-id>/*-type-signals.json` を追加する。
これにより評価結果ファイルは review scope 分類で「運用データ」扱いとなり、`SystemReviewHasher` の `code_hash` 計算の対象外となる。

`review_operational` 配列に項目を追加するだけで既存の scope classifier が処理する。`SystemReviewHasher` 側の改造は不要。

### D5: CI / merge gate での stale 検出

`verify_from_spec_json` は宣言ファイルと評価結果ファイルの両方を読み、評価結果ファイルに記録された宣言 fingerprint と現在の宣言ファイルの fingerprint を比較する。

stale (fingerprint 不一致) は **CI interim mode / merge gate のいずれでも常に `VerifyFinding::error`** とする。CI と merge gate での区別を設けない。

理由:

- **D2 の前提と整合**: pre-commit の自動再計算を通過した commit は fingerprint が一致するため stale にならない。CI で stale が発生するのは pre-commit をバイパスした commit、rebase で旧 commit が紛れた場合、意図的に宣言ファイルを手動編集した場合に限られる。いずれも `sotp track type-signals` を 1 コマンド再実行すれば解消するため、warning で通過させる合理性がない。warning 扱いは D2 (手動操作への依存を無くす) を骨抜きにする抜け道になる。
- **Fail-closed 原則との整合**: stale は「評価結果が現状を反映しているか判定できない」状態であり、Context §Fail-closed 原則の「『わからない』場合は必ずブロック」に該当する。
- **Yellow との非対称**: `strict-signal-gate-v2` の Yellow (人間の判断を要する未解決事項) は「interim で warning / strict で error」だが、stale は機械的に解消可能な状態であり性質が異なる。Yellow の warning ポリシーは stale には適用しない。

評価結果ファイルが存在しない (未評価) 場合は、既存の `signals = None` ブランチと同じく「未評価」判定とする (stale とは別の classification)。

### D6: スキーマ移行

`<layer>-types.json` は track ごとに `track/items/<track-id>/` 以下に格納される per-track ファイルである。現在 active な track は本 track (`tddd-ci-gate-and-signals-separation-2026-04-18`) の 1 件のみで、本 track の `/track:design` 実行時に per-track の `<layer>-types.json` (domain/usecase/infrastructure) が作成される。`/track:design` は本 ADR 実装前に実行されるため、当該ファイルは現行仕様 (signals 同居) で書き込まれる。

本 track 自身のマイグレーションは、実装完了後の Migration 手順 5b (コーデック変更 + 評価結果ファイル生成) と `cargo make build-sotp` による新仕様 sotp 切り替え後の初回 `sotp track type-signals` 実行で自動的に行われる。その時点で本 track の `<layer>-types.json` からは signals が剥奪され、`<layer>-type-signals.json` が新規作成される。

なお現在のシステムコーデック (`catalogue_codec.rs`) は `signals` フィールドを `Option` として読み書きしており、系全体としては signals 埋め込み形式が存在する。signals フィールドの除去は Migration 手順 5b で行う。

全 Migration 手順 (1–6) 完了後の定常運用では、新規 track 作成時点から分離された 2 ファイル形式で運用開始する。Migration 手順 2–4 の過渡期 (宣言ファイルと評価結果ファイルの二重書き出し期間) は本 ADR の実装ロールアウト期間中の一時的な状態であり、定常運用の形式ではない。

- 宣言ファイルコーデックから `signals` フィールドを除去する (encode/decode の両方向)
- 評価結果ファイル (`<layer>-type-signals.json`) を新規 schema (`schema_version: 1`) で新設する
- 宣言ファイルの `schema_version` は**bump しない**: 既存 DTO で `signals` は `Option` 扱いのため、コーデック内部変更のみで対応可能

Done / Archived トラックの既存 `<layer>-types.json` には signals が残っている可能性があるが、本 ADR のゲートは active track にしか適用されない (既存の active-track guard と整合) ため、運用上の支障はない。

### D7: symlink guard の拡張

`evaluate_layer_catalogue` (`libs/infrastructure/src/verify/spec_states.rs`) は、宣言ファイルの symlink guard (`reject_symlinks_below`) と同じガードを評価結果ファイルの読み取りパスにも適用する。

評価結果ファイルが symlink の場合 → fail-closed で error。既存の guard ヘルパーを流用し、新規 symlink 判定コードは書かない。

また、pre-commit 再計算パス (`sotp track type-signals` 相当の書き込み前チェック) においても、宣言ファイルおよび評価結果ファイルの書き込みターゲットが symlink でないことを `reject_symlinks_below` で確認してから書き込む。symlink が検出された場合は fail-closed で commit をブロックする。これにより、pre-commit → CI → merge gate のすべての経路で symlink が拒否される。

## Consequences

### Benefits

- Red 信号が埋まった状態での commit が構造的に不可能になる。
- 評価結果だけの変更が review の再レビューをトリガーしなくなる (code_hash が安定)。
- PR diff で「人間が書いた宣言の変更」と「機械が生成した評価結果の変更」が分離される。
- 宣言の stale を CI / merge gate で検出できる (fingerprint による実体比較)。

### Costs

- pre-commit で rustdoc export が追加実行されるため `/track:commit` の所要時間が増える。層ごとに 10-30 秒の見込み。
- pre-commit ステップは nightly toolchain を要求するため、未インストール環境では commit がブロックされる。既存 `sotp track type-signals` の前提を変えないが、fail-closed で commit ブロックされる点は新規の副作用。

### Neutral

- `check_type_signals` の振る舞い (strict / non-strict) は不変。Yellow の merge gate ブロック挙動も不変。
- architecture-rules.json の `tddd.enabled` による opt-in model は不変。

## Migration

1. 新規 domain 型 + codec の実装 (評価結果ファイルの I/O 層)。
2. `sotp track type-signals` CLI が評価結果を新規ファイル `<layer>-type-signals.json` に追加書き出しするように更新する。この時点では宣言ファイルコーデックは変更しないため、宣言ファイル `<layer>-types.json` への signals 書き出しは継続する (二重書き出し過渡期)。実行することで `<layer>-type-signals.json` が生成される。
3. CI 経路 (`verify_from_spec_json` / `evaluate_layer_catalogue`) が評価結果ファイルを読むように更新。評価結果ファイルが存在しない (Missing) は fail-closed error のため、**手順 2 が先行して `<layer>-type-signals.json` を生成した後でなければ merge できない**。本 track 自身も `/track:design` で `<layer>-types.json` を作成済みのため、手順 2 の先行生成制約が本 track にも適用される。
4. `track/review-scope.json` の `review_operational` を更新し、`<layer>-type-signals.json` を `code_hash` 計算対象外とする。
5. 以下を**同一 PR/commit** として merge する (ステップ 5a と 5b を分離すると pre-commit が `<layer>-types.json` を書き換え続け `code_hash` が変動する):
   - 5a. `dispatch_track_commit_message()` に pre-commit 自動再計算ステップを追加。
   - 5b. 宣言ファイルコーデックから `signals` フィールドを削除 (これにより pre-commit が `<layer>-types.json` を書き換えなくなる)。コーデック変更により既存の `declaration_hash` が失効するため、同 PR 内で `sotp track type-signals` を再実行して全評価結果ファイルの `declaration_hash` を再計算してコミットする。
   - 手順 4 (`review_operational` 更新) より後、かつ CI 経路の読取対応 (手順 3) より後に merge すること。
6. ADR を Accepted に昇格。

実装順は上記 1–6 の番号順を基本とする。詳細なタスク分割と依存順序は `/track:design` 完了後に `metadata.json` と `plan.md` で確定する。

## Rejected Alternatives

### R1: CI のみで再計算する (pre-commit を持たない)

CI (`verify-spec-states-current-local`) は検証ゲートであり、ファイル書き込みを行わない設計を維持する。CI 側で signals を書き戻すと「CI を走らせるまで signals が反映されない」ため commit 前の feedback が遅くなる。また CI が書き出すファイルを commit するかどうかで新たな運用論点が発生する。

### R2: 評価結果ファイルを .gitignore する

評価結果ファイルを git 管理から外すと、merge gate (`check_strict_merge_gate`) が `git show origin/<branch>:<path>` で読めなくなる。merge gate の opt-in 意味論が崩れるため不採用。

### R3: Stage 1 (spec.json) も同時に分離する

本 ADR は「CI ゲート接続」を主目的とし、scope を Stage 2 に限定する。Stage 1 は `spec-approve` / `content_hash` / `track-signals` / `track-sync-views` 全体に影響するため、別 ADR で扱う。一貫性のため Stage 1 も将来的に同じパターンで分離予定。

### R4: Makefile.toml の ci-local 依存として pre-commit を実装する

`ci-local` は Docker 内部で走るため、nightly toolchain の追加依存が発生する。また `/track:commit` 以外の CI 経路 (review loop 中の手動 `cargo make ci` など) でも毎回再計算が走り、CI が遅くなる。commit 時のみ再計算する Option A (Rust 実装) を採用する。

### R5: TypeCatalogueDocument を domain 型として 2 aggregate に分離

`check_type_signals` は宣言と評価結果の両方を参照して判定する純粋関数であり、domain 不変条件は結合ビュー上で定義される。分離しても表現力は増えず、signature 変更で全 caller に波及するため YAGNI。I/O 層 (codec + file layout) のみを分離する。

### R6: 宣言ファイルの schema_version を 3 に bump

`signals` フィールドは既存 DTO で `#[serde(skip_serializing_if = "Option::is_none")]`。コーデック側で省略するだけで schema_version の bump は不要。不要な bump は migration コストを増やすのみ。

## Open Questions (解消は実装段階で)

### OQ1: `review_operational` の `<track-id>` placeholder 展開

`track/review-scope.json` の `review_operational` に記載される `<track-id>` placeholder が、review scope loader で正しく current track 向けに展開されるかを、`track/review-scope.json` 更新タスク (Migration 手順 4 に対応するタスク) の実装前に検証する。既存の `track/items/<track-id>/review.json` と同じ展開経路を再利用する前提。タスク ID は `/track:design` 完了後に確定する。

### OQ2: nightly toolchain 不在時の振る舞い (解決済み)

pre-commit で `cargo +nightly rustdoc` を要求するが、nightly が未インストールの環境では commit がブロックされる。本 ADR の Context §Fail-closed 原則 (「pre-commit で必要な前提 (toolchain 等) が不在 → fail-closed で commit ブロック」) と Consequences §Costs に明記された受け入れ済みコストに従い、fail-closed (必ずブロック + インストール手順を提示) とする。実装詳細 (エラーメッセージのフォーマット等) は `/track:design` で確定する。

### OQ3: `declaration_hash` の対象範囲

宣言ファイルの fingerprint は「disk に書き出されたバイト列」を対象とする (エンコード後)。これにより fingerprint は読み直しで安定する。実装詳細は `/track:design` で決定する。

## References

- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` — 本 ADR の直接の前提 (strict gate / Yellow 降格 / ヘキサゴナル層分離)
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — baseline 4 グループ評価
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — multilayer + シグネチャ検証
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — action 宣言
- `knowledge/adr/2026-04-08-1800-reverse-signal-integration.md` — reverse signal 導入
- `knowledge/conventions/source-attribution.md` — Signal 分類ポリシー
- `knowledge/research/2026-04-18-planner-tddd-ci-gate.md` — planner 設計レビュー成果物 (型定義ドラフト含む; 型詳細は `/track:design` で確定)
