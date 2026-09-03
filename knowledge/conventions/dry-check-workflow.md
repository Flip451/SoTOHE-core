# DRY Check Workflow Convention

## 概要

`sotp dry` コマンド群（`write` / `results` / `check-approved`）と、DFP（DRY fix phase）→
RFP（review fix phase）の 2 フェーズ実行順序のルール。

## 1. ケイパビリティ

`.harness/config/agent-profiles.json` に 2 つの専用 capability が登録されている。

| capability | 役割 | 現在の provider |
|---|---|---|
| `dry-checker` | DRY 違反の判定役（agent） | codex |
| `dry-fix-lead`（dfl） | DFP で DRY 違反のみを修正する修正役 | codex |

`provider` 列は現在の routing であり、受理しうる provider の集合ではない。どちらの
capability も codex と grok の実装経路を持ち、grok は `grok-sandbox` の admission と
model 一致を満たした場合にだけ選べる。それ以外の provider を設定すると fail-closed になる。
可能なセットの一覧は `README.md` の typed-pipeline 節が SSoT である。

> **強制先**: review 観点 — harness-policy scope

`review-fix-lead`（rfl）は review 指摘専用であり、DRY 違反の修正を担わない。
`dry-checker` / `dry-fix-lead` は `reviewer` / `review-fix-lead` とは別 capability であり、
相乗りや混在は禁止する。

> **強制先**: review 観点 — harness-policy scope

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

1. **DFP は RFP より先**: DFP が有効な場合は `sotp dry check-approved` が exit 0 になるまで RFP に入らない。DRY 検査が設定上 skip された場合は、`skipped` を `completed` 相当として Review に進む。
   > **強制先**: review 観点 — full-cycle workflow / harness-policy scope
2. **RFP 後の DFP back-edge**: RFP が `zero_findings` に達した後も、fixpoint を確認するため orchestrator は full-cycle の back-edge として DFP を再実行する。rfl は DRY の判定・修正を担わず、review round の verdict だけを返す。
   > **強制先**: review 観点 — full-cycle workflow / harness-policy scope
3. **fixpoint がコミットゲート**: DRY gate（`sotp dry check-approved` exit 0）と全 review scope の `zero_findings` が同時に green になった時点でのみコミット可。どちらか片方だけでは不十分。
   > **強制先**: 機械 lint — cargo make track-commit-message
4. **DFP は全コードベーススコープ**: 一部の DRY 違反は scope をまたぐため dfl が単一スコープで扱う。rfl の scope（cli / domain / infrastructure 等）に分割しない。
   > **強制先**: review 観点 — full-cycle workflow / harness-policy scope
5. **構造類似を機械的に統合しない**: core 型と adapter mirror DTO / enum の意図的な構造類似は、関心分離に由来するため DRY 違反ではない。知識の重複だけを違反候補とし、偶発的なテキスト類似は違反としない。
   > **強制先**: review 観点 — harness-policy scope
6. **共通化の抽出方向**: 正当な cross-layer 共通化は、関係する両層が依存できるより内側の層へ抽出する。上位層への引き上げで依存方向を逆転してはならない。
   > **強制先**: review 観点 — domain / usecase / infrastructure scope

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
  > **強制先**: 機械 lint — bin/sotp dry write
- dry-check.json への書き込みはこのコマンドのみ。dfl / rfl が直接書き込まない。
  > **強制先**: review 観点 — usecase / infrastructure / cli / cli_composition / harness-policy scope
- `--items-dir` のデフォルトは `track/items`、`--capability-name` のデフォルトは `dry-checker`。`--model` は任意の上書きで、未指定なら `DryCheckServiceFactoryAdapter` が `.harness/config/agent-profiles.json` の `dry-checker` capability から fast / final lane の provider・model・reasoning effort を解決する。解決された provider に応じて `CodexDryChecker` または `GrokDryChecker` を構築し、fast と final で異なる provider も許可する（Grok は sandbox admission と model 一致を満たす場合だけ選択される）。
  > **強制先**: 機械 lint — bin/sotp dry write
- 成功時は exit 0、エラー時は非 0。
  > **強制先**: 機械 lint — bin/sotp dry write

### 出力（stdout）

各 `DryCheckFinding` を表示する：

- `changed_fragment_ref.path()` / `.content_hash().as_str()` — 変更フラグメントのパスとハッシュ（識別子）
  > **強制先**: 機械 lint — bin/sotp dry write
- `candidate_fragment_ref.path()` / `.content_hash().as_str()` — 候補フラグメントのパスとハッシュ（識別子）
  > **強制先**: 機械 lint — bin/sotp dry write
- `refactor_proposal.as_str()` — dfl 向けのリファクタ提案テキスト（必ず非空）
  > **強制先**: 機械 lint — bin/sotp dry write

`DryCheckFinding` の `changed_fragment_ref` / `candidate_fragment_ref` は `FragmentRef`（path + content_hash の識別子ペア）で、agent の JSON 出力には含まれない。選択された provider adapter（`CodexDryChecker` / `GrokDryChecker`）が共通 parser で実際の `CodeFragment` から SHA-256 を計算して `DryCheckFinding` を構築し、provider-neutral な `DryCheckAgentJudgment` を返す。usecase interactor はその judgment を分解して永続化するとともに、coverage と `DryCheckPairKey` 用の FragmentRef を別途導出する。

> **強制先**: 機械 lint — bin/sotp dry write

---

## 4. sotp dry results — 記録の読み出し（情報表示）

```bash
sotp dry results \
  --track-id <track-id> \
  [--filter all|not-a-violation|accepted|violation] \
  [--items-dir <path>]
```

- **情報表示のみ**（informational）。verdict に基づいて exit 1 にはならない。
  > **強制先**: 機械 lint — bin/sotp dry results
- 読み取りエラーのみ非 0。
  > **強制先**: 機械 lint — bin/sotp dry results
- `--filter` のデフォルトは `all`。
  > **強制先**: 機械 lint — bin/sotp dry results

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

> **強制先**: 機械 lint — bin/sotp dry results

`sotp dry results` は現在のゲート状態を返さない。ゲート判定は `sotp dry check-approved` を使う。

> **強制先**: 機械 lint — bin/sotp dry results / bin/sotp dry check-approved

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
  > **強制先**: 機械 lint — bin/sotp dry check-approved
- **exit 非 0**: Blocked（未解決ペアが 1 つでも残っている）
  > **強制先**: 機械 lint — bin/sotp dry check-approved
- `--items-dir` のデフォルトは `track/items`。
  > **強制先**: 機械 lint — bin/sotp dry check-approved

DFP 完了の判定基準。DFP が有効な場合は、RFP に移る前にこのコマンドが exit 0 になることを確認する。設定上 skip の場合は `skipped` を `completed` 相当として扱い、コマンド実行を要求しない。

> **強制先**: 機械 lint — bin/sotp dry check-approved

---

## 6. diff base の解決（write / check-approved 共通）

`sotp dry write` と `sotp dry check-approved` は同一の fail-closed ポリシーで diff base を解決する。

| `FsDryCheckCommitHashStore::read()` の結果 | 動作 |
|---|---|
| `Ok(Some(hash))` — 有効かつ HEAD の祖先 | そのまま base として使用 |
| `Ok(None)` — ファイル不在または非祖先 | `git rev-parse <base_branch>` にフォールバック（effective strategy の `base_branch()`） |
| `Err(Format)` — 不正なハッシュ | `eprintln!` 警告を出してフォールバック（CLI エラーにはしない） |
| `Err(Io)` / `Err(SymlinkDetected)` — その他のストア読み取り失敗 | `eprintln!` 警告を出してフォールバック（CLI エラーにはしない） |

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry check-approved

`--base-commit` が指定された場合はストア参照をスキップし、指定値を直接 base として使用する（任意上書き）。

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry check-approved

`FsDryCheckCommitHashStore`（dry-check 専用）を使用する。review_v2 の `FsCommitHashStore` / `resolve_diff_base` は使わない。

> **強制先**: review 観点 — infrastructure scope

---

## 7. dry-check 専用アダプタの独立性

review_v2 とのアダプタ独立を徹底する。dry-check と review は異なる責務とデータ寿命を持つため、adapter を共有しない。

> **強制先**: review 観点 — infrastructure scope

| dry-check 専用アダプタ | review_v2 の対応するもの（共有禁止） |
|---|---|
| `DryCheckDiffSource` trait（usecase） | `DiffGetter` trait |
| `GitDryCheckDiffGetter`（infra） | `GitDiffGetter`（infra） |
| `FsDryCheckCommitHashStore`（infra） | `FsCommitHashStore`（infra） |

> **強制先**: review 観点 — infrastructure scope

`GitDryCheckDiffGetter` は CLI composition 層でのみ接続する（interactor への注入禁止）。
review_v2 の diff アダプタを dry-check コードからインポートしない。

> **強制先**: review 観点 — cli_composition / usecase / infrastructure scope

---

## 8. (path, content_hash) FragmentRef 識別子設計

### 識別子の構成

各フラグメントの識別子は `(リポジトリ相対パス, content_hash)` のペア（`FragmentRef`）。
`content_hash` はフラグメント内容の SHA-256 ハッシュ（64 文字小文字 hex）。

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry check-approved

### DryCheckPairKey の仕組み

2 つの `FragmentRef` を `(path, content_hash)` の辞書順でソートして `(low, high)` に割り当てた順序不変ペア。
`DryCheckPairKey::new(a, b)` と `DryCheckPairKey::new(b, a)` は同じキーになる。

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry check-approved

### 自己マッチの除外

`path` と `content_hash` の**両方**が一致する場合のみ自己マッチとして除外する。
「パスが同じでハッシュが違う」「パスが違うがハッシュが同じ（別ファイルの完全コピー）」はいずれも有効なペアであり除外しない。

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry check-approved

### 識別子ベースの無効化

フラグメントの内容が変わると `content_hash` が変わり → `FragmentRef` が変わり → `DryCheckPairKey` が変わり → 過去レコードと一致しなくなる → 未記録として再検証される。
ハッシュを別途比較する無効化ステップは不要（識別子マッチングに内包）。

> **強制先**: 機械 lint — bin/sotp dry check-approved

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

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry results

`changed_path` は表示専用フィールドであり、ペア識別子でも無効化判定にも使わない。
自己マッチ（`low_path == high_path` かつ `low_hash == high_hash`）は記録しない。

> **強制先**: 機械 lint — bin/sotp dry write / bin/sotp dry check-approved
