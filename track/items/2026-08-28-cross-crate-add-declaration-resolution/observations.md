# Observations

## 2026-09-02 — delta 規模とローカルレビュー回数

事実記録。評価や是非の判断は含めない。

- 主 ADR `2026-08-28-1034-cross-crate-add-declaration-resolution.md` の決定数は 2（D1、D2）。
- 同一 track の delta ADR `2026-08-29-1803-type-signals-rustdoc-reuse-and-environment-contracts.md` の決定数は 6（D3〜D8）。
- 追加の track-born delta `2026-09-02-0000-evaluation-start-capture-time-bounds.md` の決定数は 1（D1）。
- 1803 D3〜D8 の起点となった review 指摘は `infrastructure final@2026-08-29T18:02:54Z`（一部は続く adr-scope fast@2026-08-29T18:16:16Z と impl-plan final@2026-08-31T02:22:06Z にも記録）。
- 時間上限 delta の起点は usecase final P1。
- track 開始は `2026-08-28T10:51:14Z`。本記録時点は 2026-09-02。
- `review.json` に残るローカルレビュー round 総数は 501。PR 起票後のローカルレビューは約 440 回で、全体の約 9 割。

## 2026-09-02 — PR #257 P1 Accepted Deviation（live rustdoc × A→B→A）

User 裁定 2026-09-02。PR review round 7 P1（`libs/infrastructure/src/tddd/rustdoc_crate_adapter.rs`）を本 track では直さない。

- 指摘: `cargo rustdoc` 実行中にソースが A→B→A と同一バイトへ戻ると、実行前後の fingerprint は一致するのに rustdoc は B または混在を読んでいる可能性がある。
- 論理: D8 は評価入力をメモリ内スナップショットに閉じる。子プロセスの `cargo rustdoc` はライブの workspace を読むため、その区間だけ D8 の保証が届かない。指摘自体は技術的に正しい。
- 対応しない理由の記録:
  1. 発火条件は 1 回の rustdoc 実行中に別プロセスが書き換え、かつ同一バイトへ戻すこと。A→B のまま残る並走は終端 fingerprint で既に fail-closed。観測事例なし。
  2. 帰結は fingerprint A に誤帰属した 1 回の評価結果（同一バイトでは cache 再利用が残り得る）。次の実質バイト変化で fingerprint が外れ、他 identity へ fail-open しない。
  3. 不変スナップショット上での rustdoc 実行は workspace 複製または platform 依存の FS snapshot になり、評価器の責務を超える。別 delta。
  4. 受け皿は 1803 Reassess When 第 3 項（Cargo が artifact 世代を識別する安定 API を提供し、D8 より狭い snapshot 境界を定義できるとき）。
- 記録文: cargo rustdoc はライブの workspace を読むため、1 回の export 実行中に同一バイトへ復帰する A→B→A の書き換えは D8 の前後 fingerprint 照合で検出できない。発火条件は病的で観測事例なし、帰結は次の実質変更で自動無効化される一時的誤帰属に限られる。不変スナップショット上での rustdoc 実行は評価器の責務を超えるため本 track では対応せず、1803 Reassess When 第 3 項（Cargo の世代識別 API）に接続する。
