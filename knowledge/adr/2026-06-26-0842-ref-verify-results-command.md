---
adr_id: 2026-06-26-0842-ref-verify-results-command
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-18:ref-verify-results-cli-consolidation"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-18:ref-verify-results-ui-and-options"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-18:cache-schema-origin-meta-persist"
    status: proposed
---
# `sotp ref-verify results` で verify-cache 直読みを置き換える

## Context

- SoT Chain の意味論レビューレーンを担う `bin/sotp ref-verify` は現状 `run` と `check-approved` の 2 サブコマンドのみを提供している。
- 一方、SoT Chain とは独立にコード品質を担保している `bin/sotp review` / `bin/sotp dry` はいずれも `results` サブコマンドを持ち、per-scope state summary と round/history を CLI で読める。同じ sotp 配下の verdict-bearing サブコマンドのうち ref-verify だけが対応する公式出口を欠いている。
- `/track:ref-verify` skill (`.claude/commands/track/ref-verify.md` Step 2) は `[BLOCKED]` 時に `spec-adr-verify-cache.json` / `<layer>-catalogue-spec-verify-cache.json` を **手で Read** して failing pair を特定せよと説明している。verify-cache は schema を持つ機械生成ファイルであり、人手で読むのは難しく、誤読リスクがある。
- この非対称を放置すると、ref-verify の結果確認手順が各 caller (writer agent / orchestrator / user) ごとに再発明され、SoT 以外の部分でロジックが重複する。

関連 ADR:

- `knowledge/adr/2026-04-28-1905-review-results-command.md`
- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md`
- `knowledge/adr/2026-06-10-1335-ref-verify-existence-based-scope-resolution.md`

## Decision

### D1: `sotp ref-verify results` の新設による結果確認の CLI 一本化

`bin/sotp ref-verify results` サブコマンドを新設し、ref-verify lane の結果確認を verify-cache JSON 直読みから CLI に一本化する。

- sotp 配下の他の verdict-bearing サブコマンド (`bin/sotp review results` / `bin/sotp dry results`) と同等の CLI 取り出し面を提供する。
- caller (skill / agent briefing / user) は cache ファイルのスキーマを意識せず CLI 出力のみを読めばよい。
- 下流の UI 形態は D2 で定義する。

### D2: 出力面と option を review/dry の results と方針整合させる

`sotp ref-verify results` の出力面と option を `bin/sotp review results` / `bin/sotp dry results` と、棚卸し的に揃えるのではなく「誤読しにくさ」と「ref-verify が担う SoT Chain semantic verification の意味」を保ちながら整合させる。

- default 出力 (human-readable text):
  1. 先頭ブロック: Chain×Layer 集計 (Chain① spec↔ADR / Chain② 各 layer × catalogue↔spec、layer 集合は `architecture-rules.json` から動的解決する) を 1 行/lane で `pass=N fail=N pending=N` 形式で表示する。「今、ref-verify lane がどこで詰まっているか」を `bin/sotp review results` の per-scope state 一覧と同じ読み口で見れるようにする。
  2. レコードブロック: fail/pending pair を `bin/sotp dry results` 調で列挙する (claim_hash / evidence_hash / verdict / reason / chain+layer)。各 pair に **claim/evidence の source** (artifact 種別 + 具体位置: spec.json の section+id / ADR の file+anchor / catalogue+layer の section+entry) を併記する。fail は verify-cache entry に persist された origin meta から source を表示し、pending (cache miss / 未確定 pair) は現在の pair source artifact から再導出した origin meta で source を表示する。writer (spec-designer / type-designer / adr-editor) への振り分けは caller (skill / agent briefing) の責務とし、本コマンドは source 表示までに留める。
  3. 末尾: `Summary: N pass, N fail, N pending, N total` を表示する。
- option:
  - `--track-id` (optional; 省略時は現在のブランチ `track/<id>` から **必ず自動解決する**。手動指定も可) / `--items-dir` (default: `track/items`)
  - `--chain {1|2|all}` (`bin/sotp review results` の `--scope`/`--all` と同趣旨)
  - `--layer <name>|all` (Chain② に対する filter; 有効な layer 名は `architecture-rules.json` から動的解決する)
  - `--filter {pass|fail|pending|all}` (`bin/sotp dry results` の `--filter` と同趣旨)
- exit code: 常に 0 (informational)。verdict gate は `check-approved` の責務 (review/dry results と同じポリシー)。
- `--format json` は導入しない (review/dry も非提供、YAGNI)。
- `bin/sotp review results` の `--limit` / `--round-type` は ref-verify に round 概念がないため採用しない。option 1:1 で棚卸ししない方針を採る。

### D3: verify-cache schema を拡張して claim/evidence の origin meta を persist する

verify-cache の `SemanticVerifyEntry` に `claim_origin` / `evidence_origin` フィールドを追加し、cached pair の source を hash と verdict だけでなく具体位置 (path + anchor + label / section + id / catalogue entry key) として保存する。これにより `sotp ref-verify results` は pass/fail の cached pair について cache entry だけで pair の origin を artifact 種別レベル + 具体位置レベルで出せる。

- Chain① pair: `claim_origin` は spec.json の `(section, requirement id, text label)`、`evidence_origin` は ADR の `(file path, anchor=decision id)`。
- Chain② pair: `claim_origin` は catalogue の `(file path, section key=types|traits|functions, entry key)`、`evidence_origin` は spec.json の `(section, requirement id, text label)`。
- pending (cache miss / 未確定 pair) は verify-cache entry を持たないため origin meta を cache には保存しない。`results` は `ref-verify run` と同じ pair source 解決を read-only で呼び、current artifact から pending pair と origin meta を再導出する。cache に保存する origin meta は「過去に判定済みの pair を、artifact 再 parse なしに読めるようにする」ためのものであり、pending 集計は current pair source と cache の差分で決める。
- 後方互換は持たない。verify-cache は track-local の実行ログであり、`bin/sotp ref-verify run` の再実行で再生成される (track 横断永続資産ではない) ため、旧 cache (origin meta 無し) は parse error として reject する。`#[serde(deny_unknown_fields)]` は維持する。

## Rejected Alternatives

### A. verify-cache 直読みを skill 本文で標準化する

現状の `/track:ref-verify` skill Step 2 (`spec-adr-verify-cache.json` / `<layer>-catalogue-spec-verify-cache.json` を手で Read) を継続し、caller ごとの読み方を skill / briefing template で統一する。

却下理由: SoT ファイル直読みは caller ごとに reimplement されやすく、cache schema 変更時に各所が壊れる。schema 構造を理解せず entry を grep / jq で表層的に拾うような判断が再生産される構造になり、誤読リスクが残る。

### B. `bin/sotp ref-verify run` の出力に詳細を全て統合する

新しい `results` を作らず、`run` の標準出力で per-pair 詳細 + writer 再エントリヒントを表示する。

却下理由: `run` は verify-cache 更新の副作用 + 模型呼び出しを伴う重い run コマンドであり、「詳細を見たいだけ」のために re-run させるのは calibration probe を含めコスト過大。また「run せずに前回結果を読みたい」ユースケース (commit gate の説明 / fix 進捗確認) に応えられない。read-only の `results` を分けるのが review/dry と整合する。

### C. `--format json` を default 面に含める

出力を機械処理しやすいよう JSON 出力を同時に提供する。

却下理由: review/dry results のいずれも JSON 入出力を提供していないが、それで運用上の問題は出ていない (YAGNI)。需要が顕在化した時点で別 ADR で追加してもよい。先掣りして surface area を広げるのは、今後の仕様変更コストを上げる。

## Consequences

### Positive

- ref-verify の結果取り出しが review/dry と対称になり、sotp 配下の verdict-bearing サブコマンド全体の取り扱いが揃う。caller (writer agent / orchestrator / user) の認知負荷が下がる。
- verify-cache schema の進化が CLI 出力に隠蔽され、cache スキーマ変更時に skill / briefing を全部更新するコストが消える。
- cache に origin meta が persist されることで、results は pass/fail の cached pair について cache 読みだけで pair の具体位置 (path + anchor + section + id) を出せる。pending は current pair source を read-only に再解決するが、run の再実行や verifier 呼び出しは要らない。

### Negative

- 新 CLI subcommand + cache 読み出し + origin meta を表示整形するロジックの実装コスト (単体テスト)。
- `/track:ref-verify` skill の Step 2 と各 agent briefing template を `results` 経由へ書き換える作業が伴う。
- verify-cache schema の bump 作業が必要 (schema_version 増分、旧 cache を parse error として reject する単体テスト)。

### Neutral

- exit code 常に 0 ポリシーは `check-approved` との責務分担が前提。`results` を誤って gate に使われないよう help text と caller ドキュメントで明記する必要 (review/dry と同じリスクで、新規リスクではない)。

## Reassess When

- review/dry の results に仕様変更が入り、ref-verify results との対称が崩れたとき (例: JSON 出力が review もしくは dry に導入された / round 概念が全 lane で扱われるようになった)。
- ref-verify lane の chain 数 / layer 数が変わったとき (例: Chain③ 追加 / TDDD 層タクソノミー拡張)。集約 UI と filter option を再評価する。
- verify-cache schema の大幅変更で「pair (claim_hash × evidence_hash)」単位そのものが変わったとき。results の表示単位を見直す。
- verify-cache の origin meta スキーマで取りこぼす情報が顕在化したとき (chain の構造変更 / TDDD タクソノミー拡張で origin の意味が変わったとき)。

## Related

- `knowledge/adr/README.md` — ADR 索引
- `knowledge/adr/2026-04-28-1905-review-results-command.md` — `sotp review results` で review.json 直読みを置き換えた前例
- `knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md` — ref-verify が独立 verdict lane として SoT Chain に位置付けられる根拠
- `knowledge/adr/2026-06-10-1335-ref-verify-existence-based-scope-resolution.md` — ref-verify の scope 解決に関する直近 ADR
- `knowledge/conventions/adr.md` — ADR 規約 (YAML front-matter / lifecycle)
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 配置規約
