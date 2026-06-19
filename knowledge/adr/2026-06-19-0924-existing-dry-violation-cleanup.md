---
adr_id: 2026-06-19-0924-existing-dry-violation-cleanup
decisions:
  - id: D1
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    candidate_selection: "from:[A,B,C,D] chose:none-of-rejected (remediation track)"
    status: proposed
  - id: D2
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
  - id: D3
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
  - id: D4
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
  - id: D5
    user_decision_ref: "chat:/adr:add existing-dry-violation-cleanup hearing (2026-06-19)"
    status: proposed
---
# 既存 DRY 違反の一掃 — 横断・既存重複を正典へ集約する

## Context

DRY ゲート（`sotp dry` / dry-checker）は **PR の diff を embedding 類似度で corpus 照合し、新規の意味的重複をブロックする** diff スコープの予防ゲートである。2026-06-19 に独立した DRY 違反 census（ゲート導入直前 `c4da67a4` と最新 main `9270de33` の before/after 比較）とゲート自身のキャッシュ verdict 評価を実施した結果、以下が判明した:

- **ゲートは既存・横断・データ重複を構造的に取りこぼす**。違反密度は before 0.942 → after 0.842 件/KLoc とわずかに低下したが、その低下は cli / cli-composition のクレート分離クリーンアップ（人手リファクタ）にほぼ全て起因し、**ゲートが新規 diff を統治する成長層（usecase +11%、infrastructure +10%）はむしろ密度が上昇**した。
- ゲートのキャッシュ（16 トラック・4,752 verdict）では violation 273 件を捕捉する一方、**最重要違反である `validate_track_id` の5重複を一度も head-to-head で評価していない**（diff に現れず、層をまたぐコピーは embedding 類似度が低く候補化されないため）。judge の判断品質は高いが、射程がローカル near-clone に限られる。

したがって、ゲートに委ねても解消されない既存重複は、意図的な remediation 作業として別途一掃する必要がある。census が確認した残存重複のうち、本 ADR では影響度の高い 4 クラスタを対象とする。

## Decision

### D1: track-ID / slug 検証を domain の正典に一本化する

`validate_track_id` / slug 検証ロジックが domain（`libs/domain/src/ids.rs` の `is_valid_track_id`）を正典としながら、usecase 3 モジュール（`catalogue_impl_signals` / `type_signals` / `baseline_capture`）+ CLI 2 箇所 + `apps/cli-composition/src/verify.rs` の `validate_track_id_str` に計 5 つ以上の独立実装として散在している。全コピーを削除し、`TrackId::try_new`（domain）への委譲に置き換える。これは cross-layer knowledge-dup であり、文法変更時の乖離バグリスクが最も高い。

### D2: 空/空白禁止の不変条件を NonEmptyString に集約する

「フィールドが空・空白のみであってはならない」という不変条件が、既に `NonEmptyString` 型が存在するにもかかわらず domain の 8 箇所以上（`ids.rs` / `plan.rs` / `spec.rs` / `impl_plan.rs` / `review_v2/types.rs` 等）でインライン再実装されている。各箇所を `NonEmptyString` への委譲に置換し、不変条件の単一定義を回復する。

### D3: review_v2 と dry_check で重複する Codex subprocess 管理と SHA-256→hex を共通化する

`review_v2` と `dry_check` の間で抽出対象とする重複は以下の 2 件に限定する:

- **(1) Codex subprocess 管理**: `spawn_codex` / `drain_pipe` / `tee_stderr_to_file` / ランタイムパスビルダが `libs/infrastructure/src/review_v2/codex_reviewer.rs` と `libs/infrastructure/src/dry_check/codex_dry_checker.rs` の間でバイト単位で重複している。`infrastructure` クレート内の `pub(crate)` 共通モジュールへ抽出する。
- **(4) SHA-256→lowercase-hex**: `infrastructure::dry_check::corpus::sha256_hex` に正典ヘルパが既に存在する。インラインの `format!` 呼び出し箇所をこの正典ヘルパへ委譲する。

以下の 2 件は**現状維持**とし、共通抽出の対象外とする:

- **(2) 排他ロック取得パターン**: `FsDryCheckStore::acquire_write_lock`（`DryCheckWriterError`）と `FsReviewStore` のロック（`ReviewWriterError`）は、**異なる domain port を異なるエラー型で実装するポート固有の並行構造**である。共通抽出にはポートをまたぐ過剰結合の抽象が必要になり、hexagonal 層境界を侵す。これは DRY 違反ではなく、ポート分離を尊重した意図的な並行実装である。
- **(3) 4-source git-diff union**: `GitDiffGetter` は `Vec<FilePath>`（`BTreeSet` 経由）を返し、`GitDryCheckDiffGetter` は `Vec<DiffFileHunks>`（ハンクレベルの `BTreeMap` 経由）を返す。**異なる domain port で出力型が異なる**ため、共通化にはポートをまたぐ抽象が必要になり同様に層境界を侵す。

**hexagonal の層配置は尊重**し、共通化が層境界（domain / usecase / infrastructure）を侵さない形にする。

### D4: test ヘルパ・定数を境界ごとの単一定義へ集約する

**test ヘルパ**（`CwdGuard` / `init_git_repo` / stub bindings）は、**テストコンパイル境界ごとに** 単一の `#[cfg(test)]` 共通 test-support 定義へ集約する — 境界内の冗長コピーを除去する。テストコンパイル境界とは「クレートの src ユニットテスト」と「`tests/common` などの integration-test クレート」の各単位であり、同一ヘルパが integration-test クレートと src ユニットテストの両方で使われる場合は境界ごとに 1 定義持つことが正当である。境界をまたぐ dev-visible / public / workspace dev-dependency な test-support API は作成しない。

**定数**（`POLL_INTERVAL`、`"tmp/reviewer-runtime"` など）は、**アーキテクチャ層境界ごとに** 単一の `const` 定義へ集約する — 同一層内の冗長コピー（`const` 定義の重複および同値インラインリテラル）を除去する。`architecture-rules.json` が禁止する層境界をまたぐ同値定数（例: `cli ↛ infrastructure` の `POLL_INTERVAL`）は**偶然の一致定数として保持**し、DRY 違反と見なさない（D3 のポート固有並行構造の保持と同じ判断）。`cli → infrastructure` 依存は追加しない。

### D5: 進め方 — クラスタ別・小コミットで挙動不変を保証する

各クラスタについて「正典を決める → コピーを正典へ委譲 → `cargo make ci` で挙動不変を確認 → 小さく分割してコミット」の手順を踏む。4 クラスタは互いに独立しているため別タスク/別トラックに分割し、1 コミットあたりの diff を小さく保つ（レビューコストは diff サイズに対し約 O(N^2) で増大するため、guardrails の small-task-commit 方針に従う）。

## Rejected Alternatives

### A. ゲートに任せて何もしない

却下。ゲートは diff スコープかつ embedding 候補ゲートのため、既存重複・cross-layer knowledge-dup・data-dup を構造的に検出しない（キャッシュ評価で `validate_track_id` 5重複が未評価であることを実証）。放置すれば残存し続ける。

### B. ゲートを full-corpus 化して自動修正させてから一掃する

却下（本 ADR の範囲外）。diff 非依存の full-corpus sweep や cross-layer 閾値調整はゲート再発防止として有用だが、ゲート本体の大改修であり、既存重複の即時一掃という目的とは軸が異なる。ゲート拡張は別 ADR で検討し、本 ADR は remediation に限定する。

### C. 全クラスタを 1 トラックで一括大規模リファクタする

却下。レビューコストが diff サイズに対し超線形（約 O(N^2)）で増大し、レビュー往復が膨らむ。クラスタ別・小コミットに分割する（D5）。

### D. 重複を許容し clippy 等の lint 強化だけで対応する

却下。lint は near-clone やセマンティックな cross-layer knowledge-dup を捕捉できず、「どのコピーを正典とし他を委譲させるか」という設計判断を代替しない。定数 data-dup の一部は lint で補完しうるが、それは D4 の補助に留まる。

## Consequences

### Positive

- 変更増幅の解消（例: track-ID 文法変更が正典 1 箇所で完結）と、層をまたぐ乖離バグの予防。
- `NonEmptyString` 等の既存型が本来の用途に回帰し、不変条件が単一の真実源を持つ。
- ゲートの構造的死角（既存・横断・データ重複）を人手で補完し、コードベース全体の DRY 衛生が底上げされる。

### Negative

- 一時的なリファクタ工数とレビュー負荷が発生する。
- 共通化により層間・テスト間の結合がわずかに増える（hexagonal 境界を尊重し、過剰な共通化は避けて最小化する）。ポート固有の並行構造（(2) 排他ロック / (3) git-diff union）は共通化対象外とし、ポート分離を維持する。
- D4 の境界ごとの集約方針により、テストコンパイル境界をまたぐ test ヘルパおよび層境界をまたぐ定数は複数定義が残存する（これは意図的な保持であり、境界内の冗長コピーとは区別される）。

### Neutral

- 本 ADR はゲート本体を変更しない（remediation のみ）。再発防止のためのゲート拡張は別 ADR の関心事とする。

## Reassess When

- 一掃後に DRY 違反 census を再実行し、密度（特に cross-layer / knowledge-dup）が目標水準に低下したことを確認したとき（完了判定）。
- ゲートが full-corpus / cross-layer 検出に拡張され、既存重複を自動検出・修正できるようになったとき（本 ADR の前提が変わる）。
- 共通化が過剰結合を生み hexagonal 境界を侵し始めたとき（揺り戻しの検討）。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/` 配下の dry-checker / DFP⇄RFP 系 ADR — DRY ゲート本体。本 ADR が補完する対象
- `knowledge/conventions/coding-principles.md` — エラーハンドリング / 命名 / モジュール規約
- `knowledge/conventions/prefer-type-safe-abstractions.md` — Newtype / Enum-first パターン（`TrackId` / `NonEmptyString` への集約根拠）
- `knowledge/conventions/dry-check-workflow.md` — DRY ゲートの運用
