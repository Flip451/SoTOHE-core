---
adr_id: 2026-05-11-2330-catalogue-impl-signals-command-layering
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-v2-catalogue-impl-signals-design:2026-05-12"
    candidate_selection: "from:[no-replacement-for-spec-code-consistency,introduce-catalogue-impl-signals] chose:introduce-catalogue-impl-signals"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:tddd-v2-catalogue-impl-signals-design:2026-05-12"
    candidate_selection: "from:[keep-cli-orchestration,move-to-usecase] chose:move-to-usecase"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:tddd-v2-catalogue-impl-signals-design:2026-05-12"
    candidate_selection: "from:[persisted-view-and-output-mode,stdout-only] chose:stdout-only"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:tddd-v2-catalogue-impl-signals-design:2026-05-12"
    candidate_selection: "from:[new-verify-subcommand,reuse-existing-spec-states-gate] chose:reuse-existing-spec-states-gate"
    status: proposed
---
# 旧 spec-code-consistency の廃止と catalogue-impl-signals 診断コマンドの導入: レイヤー配置・インターフェース・CI ゲート

## Context

### §1 v3 カタログスキーマと S/D/C 3-way 評価

ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` により `schema_version: 3` の `<layer>-types.json` が確定し、ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` (以下「signal evaluator ADR」) が S/D/C 3-way 評価 algorithm と §D3 の 11 領域テーブルを確定した。これらが SoT Chain ③ (カタログ ↔ 実装) の評価基盤となっている。

### §2 旧 spec-code-consistency の役割と廃止

旧 `bin/sotp verify spec-code-consistency` は、v1 カタログスキーマに基づく双方向整合性チェックであり、「カタログに宣言されていない実装が C に存在する (`C\(S∪D)` 相当)」を CI で検出する役割を担っていた。v2 スキーマ移行の作業の中でこのサブコマンドは削除されたが、その CI ゲートとしての役割は引き継がれないままになっていた。

### §3 設計記録とレイヤーの空白

SoT Chain ③ の新しい 3-way 評価に基づく人向け診断コマンドを位置づけた設計記録がなく、そのようなコマンドの orchestration をどこに置くべきかも決定されていなかった。複数レイヤーをまたいで結果を収集・整形する orchestration を自然に置く場所は `libs/usecase/` のインタラクターである。ADR `2026-04-30-0848-cli-via-usecase-only.md` D1 は「cli は domain を直接参照せず usecase を経由する」ことを定めており、orchestration を `apps/cli/` のコマンドハンドラーに置くことはそのレイヤー規則に反し、型カタログへの登録対象にもならず SoT Chain 信号評価から不可視になる。

## Decision

### D1: 旧 spec-code-consistency を廃止し、catalogue-impl-signals 診断コマンドを導入する

`bin/sotp verify spec-code-consistency` (v1 カタログスキーマ向け双方向整合性チェック) を廃止する。SoT Chain ③ (カタログ ↔ 実装) の診断コマンドとして `bin/sotp track catalogue-impl-signals` を導入する。

このコマンドは signal evaluator ADR §D3 が定義した 11 領域テーブル (S∩C / S\C / D∩C / D\C / C\(S∪D) 等) と v3 カタログスキーマに基づき、各エントリーがどの領域に属するか、その領域の signal (🔵/🟡/🔴/skip) と解釈を表示する。「宣言はされているが実装がない」「実装はあるが宣言されていない」といった乖離の原因を調べるための診断ツールとして機能する。使い方はオンデマンドである。

コマンド名 `catalogue-impl-signals` は既存の `*-signals` ファミリー (`type-signals`、`catalogue-spec-signals`) の命名規則 (カバーする SoT Chain 関係で命名する) に従い、SoT Chain ③ (カタログ ↔ 実装) の診断コマンドであることを名前から直接分かるようにしている。

### D2: オーケストレーションは libs/usecase のインタラクターに置く

`catalogue-impl-signals` のオーケストレーション (各レイヤーの A/B/C TypeGraph 取得・signal evaluator 呼び出し・領域別結果の収集・整形) は `libs/usecase/` のインタラクター (use-case サービス) として実装する。`apps/cli/` のコマンドハンドラーはそのインタラクターを呼び出して結果を stdout に書き出す薄いアダプターにとどまる。

これは ADR `2026-04-30-0848-cli-via-usecase-only.md` D1 (cli は domain を直接参照せず usecase を経由する) を本機能に適用したものである。オーケストレーションを `libs/usecase/` に置くことで、この機能が型カタログの登録対象になり、SoT Chain 信号評価から参照できるようになる。

### D3: インターフェースは stdout 専用のオンデマンド形にする

`catalogue-impl-signals` のインターフェースは stdout 専用かつオンデマンドとする。永続化された view ファイルの生成はしない。`--output <path>` ファイル書き出しモードも持たない。`Makefile.toml` のラッパータスクも追加しない。

理由: 領域別内訳は最終 signal の根拠を調べるための診断情報であり、毎回コミットする種類の成果物ではない。各型の最終 signal は既に `<layer>-type-signals.json` に格納され `<layer>-types.md` にレンダーされており、二重管理になる。インターフェースをシンプルに保つことで不要なコミットチャーンを避ける。

### D4: spec-code-consistency の CI ゲート役割は既存の chain-③ ゲートが引き継ぐ (新規 verify サブコマンドは追加しない)

旧 `spec-code-consistency` が担っていた契約違反の検出、具体的には「宣言されていない実装が C に存在する (`C\(S∪D)` 領域)」と「reference / modify 宣言した項目が C から消えている (`S\C` + Reference または Modify 領域)」は、**既存の** chain-③ ゲートが引き継ぐ。新たな `sotp verify` サブコマンドは追加しない。

具体的には、`verify spec-states-current` (コミットゲート、`cargo make ci` に組み込み) と `check_strict_merge_gate` (マージゲート、`track-pr-merge` 経由) が、3-way 評価器が生成した `<layer>-type-signals.json` を読み込み、domain 層の純粋関数 `check_type_signals(doc, strict)` を適用する。

上記 2 つの領域グループはいずれも `<layer>-type-signals.json` に 🔴 エントリとして記録される。`check_type_signals` の Red チェックは `kind_tag` やカタログ宣言有無にかかわらず 🔴 エントリを無条件にカウントする → `Finding::error` となり `strict` の値に関係なくブロックされる。🟡 エントリ (例: `S\C` + Add — 宣言済みの追加項目がまだ実装されていない) はマージゲート (`strict=true`) でブロック、コミットゲート (`strict=false`) では警告として通過する。この strict / interim の分割は ADR `2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2 および §D8.6 に従う。

コードトレースにより、宣言されていない実装が Red チェックをすり抜けるケースはないことを確認した。したがって `catalogue-impl-signals` 専用の verify サブコマンドを追加する必要はない。

関連する ADR:
- `2026-04-12-1200-strict-spec-signal-gate-v2.md` — strict / interim 分割、domain 層の純粋シグナル関数
- `2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` — `<layer>-type-signals.json` ゲートのインフラ、pre-commit での自動再計算
- `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.3 — コミット / マージゲートのマトリクス

## Rejected Alternatives

### A. オーケストレーションを apps/cli に置いたままにする

3-way 評価の orchestration を `apps/cli/` のコマンドハンドラーに置き続ける案。

却下理由: ADR `2026-04-30-0848-cli-via-usecase-only.md` D1 が定めるレイヤー規則に違反する。機能が型カタログに登録されないため SoT Chain 信号評価から不可視のままになる。`apps/cli/` は composition root であるため、この orchestration は hexagonal アーキテクチャの層として位置づけられず、テストや再利用の対象にもなりにくい。

### B. catalogue-impl-signals コマンド自体を導入しない

旧 `spec-code-consistency` を廃止したうえで、人向けの診断コマンドを導入せず既存の chain-③ ゲート (`verify spec-states-current` / マージゲート) だけに頼る案。3-way 領域内訳は signal evaluator の内部実装であり、ユーザー向けコマンドにする必要はないという立場。

却下理由: signal が 🔴/🟡 になった原因を調べるには「どの領域に属するか」と「その解釈」を確認する必要がある。診断コマンドがなければ evaluator のコードを読んで確認するしかなく、デバッグコストが高い。診断コマンドには実用的な価値があるため導入する (D1)。ただしインターフェースは最小限に保つ (D3)、かつ適切なレイヤーに置く (D2)。

### C. chain-③ ゲート用に専用の sotp verify サブコマンドを新設する

`verify spec-states-current` を流用するのではなく、3-way 評価の 🔴 件数を数える `sotp verify catalogue-impl-signals` という CI サブコマンドを新たに追加する案。

却下理由: `verify spec-states-current` が読む `<layer>-type-signals.json` は既に `C\(S∪D)` および `S\C`+Reference/Modify を 🔴 として記録しており、`check_type_signals` の無条件 Red カウントがそれらをすべてカバーしていることをコードトレースで確認した。専用サブコマンドを追加しても coverage は変わらず、evaluator との同期を維持しなければならない対象が増えるだけになる。

## Consequences

### 良い影響

- オーケストレーションが `libs/usecase/` に移ることで型カタログへの登録対象になり、SoT Chain 信号評価から参照できるようになる。
- 旧 `spec-code-consistency` が担っていた chain-③ ゲート役割が既存の gate 機構で引き継がれ、新しい機械を増やさずに整合性ドリフトの自動検出が復活する。

### 悪い影響

- `apps/cli/` にある orchestration を `libs/usecase/` のインタラクターに切り出す実装作業が必要になる。

### 中立

- 3-way 評価ロジック自体は変わらない。signal evaluator ADR がアルゴリズムを定義しており、本 ADR はその orchestration の置き場所・コマンドのインターフェース・CI ゲートを決めるものである。

## Reassess When

- `bin/sotp` 以外の entry point (LSP サーバー、IDE 拡張など) からこのユースケースを呼ぶ必要が生じた場合: インタラクターの public API を安定させるための追加設計が必要になる。
- signal evaluator ADR §D3 の 11 領域テーブルが変わった場合 (新しい action が追加された等): D1 で列挙している領域・signal の記述も更新が必要になる。
- chain-③ ゲートのカバレッジが変わった場合 (例: `check_type_signals` のロジックが変更される): D4 の「既存ゲートがカバーしている」という前提を再確認する必要がある。

## Related

- `knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` — signal evaluator ADR。S/D/C 3-way 評価の algorithm と 11 領域テーブルを定義する。本 ADR はその診断コマンド・レイヤー配置・CI ゲートの設計記録である。
- `knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md` — cli→domain 直接参照禁止と usecase 経由への一本化。本 ADR の D2 はこの ADR D1 を本機能に適用したものである。
- `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` — strict / interim 分割と domain 層の純粋シグナル関数。D4 で参照。
- `knowledge/adr/2026-04-18-1400-tddd-ci-gate-and-signals-separation.md` — `<layer>-type-signals.json` ゲートのインフラと pre-commit 自動再計算。D4 で参照。
- `knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.3 — コミット / マージゲートのマトリクス。D4 で参照。
- `knowledge/adr/README.md` — ADR 索引
