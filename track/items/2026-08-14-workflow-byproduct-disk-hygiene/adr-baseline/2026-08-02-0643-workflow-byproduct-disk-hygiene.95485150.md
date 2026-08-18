---
adr_id: "2026-08-02-0643-workflow-byproduct-disk-hygiene"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-02; config-extraction hearing 2026-08-09"
    candidate_selection: "from:[A,B,C] chose:C"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-02"
    candidate_selection: "from:[A,B,C] chose:C"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-02"
    candidate_selection: "from:[D,E,F] chose:F"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-02"
    candidate_selection: "from:[D,E,F] chose:F"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-02"
    candidate_selection: "from:[D,E,F] chose:F"
    status: proposed
---
# ワークフロー副産物によるディスク衛生の修繕: scope diff の untracked ディレクトリ計測失敗と template export テストの /tmp scaffold リーク

## Context

2026-08-02 のディスク枯渇調査（`/` 使用率 89%、telemetry 横断分析）で、track ワークフローの副産物に起因する独立した 2 つの不具合を特定した。

**不具合 1: scope diff 計測が untracked ディレクトリで失敗する。**
scope diff 計測（`libs/infrastructure/src/scope_diff_measure.rs`）は untracked ファイルの行数を加算するため `git ls-files --others` で未追跡パスを列挙する。しかし cargo-deny / audit が `.cache/cargo/advisory-dbs/advisory-db-<hash>/` に RustSec advisory DB を git clone すると、git はネストした git リポジトリを再帰せず「末尾 `/` 付きのディレクトリ 1 エントリ」として報告する。このエントリが行数カウント（`count_file_lines`）に渡ると `is not a regular file` の fail-closed エラーとなり、計測全体、ひいては `track transition` が失敗する。

- 計測対象外とすべきパスの除外リスト `IGNORED_NON_REVIEW_PATHS` は `target/` や `.fastembed_cache/` を含むが、`.gitignore` 済みの `.cache/` を含んでいない。
- telemetry 上、このエラーは 2026-07 以降 38 回以上再発し、`track transition` 失敗率 8.3%（luna 移行後期間）の最大要因だった。advisory DB キャッシュは cargo-deny 実行のたびに現れたり消えたりするため、間欠的に再発する。`tmp/advisory-db-*` や `tmp/implementer-*` に同種のネスト repo が現れた変種も観測した。

**不具合 2: template export テストが /tmp に 1.6 GB 級の scaffold を蓄積する。**
template export の binary transplant は「実行中のバイナリ自身」を scaffold の `bin/sotp` として複製する（`current_exe()` 解決）。この設計と、テストが scaffold を `tempfile::tempdir()`（`/tmp/.tmpXXXXXX`）へ書き出す構成が重なり、2 日間で残骸 246 個・約 264 GB が `/tmp` に蓄積してディスク枯渇に至った。

- ユニットテストから in-process で export すると `current_exe()` はワークスペース全体を debuginfo 込みでリンクした約 1.6 GB のテストバイナリになる（本番の `bin/sotp` は 21 MB、`target/debug/sotp` でも約 200 MB）。残骸 183 個の `bin/sotp` のうち 182 個が 1.6 GB 級だった。
- 蓄積経路は 2 系統ある。(a) `apps/cli/tests/consumer_scaffold_host_first.rs` の `static EXPORTED_SCAFFOLD: OnceLock<TempDir>` は static のため Drop が走らず、テストバイナリ実行ごとに確実にリークする。(b) その他のテストの `TempDir` は正常終了時には削除されるが、ハーネスのタイムアウトや中断でテストプロセスが強制終了すると削除されずに残る。5 クローン並列の track ループが `cargo test` を高頻度に実行するため、リークが日次で数十個単位に増幅された。
- `/tmp` は systemd-tmpfiles による自動清掃が動かない環境（WSL2）でも運用されるため、残骸は自然回収されない。

## Decision

### D1: scope diff 計測の除外パス集合を operator 所有 config へ抽出する

ソースにハードコードされた除外配列（`IGNORED_NON_REVIEW_PATHS`、現行 32 エントリ）を廃止し、`.harness/config/` 配下の計測除外 config（ファイル名は実装時に確定）へ移す。

- **mechanism（テンプレート所有）**: config を読み込み、git pathspec（`:(top,exclude)` 群）として untracked 列挙へ適用する機構と、config の fail-closed 検証（不在・不正・空は hard error — 既存の必須 config 群と同じ規律）。
- **policy（operator 所有）**: 除外パターンの実値。出荷既定として現行 32 エントリに加え、`.cache/**`（advisory-db 変種の根)、`tmp/**`・`libs/**/tmp/**`・`apps/**/tmp/**`（repo 内 scratch へのテスト成果物漏れの変種）を同梱する。
- 帰結として、**計測除外の新変種はコード変更なしに config 編集で対応可能**になり、「変種が見つかるたびに実装 track を切る」という再発パターン自体が消える。

（2026-08-09 改稿: 当初案「ハードコード配列へ `.cache/**` を 1 行追加」は、policy をソースに埋める形を温存するため User 裁定で棄却。）

### D2: untracked 列挙のディレクトリエントリを計測対象から除外する

`untracked_paths` の解析で、git が末尾 `/` で報告するエントリ（ネストした git リポジトリ）を untracked 集合から除外する。ネストした git リポジトリがレビュー対象ソースであることはあり得ないため、fail-closed 原則（symlink 等の非通常ファイルをエラーとする既存挙動）は維持したまま、このケースのみを既知の非対象として静かにスキップする。D2 は mechanism 側の構造規則であり config 化しない（「ネスト repo は計測対象になり得ない」は選好ではなく事実であるため）。D1 と D2 は二段構えであり、D1（config の出荷既定）が計測ノイズの主因を塞ぎ、D2 が config 除外の外側にネスト repo が現れた場合の再発を防ぐ。

### D3: template export テストの書き出し先を `CARGO_TARGET_TMPDIR` に変更する

scaffold を書き出すテスト（integration / in-process の両方）は、`tempfile::tempdir()` による `/tmp` 直下ではなく、cargo が統合テスト向けに提供する `CARGO_TARGET_TMPDIR`（`target/tmp/`）配下に一時ディレクトリを作る。取り残しが発生しても `cargo clean` および `sotp maintenance` の清掃対象に含まれ、共有領域 `/tmp` を汚さない。

<!-- illustrative, non-canonical -->
```rust
let export_parent = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
```

### D4: `static OnceLock<TempDir>` によるテスト間共有をやめる

`consumer_scaffold_host_first.rs` の `static EXPORTED_SCAFFOLD: OnceLock<TempDir>` は Drop が実行されない構造的リークであるため廃止する。テスト間で export 結果を共有する場合も、プロセス終了時に確実に削除される方式（または D3 の配置により清掃可能な固定パスの再利用）に置き換える。

### D5: テスト経路の binary transplant はハードリンクで行う

テストが transplant する `bin/sotp` は、フルコピーではなく同一ファイルシステム内のハードリンクとして作成する。テストバイナリ（約 1.6 GB）の複製に伴う書き込み時間と一時ピーク容量を実質ゼロにする。byte-identical 検証（既存テストの assert）はハードリンクでも成立する。本番経路（実 `sotp` からの export）の複製方式は本決定の対象外とし、現行のコピーを維持する。

## Rejected Alternatives

- **A: `.cache/**` の除外追加のみ** — 最小修正だが、`tmp/` 配下など除外リスト外にネストした git リポジトリが現れた変種（観測済み）で再発する。単独では不十分として却下。
- **A': ハードコード配列への逐次追加（本 ADR の当初 D1 案）** — 除外パターンという policy をソース（mechanism）に埋める形を温存し、変種が見つかるたびに実装変更が必要になる。policy と mechanism の分離に反するため却下し、config 抽出（改稿後 D1）を採用（2026-08-09 User 裁定）。
- **B: ディレクトリエントリのスキップのみ** — 再発防止としては汎用的だが、`.cache/` 配下の通常ファイルが引き続き計測対象に含まれ、計測ノイズと読み取りコストが残る。単独では不十分として却下し、D1+D2 の二段構えに含めて採用。
- **C: A+B の二段構え** — 採用（D1, D2）。
- **D: 書き出し先の変更のみ** — リーク自体は残り、`target/tmp/` が同様に肥大する。清掃可能にはなるが根本の取り残しを放置するため却下。
- **E: 書き出し先の変更 + static 共有の廃止** — リーク経路は塞がるが、1.6 GB コピーのビルド時間・一時ピーク容量のコストが残るため却下。
- **F: D+E+ハードリンク化の全部入り** — 採用（D3, D4, D5）。
- **`/tmp` の定期清掃（cron / tmpfiles）だけで対処** — 応急処置としては有効（今回 242 個・約 263 GB を手動回収済み）だが、リークを生む構造が残り、清掃が動かない環境で再発する対症療法のため恒久策としては却下。

## Consequences

**Positive:**

- `track transition` の間欠失敗（telemetry 上の最頻エラー、38 回以上）が解消され、transition 失敗率が低下する。
- テストバイナリ実行ごとの約 1.6 GB のディスク流出が止まり、5 クローン並列運用でのディスク枯渇の再発リスクが除去される。
- 取り残しが発生しても `cargo clean` / `sotp maintenance` で一括回収できる配置になる。
- transplant のハードリンク化により scaffold 系テストの実行時間が短縮される。
- 計測除外が operator 所有 config になり、新変種への対応がコード変更なしの config 編集で閉じる。除外の全量が公開 config として監査可能になる。

**Negative:**

- scope diff の除外リストとディレクトリスキップは「計測しない領域」を広げるため、万一 `.cache/` やネスト repo 内にレビューすべき成果物を置く運用が生まれた場合、計測から漏れる。現行の運用前提（キャッシュ領域にソースを置かない）に依存する。
- `CARGO_TARGET_TMPDIR` は cargo 統合テスト実行時に提供される環境変数であり、テストの起動経路によっては未定義の場合のフォールバック処理が必要になる。
- 除外 config の絞りすぎ（本来計測すべきソースの除外）は検証漏れを機構化する。config の fail-closed 検証は形式検査に留まり、パターンの妥当性自体は operator の責任に残る。

**Neutral:**

- 本番の template export（実 `sotp` バイナリの transplant）の挙動・成果物は変わらない。
- 既存の fail-closed 方針（symlink 等の非通常ファイルをエラーにする挙動）は維持される。

## Reassess When

- scope config が `.cache/` 配下やネストした git リポジトリ内の成果物を計測対象に含める必要が生じたとき。
- git の `ls-files --others` がネストした git リポジトリの報告形式（末尾 `/` エントリ）を変更したとき。
- binary transplant の本番経路の複製方式を変更する（例: 圧縮・strip の導入）とき — テスト経路のハードリンク前提と整合するか再確認する。
- テストハーネスの強制終了ポリシー（タイムアウト・中断）が変わり、一時成果物のリーク特性が変化したとき。
- `CARGO_TARGET_TMPDIR` 相当の仕組みが cargo 側で変更・廃止されたとき。
