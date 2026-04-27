<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# TRACK_TRACEABILITY.md §5 を track/workflow.md に merge してから削除する

## Summary

本 track は `TRACK_TRACEABILITY.md §5 (registry.md Update Rules)` の内容を `track/workflow.md` に merge してから `TRACK_TRACEABILITY.md` を削除し、derived 文書の broken link を解消する。Rust コードへの変更はゼロ。
CN-01 により merge (T001) → broken link 修正 + 削除 (T002) → CI 確認 (T003) の順序で実施する。
CN-02 により Tier 0 SoT ファイル (`.claude/rules/` / `.claude/commands/` 等) への残存参照は本 track では変更しない。AC-03 は Tier 1/2 ファイルのみを対象とする。
§5 以外のセクション (§1/§2/§3/§4/§6/§7/§8) は `track/workflow.md` と重複しており固有情報がないため merge 対象外 (OS-01)。

## Tasks (3/3 resolved)

### S001 — §5 merge — track/workflow.md に registry.md 更新ルールを追加 (T001)

> IN-01: `TRACK_TRACEABILITY.md §5` の trigger → required updates テーブルを `track/workflow.md` に追加する。
> `/track:plan` / `/track:commit` / `/track:archive` それぞれのトリガーと必要な registry.md 更新内容が `track/workflow.md` で参照可能になることを確認する (AC-01)。
> CN-03: 逐語コピーではなく `track/workflow.md` の文体・フォーマットに合わせて統合する。既に記載済みのルールは重複追加しない。
> CN-02: Tier 0 SoT ファイルは変更しない。変更対象は `track/workflow.md` のみ。

- [x] **T001**: `TRACK_TRACEABILITY.md §5` の registry.md 更新ルール (trigger → required updates テーブル) を `track/workflow.md` に merge する。追加位置は `## Generated Views` セクションの直前あるいは `## plan.md と metadata.json SSoT` セクション内の適切な位置とし、`track/workflow.md` の文体・スタイルに合わせて統合する。重複するルール (既に `track/workflow.md` に記載済みのもの) は追加しない (CN-03)。Tier 0 SoT ファイルは変更しない (CN-02)。これにより IN-01 / AC-01 を満たす。 (`aae807d`)

### S002 — broken link 修正 + TRACK_TRACEABILITY.md 削除 (T002)

> IN-03: `track/workflow.md` 中の `TRACK_TRACEABILITY.md` を参照する記述を削除または自己参照に変更し、broken link を解消する (AC-03)。
> IN-02: T001 の merge 完了確認後、`TRACK_TRACEABILITY.md` ファイルをリポジトリから削除する (AC-02)。CN-01 に従い必ず merge 後に削除する。
> CN-04: migration shim (redirect stub 等) は作成しない。
> CN-02: Tier 0 SoT ファイル (`.claude/rules/` / `.claude/commands/` 等) の残存参照は本 track では変更しない。

- [x] **T002**: `track/workflow.md` 内に残っている `TRACK_TRACEABILITY.md` への参照 2 箇所 (「`TRACK_TRACEABILITY.md` を参照する」旨の記述) を削除または自己参照に置き換え、broken link を解消する (IN-03 / AC-03)。その後、`TRACK_TRACEABILITY.md` ファイル自体を削除する (IN-02 / AC-02)。T001 の merge 完了後に実施する (CN-01)。migration shim は作成しない (CN-04)。Tier 0 SoT ファイルへの残存参照は変更しない (CN-02)。 (`5e0ab39`)

### S003 — CI gate 確認 (T003)

> T001/T002 完了後に `cargo make ci` を実行し全 gate が pass することを確認する (AC-04)。
> 本 track では Rust ソースコードを一切変更しないため、fmt-check / clippy / test / deny / check-layers は変更前後で差異がない想定。
> verify-* (verify-latest-track / verify-view-freshness 等) が `TRACK_TRACEABILITY.md` 削除により誤検出しないことを確認する。

- [x] **T003**: `cargo make ci` を実行して全 gate が pass することを確認する (AC-04)。Rust コードへの変更はないため fmt-check / clippy / test / deny / check-layers はそのまま通過する想定。`TRACK_TRACEABILITY.md` 削除による verify-* (verify-latest-track / verify-view-freshness 等) への影響がないことを確認し、T001/T002 の変更が clean であることを最終確認する。 (`5e0ab39`)
