---
adr_id: 2026-07-06-1717-template-extraction-boundary
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_17d04bcc-a833-4620-9a0b-55b3c2bde368:2026-07-06"
    candidate_selection: "from:[D1,A,B] chose:D1"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_17d04bcc-a833-4620-9a0b-55b3c2bde368:2026-07-06"
    candidate_selection: "from:[D2,C,D] chose:D2"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_17d04bcc-a833-4620-9a0b-55b3c2bde368:2026-07-06"
    candidate_selection: "from:[D3,G] chose:D3"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_17d04bcc-a833-4620-9a0b-55b3c2bde368:2026-07-06"
    candidate_selection: "from:[D4,E,F] chose:D4"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session_17d04bcc-a833-4620-9a0b-55b3c2bde368:2026-07-06"
    status: proposed
---
# sotp 開発領域と汎用テンプレートの分離境界・切り出し方式

## Context

このリポジトリは二重のアイデンティティを持つ:

1. **sotp 開発ワークスペース** — ワークスペース 6 crate (`libs/domain` / `libs/usecase` /
   `libs/infrastructure` / `apps/cli` / `apps/cli-composition` / `apps/cli-driver`) は sotp CLI
   自体の実装であり、約 141 の ADR、164 の track items、約 18 の archive は sotp 自身の開発履歴である。
2. **汎用 SDD テンプレート** — track ワークフロー、エージェントハーネス (`.claude/` / `.codex/` /
   `.gemini/` / `.agents/` / `.harness/`)、git hooks、CI ハーネス、conventions は任意の Rust
   プロジェクトをホストすべき汎用部分である。

過去の判断 (`knowledge/adr/2026-03-23-2110-sotp-extraction-deferred.md`) では、利用者ゼロ段階での
配布インフラ投資を YAGNI として物理分割 (SPLIT-03/04/05) を延期し、論理境界の文書化 (SPLIT-01) と
`bin/sotp` パス抽象化 (SPLIT-02) のみ「低コストで将来の選択肢を閉じない」として実施可能とした。
本 ADR はその再評価であり、「任意の Rust プロジェクトを扱える汎用テンプレートとして切り出す」ことを
目標に、分離境界と切り出し方式を決定する。

事前調査 (`knowledge/research/2026-07-06-1700-template-extraction-separation-map.md`) の要点:

- sotp はランタイム的にほぼ config-driven 済み。層グラフ・TDDD カタログ・schema export は
  `architecture-rules.json` から解決され、規約パスはすべて解決済み repo root に対する相対結合。
- 最大の結合は「sotp が `apps/cli` としてワークスペース内でソースビルドされること」。約 20 の
  Makefile タスクが `cargo run -p cli --` を呼ぶ。
- sotp ソース内のハードブロッカーは 3 verifier (`domain_purity` / `domain_strings` /
  `usecase_purity`) のみ — `libs/domain/src` 等を定数で持ち、config を迂回して無条件 dispatch される。
- 重量ビルド依存 (vendored conch-parser、lancedb → protoc / MSRV 1.91) は `libs/infrastructure`
  に閉じており、sotp の「ビルド」のみを制約し、プレビルトバイナリの実行には影響しない。
- ハーネスには純粋な sotp-dev ファイルは存在せず、パラメタライズ必要点は機械可読 config
  (`review-scope.json`、catalogue-lint `permitted_layers`) と一部 capability 文書の
  ガードブロック・provenance 参照に集中している。

## Decision

### D1: 分離方式 — in-repo 境界 SSoT + export 成果物方式

SoTOHE-core は sotp 開発リポジトリのまま維持し (自己ホスト開発ループの継続)、リポジトリ内に機械可読な
境界 SSoT を置き、`sotp template export` (または同等の cargo make タスク) が汎用テンプレートツリーを
**ビルド成果物として生成**する。物理的なリポ分割は行わず、export 出力がそのまま将来の物理分割の
内容となる (分割は「export 出力の確定作業」に縮退する)。

### D2: bin/sotp 配布 — cargo install --git 固定タグ方式

切り出されたテンプレートは sotp ソースをワークスペースに同梱せず、bootstrap 時に
`cargo install --git <SoTOHE-core> --tag <pinned> --locked` で固定タグの sotp を bootstrap host 上に
ビルドして `bin/sotp` を導入する。バージョンピンは設定ファイル
(例: `.harness/config/sotp-version.json`) を SSoT とする。D2 の初期方式では sotp のビルド前提
(protoc / sotp の MSRV / vendor patch) はテンプレートの cargo workspace からは分離されるが、
bootstrap host には引き続き必要である。将来 lancedb 系を feature-gate すれば protoc 要件は除去可能
(follow-up、本 ADR では決定しない)。

### D3: 雛形ワークスペース — 6 crate コンパイル可能プレースホルダ

テンプレートは現行標準の 6 crate 構成 (`libs/domain` / `libs/usecase` / `libs/infrastructure` /
`apps/cli` / `apps/cli-composition` / `apps/cli-driver`) を最小のコンパイル可能プレースホルダとして
同梱する。`architecture-rules.json` / `deny.toml` / catalogue-lint 設定と整合し、out-of-the-box で
`cargo make ci` が green であることを要件とする (TDDD の「ビルド可能な cargo workspace」前提を
初期状態で満たす)。利用者は既存の architecture-customizer skill で改名・再構成する。

### D4: 境界 SSoT と export 実装 — manifest + overlay 方式

機械可読 manifest (path → `include` / `exclude` / `overlay` の分類) を境界の SSoT とし、
「テンプレートに必要だが sotp 固有値を含む」ファイル (Makefile.toml / Cargo.toml / deny.toml /
Dockerfile / track/*.md 等) は **template 版を overlay ディレクトリに静置**する。
export = 除外コピー + overlay 上書きのみで、プログラム的なファイル書き換えは行わない。
overlay と実ファイルの drift は「export 出力に対して雛形 CI を回すスモークゲート」で検出する。
これは旧 ADR の SPLIT-01 (論理境界の文書化) を機械可読形で実現するものである。

### D5: 汎用化の前提修正を本決定に含める

分離の成立条件となる以下の前提修正を本 ADR の決定範囲に含める (track 分割は impl-plan に委ねる):

- **D5-a**: 3 verifier (`domain_purity` / `domain_strings` / `usecase_purity`) の
  `architecture-rules.json` 駆動化 (層 role ないし layer entry からの導出、または opt-in 化)。
- **D5-b**: `review-scope.json` の層グループ、catalogue-lint `permitted_layers`、fix-lead 系
  capability のアーキテクチャガードブロック、`workflows/track/init.md` のカタログ参照を
  `architecture-rules.json` から導出ないし整合させる (発見済みの `apps/cli-composition` vs
  `apps/cli` ガードブロック矛盾の解消を含む)。
- **D5-c**: ハーネス文書の sotp 固有 provenance 参照 (特定 ADR ファイル名・sotp ソースパスの cite)
  を generic な記述に置換する。
- **D5-d**: Python 残渣 (`.tool-versions` の python、`__pycache__` / `*.pyc` / `.cache/pytest`
  の gitignore エントリ) の除去。

## Rejected Alternatives

### A. 物理リポ分割を今実施

sotp リポとテンプレートリポを即座に分ける案。配布インフラとバージョン互換管理が今すぐ必要になり、
sotp 自身をテンプレートワークフローで開発する自己ホストループが断絶する。旧 ADR が却下した
理由が依然有効。export 成果物方式なら物理分割を任意のタイミングに繰り延べられる。

### B. ルート入れ替え (sotp subtree 化)

ルートワークスペースを利用者雛形に置き換え、sotp ソースを `sotp/` サブツリーの独立ワークスペースへ
移す案。単一リポのまま主従を反転できるが、既存の track/ADR 履歴・CI・TDDD カタログの参照が大規模に
壊れ、移行コストとリスクが export 方式より大きい。

### C. GitHub Releases プレビルト配布

プラットフォーム matrix のバイナリをリリースする案。利用者のビルド制約 (protoc / MSRV) をゼロに
できるが、リリースインフラ整備が先行投資として必要 (旧 ADR の SPLIT-04/05 相当)。利用者が現れて
から D2 の上に追加する再評価事項とする。

### D. ソース同梱維持

テンプレートに sotp ソースを含めたまま切り出す案。分離目標が達成されず、vendor patch と protoc
要件が全利用者に伝播する。

### E. プログラム的変換 export

export ツールが Makefile.toml / Cargo.toml 等をパースして書き換える案。overlay が不要になるが、
変換ロジックが肥大・脆弱化し、変換結果の検証も別途必要になる。overlay 方式の drift リスクは CI
スモークで検出できるため、単純さを優先する。

### F. manifest のみ (export なし)

SPLIT-01 の境界文書化だけ行い切り出しは手作業とする案。「切り出す」という目標を満たさず、
手作業切り出しは再現不能で drift を検出できない。

### G. 最小 1 crate 雛形 / 雛形なし

雛形を単一 crate に縮小、または空にする案。層アーキテクチャの手本にならず、
`architecture-rules.json` / deny.toml / catalogue-lint のデフォルトと乖離する。雛形なしは初回 CI
が red になり TDDD 前提 (ビルド可能ワークスペース) を満たさない。

## Consequences

### Positive

- 自己ホスト開発ループ (sotp を sotp のワークフローで開発する dogfooding) を維持できる。
- テンプレートが再現可能なビルド成果物になり、手作業切り出しの drift が構造的に排除される。
- 将来の物理分割が「export 出力の確定作業」に縮退し、任意のタイミングで実施可能になる。
- 利用者の cargo workspace は sotp ソース・vendor patch・sotp ビルド制約から分離される。初期方式では
  bootstrap host に protoc / sotp の MSRV が残るが、将来の feature-gate やプレビルト配布でさらに解放できる。

### Negative

- overlay と実ファイルの drift リスクが残る (export 出力への CI スモークゲートで緩和)。
- export ツールと manifest の保守コストが新たに発生する。
- タグリリース運用 (バージョンピンの更新規律) が新たに必要になる。

## Reassess When

- テンプレート利用プロジェクトが複数登場したとき (物理分割・プレビルト配布 (C 案) の再評価)。
- sotp の変更頻度が下がり安定期に入ったとき。
- overlay drift が繰り返し問題化したとき (プログラム的変換 (E 案) への切替検討)。

## Related

- `knowledge/adr/2026-03-23-2110-sotp-extraction-deferred.md` — 本 ADR が再評価した延期決定
- `knowledge/research/2026-07-06-1700-template-extraction-separation-map.md` — 分離境界の事前調査
- `knowledge/conventions/responsibility-boundary.md` — framework enforce 領域と利用者所有領域の分界
- `knowledge/conventions/pre-track-adr-authoring.md` — 本 ADR のライフサイクル規約
