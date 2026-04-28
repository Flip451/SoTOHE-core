---
adr_id: 2026-04-28-1905-review-results-command
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-review-results-command:2026-04-29"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-review-results-command:2026-04-29"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-review-results-command:2026-04-29"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add-review-results-command:2026-04-29"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:adr-add-review-results-command:2026-04-29"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:adr-add-review-results-command:2026-04-29"
    status: proposed
---
# `sotp review results` で review.json 直読みを置き換える

## Context

### §1 問題: `review.json` の直読みが横行している

`review.json` は scope 別に `rounds: [{type, verdict, findings, hash, at}, ...]` を持つ SSoT ファイルである。実運用では「直近の round の verdict を scope 別に取りたい」「直近 N round を時系列で見たい」「findings だけ抜きたい」という read パターンが頻出する。

しかし正規 API として提供されているのは以下の 2 つだけ:

- `sotp review status` — 現在の `ReviewState`（Required / NotRequired）を human-readable に表示
- `sotp review check-approved` — approved/blocked の 2 値ゲート（exit code）

「**scope 別に直近 N round の verdict/findings を CLI 一発で取る**」読み口は存在しない（`status` は state 要約のみで round の verdict/findings を返さず、`check-approved` は二値ゲートで詳細を返さない）。結果として:

1. agent prompt / 人間操作ともに `Read review.json` → 目視で JSON を舐める、または
2. `python3 -c 'import json; ...'` / `jq` one-liner で ad hoc 抽出、という回避が**恒常的に発生している**

### §2 直読みが温存してしまう問題

1. **SSoT の内部スキーマに consumer が結合する** — `rounds[].type` / `rounds[].verdict` / `rounds[].hash` のフィールド名に対して agent prompt や人間の手順書が直接依存する。schema を改訂するたびに無数の箇所を追わなければならない
2. **順序仮定が散在する** — 「最後の round が最新」は現状の append-only 実装に依存する暗黙の仮定であり、consumer 側で re-implement されると将来の改修（例: timestamp 降順で並べ替え、ラウンドタイプ別 index の追加）でサイレントに壊れる
3. **Python 依存の逆流** — `.claude/rules/10-guardrails.md` で「`python3 -c` を Bash に埋めるな（`block-direct-git-ops` に引っかかる、ファイルに書けば OK）」と defense-in-depth しているが、そもそも Python one-liner を書く誘因を潰す方が根本対策
4. **verdict の解釈ロジックが二重化する** — codex-local は Rust 側で `ReviewFinalPayload` に shape を揃えた JSON を stdout に吐いているが、review.json の round 配列からの抽出ロジックはこれを経由せず生 JSON を読む。shape が 2 系統に分岐する

`review.json` の扱いは書き込み面では `ReviewWriter` に集約され「直接編集・削除禁止」のルールが `.claude/agents/review-fix-lead.md` の Rules に明文化されている。一方で読み取り面はそのような正規 API を欠いており、書き込み面 (ReviewWriter) と非対称になっている。

### §3 既存 `review status` との関係 — 責務が連続している

`review status` は「現在の `ReviewState` を scope 別に表示」する read コマンドで、本質的には「各 scope の最新 round から導出できる state の view」である。一方 §1 で不足しているのは「最新 round 自体（および過去 N round）の verdict + findings を CLI で直接取る」コマンドで、両者は **同じデータソースに対する window query の長さが違うだけ** の関係にある。

`status` と `results` を別コマンドとして並立させると:

- 同じ `review.json` を 2 箇所の CLI コードパスで load / 解釈する
- JSON 出力 shape が 2 形態に分岐する（state 要約 vs round 履歴）
- 人間と agent で使い分けるコマンドが分岐し学習コストが増える
- commit hint のような横断判定をどちらに載せるか、という意思決定が恒常的に発生する

責務が連続している以上、**1 コマンドに統合し、window 長を flag で制御する** 方が自然。

### §4 副次要望: commit hint を 1 箇所に載せたい

議論スレッドで「全 scope が review 完了 かつ 1 つ以上が zero_findings の状態では commit を早めに打つ推奨メッセージを出したい」という要望があった。理由: 続く編集で hash が stale 化するリスク窓を閉じる。

hint の**本当のターゲットは「hint が必要だが、その認識がない consumer」** である。全 scope が review 通過した瞬間が commit の好機であることを自力で判定できず、続く編集で hash を stale 化させて手戻りを発生させる — これは実運用で繰り返し観測されている既往パターンである。したがって hint は「欲しい人がオプトインで取りに行く情報」ではなく「review 状態を確認するタイミングで自然に目に入り、行動を促すリマインダ」として設計する必要がある。

この観点で配置先を比較する:

- **`codex-local` の stderr に出す案（当初草案）**: review 完了直後に必ず発火するため「リマインダ」としての到達率は最も高い。ただし `codex-local` は「1 scope の review を回す」責務のツールであり、**track 全体の状態を判定する責務を持たない**。hint ロジックをここに置くと `codex-local` が review.json 全体を load する必要が生じ、責務が混ざる。verdict サマリも同様の理由で不要（stdout JSON で事足りる）
- **`review status` / `review results` に出す案（本 ADR 採用）**: review 状態を確認する human / agent は review サイクル中で必ずこのコマンドに触れる（どの scope が終わっているか / 次に何をすべきかの確認）ため、**認識のない consumer でも自然に hint に遭遇する**。`codex-local` の責務分離を保ちつつ、リマインダ性を失わない

本 ADR では後者を採用する。`codex-local` には hint も verdict サマリも追加しない。

## Decision

### D1: `sotp review status` 廃止と `sotp review results` 新設への統合

読み取り専用の正規 API として `sotp review results` を提供し、以下を同時に満たす:

1. 既存 `sotp review status` の機能（scope 別 `ReviewState` の表示）を包含する
2. `review.json` の round 履歴を window query として CLI 経由で取得できる
3. commit hint（全 scope が `NotRequired(*)` 状態 かつ `review.json` が存在する）を同コマンド内で出力する（詳細条件は D5 参照）

既存 `sotp review status` は **削除** する（後方互換のためのエイリアスは置かない）。`sotp review check-approved` は無変更（stateless gate として責務が独立）。

### D2: `sotp review results` のコマンド surface

<!-- illustrative, non-canonical -->
```bash
sotp review results \
    --track-id <id> \
    --items-dir track/items \
    [--scope <name>|--all]              # 既定: --all
    [--limit N|all]                     # 既定: 0(履歴を含めない)
    [--round-type fast|final|any]       # 既定: any(--limit > 0 のときのフィルタ)
    [--no-hint]                         # commit hint を抑制(既定: hint を出す)
```

出力レイヤ:

- **常に出す**: state summary（scope 別の `ReviewState` + 最新 round の verdict を state line に埋め込む）。既定実行（`--limit 0`）は旧 `review status` の置き換え相当
- **opt-in で出す**: `--limit N` (> 0) を指定したとき各 scope の過去 N round を詳細ブロックで追加表示、`--limit all` で全 round

`--limit` は **履歴詳細の深さだけに適用される次元** で、state summary の表示有無には影響しない。この rule を docstring に明示すれば `--limit 0` = "履歴 0 件" で完全に一意（"0 件返す" vs "無制限" の曖昧性は発生しない）。

### D3: 出力フォーマット — text 1 系統、state line + history 詳細 + hint append

text 形式の **1 系統のみ** を提供する。JSON 出力は本 ADR では採用しない（YAGNI — Rejected Alternatives §A 参照）。

旧 `review status` と親和する形式を踏襲しつつ、latest round の verdict と commit hint を append する。

**既定実行（`--limit 0`）— state summary のみ:**

<!-- illustrative, non-canonical -->
```text
Review results (track: tddd-contract-map-phase1-2026-04-17)
Diff base: HEAD~3

  [+] domain          final@2026-04-17T14:41:09Z zero_findings
  [+] usecase         final@2026-04-17T14:42:11Z zero_findings
  [-] infrastructure  fast@2026-04-17T14:29:38Z  findings_remain (2 findings)

Summary: 2 approved, 0 empty, 1 required, 3 total
```

**`--limit N (> 0)` 指定時 — 各 scope の最新 round の findings 詳細 + 過去 round の履歴を展開:**

`--limit > 0` の唯一の価値は **finding の指摘内容そのものを CLI で取れること** なので、message / severity / file:line / category まで展開する。verdict と件数だけなら既定の state line で得られるため、それを再掲するだけでは flag の存在意義がない。

<!-- illustrative, non-canonical -->
```text
Review results (track: tddd-contract-map-phase1-2026-04-17, --limit 2)
Diff base: HEAD~3

  [-] infrastructure  fast@2026-04-17T14:29:38Z  findings_remain (2 findings)
        - [P1 correctness] libs/domain/src/tddd/contract_map_render.rs:156
            `sanitize_id` is not injective over the allowed `LayerId` space:
            valid ids such as `my-gateway` and `my_gateway` both collapse to
            `my_gateway`, so subgraph/node IDs can collide...
        - [P1 correctness] libs/domain/src/tddd/contract_map_render.rs:83
            Edges are resolved by bare short name (first-wins semantics)...
      history (newer first, up to --limit):
        - fast@2026-04-17T13:38:41Z   zero_findings
        - final@2026-04-17T14:33:55Z  findings_remain (1 finding)
            - [P1 infeasibility] libs/domain/src/tddd/contract_map_render.rs:182
                `node_shape` injects `entry.name()` directly into Mermaid
                shape syntax without escaping...

Summary: 2 approved, 0 empty, 1 required, 3 total
```

レイアウト規則:

- 最新 round の findings は state line 直下に indent 6 で展開
- それより古い round は `history (newer first, up to --limit):` 見出しの下に indent 6 でリスト化
- 各 round 内の findings は `- [<severity> <category>] <file>:<line>` 行 + indent 12 の message 本文
- message が長い場合は文単位で wrap（80 列目安）

**commit hint** は条件を満たすときのみ末尾に append:

<!-- illustrative, non-canonical -->
```text
Summary: 3 approved, 0 empty, 0 required, 3 total

[hint] All scopes reviewed (3 zero_findings). Recommend /track:commit now
       to avoid stale review hashes from subsequent edits.
```

### D4: scope universe の完全列挙 (`Other` を含む)

出力の scope 一覧は **`scope_config.all_scope_names()` が返す scope universe を完全に列挙する**（`libs/domain/src/review_v2/scope_config.rs:144-150`）。universe は以下で構成される:

- `track/review-scope.json` の `groups` で宣言された **named scopes**（domain / usecase / infrastructure / cli / harness-policy など）
- **implicit な `Other` scope**: 常に universe に含まれる。named scope にマッチしないファイルがすべて振り分けられる先（`scope_config.rs:117-120`）

各 scope の state は diff と stored verdicts から導出される（`libs/usecase/src/review_v2/cycle.rs::get_review_states`）:

- diff にマッチするファイルがない scope → `NotRequired::Empty`
- diff にマッチし、かつ review.json に entry がない → `Required::NotStarted`
- diff にマッチし、stored verdict が ZeroFindings かつ hash 一致 → `NotRequired::ZeroFindings`
- diff にマッチし、stored verdict が ZeroFindings だが hash 不一致 → `Required::StaleHash`
- diff にマッチし、stored verdict が FindingsRemain → `Required::FindingsRemain`

レビュー未実行や差分なしの scope も省略せず `[-] <scope>: Required(NotStarted)` / `[.] <scope>: NotRequired(Empty)` として表示する。理由:

- scope universe は `scope_config.all_scope_names()` という code 上の SSoT に明示されており、コマンド出力と SSoT が完全一致するべき
- 省略すると consumer 側で full scope set を別途読まないと「ある scope が存在しない」のか「省略された」のか区別できず情報損失になる
- 既存 `review status` の表記と整合する

`Other` が常に implicit に存在するため scope universe が空になるケースは実装上発生しない。ただし全 scope が `NotRequired::Empty` のとき（差分が一切ない、または差分が operational/other_track だけのとき）は state line 群がすべて `[.]` プレフィックスで揃い、`Summary: 0 approved, N empty, 0 required, N total` となる。

### D5: commit hint の発火条件 + check-approved ロジックを domain/usecase に lift

発火条件は既存 `check-approved` の判定ロジックそのものを再利用する。条件は次の 2 つの AND:

1. **`check-approved` の OK 条件成立**: 全 scope が `NotRequired(*)` 状態（= `required.is_empty()`）
2. **bypass パスを通っていない**: `review.json` が存在する（= PR-based workflow による「全 NotStarted + review.json 不在」の bypass 経路ではない、実際にローカルレビューが行われた）

文言例:

<!-- illustrative, non-canonical -->
```text
[hint] All scopes reviewed and approved. Recommend /track:commit now
       to avoid stale review hashes from subsequent edits.
```

`--no-hint` で抑制可能（CI パイプラインや agent 実行時のノイズ回避用）。

#### D5.1 ロジックの位置 — 現状の CLI 層直書きを domain/usecase に lift する

現状 `apps/cli/src/commands/review/mod.rs:206-258` の `run_check_approved` には次のドメイン判定が CLI 層に直書きされている:

- L225 `required.is_empty()` → 「track が approved か」のドメイン判定
- L244-247 `all_not_started && !review_json.exists()` → 「PR-based bypass の妥当性」のドメインルール
- L237-243 `review.json` パス解決 → infrastructure 関心が judgment と混在

この状態のまま `review results` の hint 判定で同じ条件を再実装すると **CLI 2 箇所に同じドメインロジックが重複** し、ヘキサゴナルアーキテクチャ（`.claude/rules/04-coding-principles.md` の Trait-Based Abstraction）に反する。**hint 実装の前提として、判定ロジックを domain/usecase に lift する**:

- **domain 層**: `ReviewApprovalVerdict` 相当の enum を導入（例: `Approved` / `ApprovedWithBypass { not_started_count }` / `Blocked { required_scopes }`）
- **usecase 層**: `ReviewCycle` に `evaluate_approval(reader: &impl ReviewReader, review_json_exists: bool) -> ReviewApprovalVerdict` を追加
- **CLI 層 (`run_check_approved`)**: usecase の戻り値を exit code と eprintln にマップするだけに痩せる
- **CLI 層 (`run_results` hint)**: 同じ usecase 戻り値を解釈して `Approved` のときだけ hint を出す（`ApprovedWithBypass` では出さない）

`review.json` 存在判定の I/O は infrastructure 側に出す（`ReviewStore` または別 port に「ローカル review が記録されているか」を問う method を追加）。

#### D5.2 既知の minor edge case

全 scope が `NotRequired::Empty`（差分が一切ない、または operational/other_track のみ）かつ `review.json` が存在する場合、上記 2 条件が成立して hint が出る。だが diff が空なので `/track:commit` 側の CI/git で弾かれるため、ユーザーへの実害はない。明示的な救済は入れない。

### D6: read 面の責務分離 (`results` / `check-approved` / `codex-local`)

| コマンド | 返すもの | 主な用途 |
| --- | --- | --- |
| `review results` | scope 別の state + 最新 round の verdict（既定 `--limit 0`）/ 過去 N round の詳細（`--limit N` 指定時）+ commit hint | agent / tool / 人間 review の共通読み口（旧 `review status` を包含） |
| `review check-approved` | approved/blocked の 2 値（exit code） | commit guard |

`codex-local` の stdout JSON（1 round shape）は無変更。`codex-local` には verdict サマリも commit hint も追加しない（Context §4 の責務分離参照）。

## Rejected Alternatives

### A. JSON 出力 (`--format json`) を最初から提供する

**却下理由 (YAGNI):** 直読みを排除したい primary consumer は **LLM agent と人間** の 2 つで、両者とも text を直接読める（むしろ LLM は holistic に text を読む方が得意）。機械可読が必要な「approved/blocked の二値ゲート」は既に `review check-approved` が exit code で提供している。

「review history を script で grep / parse したい」という具体的 consumer は現時点で特定できておらず、JSON を出すコストは非自明である:

- DTO 設計 + serde 実装
- 出力 schema versioning（内部 schema との分離管理）
- format negotiation 面（`--format` flag）
- text と JSON の二系統の output path 保守

JSON は **後から追加しても非破壊**（`--format json` flag を後付けで足せる）。具体的な script consumer の必要性が顕在化したタイミングで導入する方が YAGNI 原則に合致する。

### B. `sotp review status` を残し alias で並立させる

**却下理由 (後方互換 alias 不採用):**

- 本プロジェクトは後方互換 alias を置かない方針（新旧コマンドの並立は廃止対象の混乱を長引かせる）
- `status` と `results` が同じ責務を持つ状況は「どちらを使うのが正規か」の判断を consumer に強いる
- `review results`（flag なしの既定実行）が旧 `status` の等価コマンドなので、ドキュメント上でこの対応を明示すれば移行コストは最小

### C. commit hint / verdict サマリを `codex-local` に付加する

**却下理由 (責務混在):**

- `codex-local` は「1 scope の review を回す」責務のツール。track 全体の状態判定を混ぜると review.json 全体を load する副作用が増える
- verdict の stdout JSON が既に機械可読で存在する。人間向けが欲しければ `sotp review results` を明示的に呼ぶ方が正規
- hint ロジックが 2 箇所（codex-local と results）に分岐することを避ける

### D. `review check-approved` も `review results` に統合する

**却下理由 (stateless gate 独立責務):**

- `check-approved` は exit code で approved/blocked を返す **stateless gate** であり、query（窓長・findings 詳細）ではない
- commit guard パスとして `track-commit-message` から呼ばれており、出力形式が変わると破壊的影響が大きい
- 将来 `results --exit-code-on-blocked` のような flag で吸収する選択肢はあるが、本 ADR のスコープ外

### E. commit hint 判定を check-approved とは別ロジックで実装する

**却下理由 (drift リスク / 判断二重化):**

- 別ロジックを起こすと「hint は出るが check-approved は blocked」「hint は出ないが check-approved は OK」という drift が発生し、consumer の信頼を損ねる
- 同じ問い（"今 commit してよいか"）に対して 2 系統の判定実装を持つこと自体が anti-pattern
- 既存ロジックを再利用するには domain/usecase への lift が前提となる（D5.1）。この lift 自体がヘキサゴナル違反の解消という独立した価値を持つ

## Consequences

### Positive

- `review.json` の直読みが agent / 人間手順の両面で規約的に排除できる
- read 面のコマンドが 1 本に統合され、学習コスト / 保守コスト / 意思決定コストが下がる
- text 単一フォーマットで実装範囲が小さく、将来 JSON を additive に追加できる余地を残せる
- review-fix-lead が前回 round との比較で regression 検知できるようになる（将来拡張）
- commit hint により stale hash による手戻りの窓が狭まる
- `codex-local` に横断判定ロジックが流入せず、ツールの責務が保たれる
- approval/bypass 判定が CLI 層から domain/usecase に lift され、ヘキサゴナル違反が解消される。`check-approved` と `review results` の hint が同じ usecase 戻り値を消費するため drift が起こらない

### Negative

- `sotp review status` の削除は破壊的変更。使用箇所（`.claude/`, `knowledge/`, `track/workflow.md`, Makefile.toml wrapper 等）をすべて追う必要がある
- `review results` の flag が増える（status より複雑）。ただし既定実行（flag なし）が旧 `status` 等価なので、単純な state 確認の操作感は変わらない
- approval/bypass 判定の lift（D5.1）は `check-approved` の内部実装変更を伴う。CLI の振る舞い（exit code / eprintln）は維持するが、リファクタリング範囲が広がる
- 正式 ADR 化 → 実装 → agent rule 更新 → 規約浸透まで複数 track に跨る可能性がある

### Neutral

- `review.json` のスキーマ自体は変えない（schema_version 2 のまま）
- `review check-approved` の **CLI surface**（exit code / 引数）は無変更（内部実装は usecase に lift）
- `codex-local` stdout JSON は無変更

## Reassess When

- JSON 出力 (`--format json`) を script で消費したい具体的 consumer が顕在化したとき（YAGNI 判断の前提が崩れた場合）
- `review.json` の schema が改訂されるとき（`schema_version` 2 → 3 のように shape が変わり、round クエリ表現が追随誤りを起こす場合）
- commit hint が false positive / false negative を頻発させ、consumer の信頼を損ねるとき
- `review check-approved` を `review results --exit-code-on-blocked` のように統合する具体的要求が出たとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-04-12-1800-reviewstate-v1-decommission.md` — v1 撤去完了の前提
- `knowledge/adr/2026-04-04-1456-review-system-v2-redesign.md` — v2 (scope-based) データモデル
- `knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md` — scope 別 rounds[] schema の根拠
- `knowledge/adr/2026-03-25-2125-review-json-separation-of-concerns.md` — review.json SSoT / metadata.json 分離の原則
- `knowledge/conventions/adr.md` — ADR 運用ルール
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR lifecycle
