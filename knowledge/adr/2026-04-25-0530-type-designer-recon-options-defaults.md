# type-designer reconnaissance のレンダリングオプション既定値 — depth=1+2 + edges=all

## Context

親 ADR (`knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md`) D1 では、reconnaissance step で `bin/sotp track type-graph` を実行することを決定したが、`--cluster-depth` および `--edges` の最適値の選択を「別途調査用トラックで決定する」として先送りにした。

本 ADR はその調査の結果を受けて、reconnaissance step に用いるレンダリングオプションの既定値を確定した。

調査では TDDD 有効化済み 3 層に対し `--cluster-depth 0/1/2` × `--edges methods/impls/all` の全 9 組み合わせを実行した:

- domain: 137 型、depth=0/1 では 50 ノード上限で切り捨てが発生。depth=2 + edges=methods が最適 (kind 選択に関係する辺を捕捉)
- usecase: 45 型、depth=2 では 14 クラスタに過分割 (1 クラスタ 3 型前後、クラスタ間辺なし)。depth=1 + edges=impls が最適 (interactor → application_service のトレイト結線を示す)
- infrastructure: 65 型、depth=1 では切り捨て発生。methods/impls 単独ではアダプタ結線がほぼ空グラフになる。depth=2 + edges=all のみが有用な情報を出す

ただしこれらの「最適値」はワークスペース固有の型数・分布に依存しており、別プロジェクトや将来の型追加で変わりうる。

## Decision

### D1: depth=1 と depth=2 を逐次実行し edges=all を固定で使う

reconnaissance step では、`tddd.enabled` 全層を対象として以下の手順で type-graph を 2 回呼び出す:

1. `bin/sotp track baseline-capture <id>`
2. `bin/sotp track type-graph <id> --cluster-depth 1 --edges all`
3. `bin/sotp track type-graph <id> --cluster-depth 2 --edges all`
4. 各 `tddd.enabled` 層の depth=1 出力 (`<layer>-graph-d1/` 配下) を Read して概観を把握する
5. 各 `tddd.enabled` 層の depth=2 出力 (`<layer>-graph-d2/` 配下) を Read して詳細を把握する
6. catalogue draft に進む

step 4 と step 5 の Read は順序自由 — D2 で決定された depth 値からの自動 suffix によって depth=1 出力と depth=2 出力が独立 path に並存するため、片方の Read を完了する前にもう片方を実行しても出力を取り逃がさない。

**層ごとのオプション固定は行わない**。親 ADR D1 で規定した reconnaissance step の具体的な CLI 呼び出し手順が本 ADR の決定内容であり、type-designer のエージェント定義に反映する。

採用理由:

- **depth=1 (概観) + depth=2 (詳細) の 2 段構成**: 小規模層 (45 型前後) は depth=2 で過分割されるが depth=1 で全体像が見える。大規模層 (137 型以上) は depth=1 で切り捨てが発生するが depth=2 で部分構造が把握できる。どちらの規模にも 2 段構成で対応できる。depth=0 は 50 ノード上限 + "connected only" の制限が誤解を招くため除外する。
- **edges=all の統一採用**: edges=all は methods + fields + impls の和集合であるため、domain (メソッド依存中心)、usecase (トレイト実装中心)、infrastructure (DTO フィールド + アダプタ実装) のいずれにも関連する辺が 1 回のレンダリングで出力される。層ごとに edges を切り替えるとエージェント定義に層知識を埋め込む必要が生じ、別プロジェクトへの移植時に定義を書き換えなければならない。
- **層ごとの固定値を避ける理由**: 型数・kind 分布は実装が進むにつれ変わる。また type-designer のエージェント定義は複数プロジェクトで再利用される。固有の layer 名やサイズを前提としたオプションを定義に埋め込むと、テンプレートの移植性が失われる。

### D2: `bin/sotp track type-graph` の depth 切り替え時自動クリーンアップを bug として修正し、depth ごとの出力を別ディレクトリに並存させる

現在 `bin/sotp track type-graph` は `--cluster-depth` の値を変えると前回の出力を自動消去する:

- depth=0 のフラット出力 (`<layer>-graph.md`) と depth ≥ 1 のディレクトリ出力 (`<layer>-graph/`) が互いに上書きされる
- depth=1 と depth=2 は同じ `<layer>-graph/` ディレクトリに出力されるため、片方を実行するともう片方が消える

D1 では depth=1 と depth=2 の 2 回呼び出しを行う設計を採用した。この設計においては、両呼び出しの間で depth=1 の出力を消去されないよう出力を並存させる仕組みが必要となる。本 D2 が適用されない場合は、depth=1 実行後に depth=2 を実行すると depth=1 の出力が自動消去され、Read のタイミングによっては depth=1 の出力を取り逃がす可能性がある。本 D2 が depth 値からの自動 suffix によってこの問題を解消し、D1 の手順が Read 順序制約なしに実行できるようになる。

本 D2 は CLI 修正を D1 と同じ実装単位で扱うことを決定する。D1 を正しく動作させるための CLI 前提条件であり、CLI 修正を別の実装単位に分離すると D1 の運用が不安定なまま D1 が deploy される構造的不整合が生じる。

### 採用案: 候補 B (depth 値を自動 suffix に使う)

本 D2 では **候補 B (depth 値を自動 suffix に使う)** を採用する。

採用理由:

- **フラグ操作が不要**: CLI 利用者は suffix 名を毎回指定する必要がない。depth の値そのものが出力先を決定するため、reconnaissance 手順の CLI 呼び出しがシンプルになる。
- **depth と出力パスが 1:1 対応する**: depth=1 は常に `<layer>-graph-d1/`、depth=2 は常に `<layer>-graph-d2/` に出力される。出力先が予測可能であり、呼び出し側が suffix 名を管理する必要がない。
- **depth=0 の既存動作を保持する**: フラットモード (`--cluster-depth 0`) の出力ファイルは従来通り `<layer>-graph.md` のままとする (suffix なし)。既存の呼び出し箇所への影響がない。
- **`.gitignore` 変更が 1 行**: クラスタモード用に `track/items/**/*-graph-d*/` を追加するだけで済む (depth=0 のフラット出力は既存パターンが対応済み)。

具体的な仕様:

- クラスタモード (`--cluster-depth ≥ 1`) では出力ディレクトリ名を `<layer>-graph-d<depth>/` に変更する (例: `--cluster-depth 1` → `<layer>-graph-d1/`、`--cluster-depth 2` → `<layer>-graph-d2/`)。depth の値が異なれば出力が独立したパスに並存し、互いに上書きされない
- フラットモード (`--cluster-depth 0`) では出力ファイル名を従来通り `<layer>-graph.md` とする (変更なし)
- CLI フラグの追加はない。`bin/sotp track type-graph` の引数インターフェースは変わらず、出力パス計算の内部実装のみ変更する
- `.gitignore` に `track/items/**/*-graph-d*/` (クラスタモード用ディレクトリ) を追加する (`.gitignore` の編集は実装作業として行う)

本 D2 採用に伴い、`knowledge/strategy/TODO.md` の TDDD-Q07 (「`--out-suffix` 機構の検討」) は本 ADR に内部化される。

### 検討した代替案と却下理由

- **候補 A (`--out-suffix <name>` フラグ追加)**: `--out-suffix d1` / `--out-suffix d2` のように suffix 名を利用者が明示指定する必須フラグを追加する案。reconnaissance 手順では depth と suffix が常に 1:1 対応するため、フラグを明示する意味がない。さらに必須フラグになるため既存の呼び出し箇所すべてに修正が必要となり、修正漏れは実行時 CLI エラーを引き起こす。suffix 名の管理コストに対して得られる自由度が reconnaissance の用途では不要であるため却下。
- **候補 C (`--cluster-depth` を複数値指定可能にする)**: `--cluster-depth 1 2` のような複数値指定で 1 回の invocation で両 depth を出力する案。実装範囲が候補 B より大幅に広く (引数パーサーの変更 + 複数回の型グラフ計算 + 出力先の衝突解決)、本 D2 の本来の scope (D1 の reconnaissance を安定させる) に対して過剰な変更になる。

## Rejected Alternatives

### A. 層ごとにオプションを固定する (domain=depth=2/methods, usecase=depth=1/impls, infrastructure=depth=2/all)

調査で得た層別最適値をエージェント定義に直接組み込む。

**却下理由**: 層名・型数・kind 分布はプロジェクト固有かつ変化する。現ワークスペースの調査結果を定義に固定すると、別プロジェクトへの移植時や型追加時にエージェント定義の書き換えが必要になる。また depth と edges の最適値がそれぞれの層で独立に決まる保証もなく、将来の調査で判断が変わるたびに定義を更新しなければならない。層を問わず depth=1+2/edges=all という汎用既定値を持てば、エージェント定義は層知識を持たずに済む。

### B. depth=2 + edges=all の 1 回呼び出しのみ

呼び出し 2 回のコストを避けるため、1 つの depth を選んで固定する。

**却下理由**: 調査では小規模層 (45 型の usecase) が depth=2 で 14 クラスタに過分割され、クラスタ間辺もほぼゼロになることが確認された。一方、大規模層 (137 型の domain) は depth=1 で 87 型が切り捨てられる。1 つの depth 値で全規模の層をカバーするのは難しい。2 回の呼び出しコストは 1 層あたり数百ミリ秒程度で、型インベントリの全体把握から得られる価値に比べて小さい。

## Consequences

### Positive

- type-designer のエージェント定義が層名・型数を前提としないため、別プロジェクトへの移植が容易になる
- reconnaissance の手順が固定されているため、エージェントが呼び出しオプションを毎回判断する必要がなく、再現性が高い
- edges=all によりドメイン (メソッド依存)、usecase (トレイト結線)、infrastructure (ポート・アダプタ結線) を 1 回のレンダリングで把握できる
- D2 で採用された depth 値からの自動 suffix によって、クラスタモードの depth=1 出力は `<layer>-graph-d1/`、depth=2 出力は `<layer>-graph-d2/` として独立したパスに並存する。D1 の reconnaissance 手順が Read 順序制約なしに実行できるようになり、depth=1 出力の取り逃がしリスクが解消される。フラットモード (depth=0) は `<layer>-graph.md` のまま変わらない
- `bin/sotp track type-graph` の引数インターフェースは変わらないため、既存の呼び出し箇所への修正が不要
- TDDD-Q07 (「`--out-suffix` 機構の検討」) が本 D2 で解消される

### Negative

- `bin/sotp track type-graph` を depth ごとに計 2 回呼ぶため、reconnaissance step の実行時間が増える (1 回数百ミリ秒程度)
- edges=all はクラスタグラフの辺数が増えるため、大規模クラスタでは可読性が下がる場合がある。ただし 50 ノード上限がクラスタ 1 つあたりの辺数増加を抑制する
- 本 D2 採用により Rust CLI への改修が必要になる。これは「Rust ソースコードへの変更を伴わないトラックでも本 ADR を採用できる」という想定を崩すため、本 ADR を実装するトラックでは spec.json 等で対応する実装制約 / 受け入れ基準を追加する必要がある (実装 scope の調整は本 ADR の関心事ではなく、ADR を採用するトラック側の責務)

## Reassess When

- TDDD-Q03 (クロスレイヤー impl 結線の可視化)、TDDD-Q04 (methods と field accessor の意味論的区別) のいずれかが CLI 拡張で解消された場合 — 既定の組み合わせを見直す契機となる (TDDD-Q07 は本 D2 で内部化・解消される)
- type-designer の reconnaissance step が CLI 呼び出し以外の仕組み (例: rustdoc JSON の直接読み込み) に置き換えられた場合 — 本 ADR の前提が変わるため全面的に再評価する
- 別プロジェクトがこのテンプレートを採用し、depth=1+2/edges=all が過不足と判断された場合 — 汎用既定値の再検討またはプロジェクト別上書き機構の導入を検討する

## Related

- `knowledge/adr/2026-04-25-0353-type-designer-reconnaissance-step.md` — 親 ADR。reconnaissance step の導入決定 (D1) を定義し、レンダリングオプションの選択を本 ADR に委ねた
- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — type-graph view の CLI 仕様 (`--cluster-depth`、`--edges` の意味論)
- `knowledge/strategy/TODO.md` — TDDD-Q03 / Q04 (本 ADR が未解決として残す後続課題); TDDD-Q07 は本 ADR D2 に内部化されたため、当該項目は本 ADR への参照に書き換えられる予定
- `.claude/agents/type-designer.md` — reconnaissance step の CLI 呼び出し手順を本 ADR の決定に従って更新するエージェント定義ファイル
