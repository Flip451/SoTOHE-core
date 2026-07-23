---
adr_id: "2026-07-20-1608-disk-footprint-and-dry-feature-gating"
decisions:
  - id: decision-1
    user_decision_ref: "chat_segment:session_01BSF6DqNmoezZpsj5kojZtZ:2026-07-20 /adr:add hearing D1"
    candidate_selection: "from:[feature-gate-default-off,A,B,C] chose:feature-gate-default-off"
    status: proposed
  - id: decision-2
    user_decision_ref: "chat_segment:session_01BSF6DqNmoezZpsj5kojZtZ:2026-07-20 /adr:add hearing D2"
    status: proposed
  - id: decision-3
    user_decision_ref: "chat_segment:session_01BSF6DqNmoezZpsj5kojZtZ:2026-07-20 /adr:add hearing D3"
    status: proposed
  - id: decision-4
    user_decision_ref: "chat_segment:session_01BSF6DqNmoezZpsj5kojZtZ:2026-07-20 /adr:add hearing D4"
    status: proposed
---
# ビルド成果物によるディスク圧迫の解消と dry gate 重量依存の feature flag 化

## Context

定期的に `cargo clean` を実行しないとディスクが圧迫される状態が続いていた。調査の結果、以下の構造的な原因が確認された。

1. **semantic_dup 限定の重量依存が無条件リンクされている**: `libs/infrastructure` は `fastembed`（埋め込みモデル）、`ort`（ONNX Runtime のネイティブライブラリ）、`lancedb`（Lance + DataFusion + Arrow を連れてくる大規模な依存 tree）、`arrow-array` / `arrow-schema` を素の dependencies として宣言している。これらの使用箇所は `semantic_dup/` モジュール（embedding / extractor / index）に閉じているにもかかわらず、すべてのビルドで compile / link される。
2. **同じ重量 tree が二重にビルドされる**: テスト・clippy の dev profile と、sotp binary インストール用の release build の両方で全依存が compile され、`target/` に二系統の成果物が蓄積する。
3. **repo 内ローカルキャッシュは `cargo clean` の対象外**: ビルド環境が `.cache/cargo/registry` / `.cache/cargo/git` / `.cache/sccache` / `.cache/home` を repo 内に作る。crate ソースの複製と sccache のキャッシュがここに貯まり続け、`cargo clean` では消えないため「clean しても戻る占有」になっている。
4. **cargo は旧 artifact を GC しない**: 依存を更新するたびに旧バージョンの成果物が `target/` に残り続ける。1. の重量 tree がこれに掛け算で効く。
5. **runtime は既定 OFF なのに compile は常時 ON**: DRY ゲートは利用者設定で既定無効（opt-in）と決定済みだが（`2026-06-19-2335-dry-gate-configurable-default-off.md`）、compile 側にはその opt-in が反映されておらず、dry gate を使わない利用者も重量依存のビルドコストとディスクコストを払っている。

semantic-dup 検出の導入経緯は `2026-05-29-1118-semantic-dup-detection-discoverability-gate.md` と `2026-06-02-0716-dry-checker.md` に、CI のキャッシュ戦略は `2026-06-01-0336-ci-shorten-cache-strategy-only.md` に記録されている。

## Decision

### D1: semantic-dup の重量依存を cargo feature で gate し、既定 off とする

`libs/infrastructure` の `semantic_dup/` 実装が使う重量依存（`fastembed` / `ort` / `lancedb` / `arrow` 系）を optional dependency にし、cargo feature `semantic-dup` の配下に置く。既定ビルド（default features）ではこれらを compile / link しない。

理由: 使用箇所が `semantic_dup/` モジュールに閉じており、feature 境界として自然に切れる。runtime の dry gate 既定 OFF（opt-in）と方向が揃い、opt-in した利用者だけが重量依存のコストを払う構造になる。

### D2: feature off の binary での dry 系コマンドは fail-closed エラーとする

`semantic-dup` feature 無効でビルドした sotp で dry 系コマンドを実行した場合、サブコマンドは登録されたまま、実行時に「この binary は semantic-dup feature 無効でビルドされている」ことを明示してエラー終了する。silent skip や自動 fallback はしない。

理由: サブコマンド自体を消すと「なぜ無いのか」が利用者に分からず、導線メッセージも出せない。明示エラーは既存の fail-closed 原則とも整合する。

### D3: ローカルキャッシュと target/ の掃除を機構化する

sccache のキャッシュサイズに上限を設定し、`target/` と `.cache/` 配下をまとめて掃除するメンテナンスタスクを新設する。上限値と掃除対象の範囲は hard-code せず、既定値を持つ設定ファイル（`.harness/config/` 配下）経由で利用者が変更できるようにする。

理由: 手動の `cargo clean` に依存した運用は、`cargo clean` が触れない repo 内キャッシュを取りこぼす。掃除の範囲と上限を機構側で持つことで、ディスク占有を有界に保つ。上限や掃除対象は環境ごとに適切な値が異なるため、設定ファイルでの override を機構として持つ（暗黙の hard-code にしない）。

### D4: CI は feature on の検査を維持する

リポジトリの CI 検査（clippy / test）は `semantic-dup` feature を有効にして実行し、feature gate 内のコードの腐敗を防ぐ。sotp binary ビルドの既定は軽量（feature off）とし、dry gate を利用する場合のみ feature on でビルドする。

理由: 既定 off にすると gate 内コードが日常のローカルビルドで compile されなくなるため、CI 側で compile / test を担保しないと壊れたまま気づけない。

## Rejected Alternatives

### A. 別 binary / crate への分離

semantic-dup 機能を `sotp-dry` のような別 binary / 別 crate に切り出す案。却下理由: sotp binary の transplant / install の配布経路が二重化し、provisioning の運用コストが増える。feature flag で同じ軽量化が達成できる。

### B. runtime 既定 OFF のみ（現状維持）

既存の dry gate 既定無効化（runtime opt-in）だけに留める案。却下理由: compile / link と `target/` の肥大は runtime opt-in では解消しない。今回の問題そのものが残る。

### C. 軽量代替への置換

ONNX / lancedb を軽量実装（純 Rust の近似検索など）に置き換える案。却下理由: 検出品質と既存資産を再発明するコストが大きく、feature 化で目的（既定ビルドの軽量化）は達成できる。

## Consequences

### Positive

- 既定ビルドから ONNX Runtime / DataFusion / Arrow が外れ、`target/` の蓄積とビルド時間が減る。
- sccache の上限設定で `.cache/` の成長が有界になる。
- 定期的な手動 `cargo clean` に依存した運用から脱却できる。

### Negative

- dry gate を使うときは feature on での再ビルドが必要になる。両フレーバーの成果物を保持している間は一時的にディスク使用が増える。
- feature の cfg 分岐が増え、保守コストがかかる。
- CI は feature on を維持するため、CI 側のビルド負荷は減らない。

### Neutral

- semantic-dup は compile feature と runtime 設定の二段 opt-in 構造になる。

## Reassess When

- dry gate を既定 ON に転換する決定が出たとき
- 重量依存側が軽量化されたとき（`ort` / `lancedb` の slim 化や代替 crate の登場）
- feature off の fail-closed エラーを踏む頻度が高く、使い勝手の問題になったとき
- 本施策の後もディスク圧迫が解消しない実測が出たとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md` — DRY ゲート既定無効（opt-in）の決定
- `knowledge/adr/2026-05-29-1118-semantic-dup-detection-discoverability-gate.md` — semantic-dup 検出の導入
- `knowledge/adr/2026-06-02-0716-dry-checker.md` — dry-checker capability
- `knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md` — CI キャッシュ戦略の見直し
