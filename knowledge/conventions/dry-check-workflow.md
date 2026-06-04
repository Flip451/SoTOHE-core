# DRY Check Workflow Convention

## 概要

`sotp dry` コマンド群（`write` / `results` / `check-approved`）と、DFP（DRY fix phase）→
RFP（review fix phase）の 2 フェーズ実行順序のルール。

ADR: `knowledge/adr/2026-06-02-0716-dry-checker.md`

---

## 1. ケイパビリティ

`.harness/config/agent-profiles.json` に 2 つの専用 capability が登録されている。

| capability | 役割 | provider |
|---|---|---|
| `dry-checker` | DRY 違反の判定役（agent、`CodexDryChecker` が呼ぶ） | codex |
| `dry-fix-lead`（dfl） | DFP で DRY 違反のみを修正する修正役 | codex |

`review-fix-lead`（rfl）は review 指摘専用であり、DRY 違反の修正を担わない（D12）。
`dry-checker` / `dry-fix-lead` は `reviewer` / `review-fix-lead` とは別 capability であり、
相乗りや混在は禁止（D3）。

---

## 2. DFP → RFP の 2 フェーズ実行順序

### フェーズ概要

```
DFP（DRY fix phase）
  dfl が全コードベースを対象に DRY ゲートを通過するまで回す
    ↓ DFP 通過（sotp dry check-approved が exit 0）
RFP（review fix phase）
  rfl が scope ごとに並列レビューを回す
    ↓ RFP 中に DRY 違反が発生した場合は DFP へ戻る（back-edge）
fixpoint（DRY gate + 全 review scope が同時に green）
  → コミット可
```

### ルール

1. **DFP は RFP より先**: `sotp dry check-approved` が exit 0 になるまで RFP に入らない。
2. **RFP から DFP への back-edge**: RFP 中に DRY 違反が検出された場合、rfl は即座に RFP を止めて DFP に戻る。rfl は DRY 違反を修正しない（D12）。
3. **fixpoint がコミットゲート**: DRY gate（`sotp dry check-approved` exit 0）と全 review scope の `zero_findings` が同時に green になった時点でのみコミット可。どちらか片方だけでは不十分。
4. **DFP は全コードベーススコープ**: DRY は scope をまたぐため dfl が単一スコープで扱う。rfl の scope（cli / domain / infrastructure 等）に分割しない（D13）。

---

## 3. sotp dry write — DRY 検証 & verdict 記録

```bash
sotp dry write \
  --track-id <track-id> \
  [--base-commit <sha>] \
  [--db-path <path>] \
  [--threshold <0.0-1.0>] \
  [--workspace-root <path>] \
  [--items-dir <path>] \
  [--model <model>] \
  [--capability-name <name>]
```

- diff 対象フラグメントを検索して dry-checker agent に判定させ、結果を `dry-check.json` に追記する。
- dry-check.json への書き込みはこのコマンドのみ（D11）。dfl / rfl が直接書き込まない。
- `--items-dir` のデフォルトは `track/items`、`--model` のデフォルトは `codex`、`--capability-name` のデフォルトは `dry-checker`。現行実装は `agent-profiles.json` を読まず、これらの CLI 引数を `CodexDryChecker` に渡す。
- 成功時は exit 0、エラー時は非 0。

### 出力（stdout）

各 `DryCheckFinding` を表示する：

- `changed_fragment_ref.path()` / `.content_hash().as_str()` — 変更フラグメントのパスとハッシュ（識別子）
- `candidate_fragment_ref.path()` / `.content_hash().as_str()` — 候補フラグメントのパスとハッシュ（識別子）
- `refactor_proposal.as_str()` — dfl 向けのリファクタ提案テキスト（必ず非空）

`DryCheckFinding` の `changed_fragment_ref` / `candidate_fragment_ref` は `FragmentRef`（path + content_hash の識別子ペア）で、agent の JSON 出力には含まれない。CLI adapter（`CodexDryChecker`）が実際の `CodeFragment` から SHA-256 を計算して付与する（D8/D11）。

---

## 4. sotp dry results — 記録の読み出し（情報表示）

```bash
sotp dry results \
  --track-id <track-id> \
  [--filter all|not-a-violation|accepted|violation] \
  [--items-dir <path>]
```

- **情報表示のみ**（informational）。verdict に基づいて exit 1 にはならない。
- 読み取りエラーのみ非 0。
- `--filter` のデフォルトは `all`。

### 出力フィールド（レコードごと）

先頭に `dry results: <record-count> record(s)` を表示し、各レコードで以下を表示する。

| フィールド | 説明 |
|---|---|
| `pair_key().low().path()` / `.low().content_hash()` | ペア識別子（低位側）— **識別子** |
| `pair_key().high().path()` / `.high().content_hash()` | ペア識別子（高位側）— **識別子** |
| `changed_path()` | **表示専用**: 記録時の diff フラグメント側パス。識別子にも無効化にも使わない |
| `verdict()` | `not-a-violation` / `accepted` / `violation` |
| `verdict()` の `refactor_proposal` | `violation` レコードのみ保持（`DryCheckVerdict::Violation { refactor_proposal }` の enum 内フィールド） |
| `similarity_score()` | 記録時の類似度スコア（stdout では `score`） |
| `threshold()` | 記録時の判定しきい値（stdout では `threshold`） |
| `base_commit()` | 記録時の diff base commit（stdout では `base`） |
| `rationale()` | agent の判定根拠（全 verdict で必須・非空） |
| `recorded_at()` | 記録日時（ISO-8601 UTC） |

`sotp dry results` は現在のゲート状態を返さない。ゲート判定は `sotp dry check-approved` を使う。

---

## 5. sotp dry check-approved — DRY ゲート（現在のゲート判定）

```bash
sotp dry check-approved \
  --track-id <track-id> \
  [--base-commit <sha>] \
  [--db-path <path>] \
  [--threshold <0.0-1.0>] \
  [--workspace-root <path>] \
  [--items-dir <path>]
```

- **exit 0**: Approved（全 above-threshold 非自己マッチペアが verified かつ `not-a-violation` または `accepted`）
- **exit 非 0**: Blocked（未解決ペアが 1 つでも残っている）
- `--items-dir` のデフォルトは `track/items`。

DFP 完了の判定基準。RFP に移る前に必ずこのコマンドが exit 0 になることを確認する。

---

## 6. diff base の解決（write / check-approved 共通）

`sotp dry write` と `sotp dry check-approved` は同一の fail-closed ポリシーで diff base を解決する。

| `FsDryCheckCommitHashStore::read()` の結果 | 動作 |
|---|---|
| `Ok(Some(hash))` — 有効かつ HEAD の祖先 | そのまま base として使用 |
| `Ok(None)` — ファイル不在または非祖先 | `git rev-parse main` にフォールバック |
| `Err(Format)` — 不正なハッシュ | `eprintln!` 警告を出してフォールバック（CLI エラーにはしない） |
| `Err(Io)` / `Err(SymlinkDetected)` — その他のストア読み取り失敗 | `eprintln!` 警告を出してフォールバック（CLI エラーにはしない） |

`--base-commit` が指定された場合はストア参照をスキップし、指定値を直接 base として使用する（任意上書き）。

**CN-01 遵守**: `FsDryCheckCommitHashStore`（dry-check 専用）を使用する。review_v2 の `FsCommitHashStore` / `resolve_diff_base` は使わない。

---

## 7. CN-01 — dry-check 専用アダプタの独立性

review_v2 とのアダプタ独立を徹底する（D1 の疎結合原則）。

| dry-check 専用アダプタ | review_v2 の対応するもの（共有禁止） |
|---|---|
| `DryCheckDiffSource` trait（usecase） | `DiffGetter` trait |
| `GitDryCheckDiffGetter`（infra） | `GitDiffGetter`（infra） |
| `FsDryCheckCommitHashStore`（infra） | `FsCommitHashStore`（infra） |

`GitDryCheckDiffGetter` は CLI composition 層でのみ接続する（interactor への注入禁止）。
review_v2 の diff アダプタを dry-check コードからインポートしない。

---

## 8. (path, content_hash) FragmentRef 識別子設計

### 識別子の構成

各フラグメントの識別子は `(リポジトリ相対パス, content_hash)` のペア（`FragmentRef`）。
`content_hash` はフラグメント内容の SHA-256 ハッシュ（64 文字小文字 hex）。

### DryCheckPairKey の仕組み

2 つの `FragmentRef` を `(path, content_hash)` の辞書順でソートして `(low, high)` に割り当てた順序不変ペア。
`DryCheckPairKey::new(a, b)` と `DryCheckPairKey::new(b, a)` は同じキーになる（D8/CN-08）。

### 自己マッチの除外

`path` と `content_hash` の**両方**が一致する場合のみ自己マッチとして除外する。
「パスが同じでハッシュが違う」「パスが違うがハッシュが同じ（別ファイルの完全コピー）」はいずれも有効なペアであり除外しない。

### 識別子ベースの無効化（CN-07）

フラグメントの内容が変わると `content_hash` が変わり → `FragmentRef` が変わり → `DryCheckPairKey` が変わり → 過去レコードと一致しなくなる → 未記録として再検証される。
ハッシュを別途比較する無効化ステップは不要（識別子マッチングに内包）。

### on-disk スキーマ（dry-check.json）

ペアは 4 つのフラットフィールドで格納される：

```jsonc
{
  "low_path": "...",   // low 側フラグメントのパス
  "low_hash": "...",   // low 側フラグメントの SHA-256
  "high_path": "...",  // high 側フラグメントのパス
  "high_hash": "..."   // high 側フラグメントの SHA-256
}
```

`changed_path` は表示専用フィールドであり、ペア識別子でも無効化判定にも使わない。
自己マッチ（`low_path == high_path` かつ `low_hash == high_hash`）は記録しない（D9）。
