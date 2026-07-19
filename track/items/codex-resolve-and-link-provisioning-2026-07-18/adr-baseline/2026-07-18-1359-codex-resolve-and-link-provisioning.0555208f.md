---
adr_id: 2026-07-18-1359-codex-resolve-and-link-provisioning
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01NodsFzNiEpN7j92TMfQodu:2026-07-18 全テンプレート利用者にとって一番負荷のない方法で解決し、ソースコードに asdf 等の toolchain-manager 依存を持ち込まず、かつ実バイナリの複製固定ではなくリンク保持で利用者の最新版に追従できる形とする裁定"
    candidate_selection: "from:[bootstrap-resolve-and-link,bootstrap-pinned-download,in-binary-manager-resolution,makefile-cascade,sanitization-redesign,document-codex-bin-only] chose:bootstrap-resolve-and-link"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01NodsFzNiEpN7j92TMfQodu:2026-07-18 入れ子 reviewer（fixer 内の nested codex）でも同一に成立する解決規約を求め、repo-local リンク最優先 + PATH fallback とする裁定"
    candidate_selection: "from:[repo-local-first-then-path,path-only,repo-local-only] chose:repo-local-first-then-path"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01NodsFzNiEpN7j92TMfQodu:2026-07-18 利用者に環境変数設定を要求しない方針（ref-verify で確立済みの「env var 解決はアンチパターン」制約の一般化）と、dry-fix runner に残る asdf 結合を 2026-06-01 の toolchain-manager-agnostic 決定への逸脱として削除する裁定"
    candidate_selection: "from:[remove-codex-bin-and-manager-code,keep-codex-bin-escape-hatch,keep-dry-fix-asdf] chose:remove-codex-bin-and-manager-code"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01NodsFzNiEpN7j92TMfQodu:2026-07-18 exit 126 が「fix runner did not report a completion status」に握りつぶされ原因追跡が困難だった実障害を受け、失敗（dangling link を含む）の観測可能性を要件化する裁定"
    candidate_selection: "from:[exit-code-plus-log-path-plus-provisioning-hint,generic-error-status-quo] chose:exit-code-plus-log-path-plus-provisioning-hint"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-01NodsFzNiEpN7j92TMfQodu:2026-07-18 初版は Linux の実障害解消に集中し、他プラットフォーム対応・バージョン再現性（pin モード）は再評価事項へ送る裁定"
    candidate_selection: "from:[linux-first,all-platforms-first,pin-mode-included] chose:linux-first"
    status: proposed
---
# codex reviewer runtime の bootstrap 解決リンク（resolve & link）配備

## Context

sotp は codex CLI を子プロセスとして起動する。fixer 系 2 経路
（`review_v2/review_fix_runner`、`dry_check/dry_fix_local`）は credential isolation のため
サニタイズ環境（unique safe HOME + 環境変数 allowlist、`GITHUB_TOKEN`/SSH 系の遮断、
`GIT_SSH_COMMAND=/bin/false`）で spawn する。HOME 差し替えは `~/.config/gh` の OAuth
トークン等を既定探索経路から外す実質的な isolation 層であり、撤去できない。

toolchain-manager の shim（例: asdf の `~/.asdf/shims/codex` — bash script → `asdf exec` →
node launcher → 実体）は実 HOME と PATH に依存するため、サニタイズ環境では exit 126・
出力なしで死ぬ。`command -v` / `which` は shim そのものを返すだけで実体に貫通できない。
さらに review-fix は**入れ子構造**を持つ（ADR
`2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md`: 外側 fixer の中で
`sotp review local` → 内側 reviewer の `codex exec` を起動）ため、内側の codex 解決も
サニタイズ済み HOME の下で走る。PATH/shim ベースの解決は入れ子で二重に脆い。

この問題への対処は歴史的に反復されてきた:

- commit `8a981fe`（2026-06-01）は review fixer に実装されていた asdf 対応（env 注入・
  `asdf which` 解決・`.tool-versions` コピー）を「SoTOHE is a template for arbitrary
  adopters; the infrastructure layer must not couple to one developer's machine setup」として
  **意図的に全削除**し、「実体パスは外部（`CODEX_BIN` env var）から注入する」契約に置き換えた。
  開発機側の注入実装としてソース `Makefile.toml` に
  `CODEX_BIN="${CODEX_BIN:-$(asdf which codex 2>/dev/null || command -v codex)}"` が置かれた。
- その後の dry-fix wrapper 導入（`track-local-dry-fix` / dfl）は `dry_fix_local/env.rs` に
  in-binary の `asdf which` 解決（`resolve_codex_via_asdf`）を**再持ち込み**した。これは
  2026-06-01 決定からの逸脱であり、経路間の非対称も生んだ。
- ADR `2026-07-16-1438-consumer-scaffold-host-first-makefile.md` は配布物
  `overlay/Makefile.toml` から `asdf which` 参照を「開発者個人の環境依存」として撤去し、
  `command -v codex` のみを残した。原則としては正しいが、結果として「shim 利用者は
  `CODEX_BIN` を自分で設定する」という**配布物に文書が存在しない暗黙契約**が生まれた。
- 帰結として、SoTOHE-core-003 からの export
  （`SoTOHE-core-003-export-2026-07-18`、track `mini-repomix-2026-07-18`）の初回レビューで、
  npm + asdf という codex の標準的導入形態の利用者が review fixer の exit 126 に遭遇した。
  エラーは「fix runner did not report a completion status」に集約され、exit code も原因も
  表示されなかった。

整理すると、既確立の 2 原則 — (1) インフラ層・配布物に toolchain-manager 知識を持ち込まない
（`8a981fe`、ADR 1438、`ref_verify/process_runner.rs` の「env var 解決はアンチパターン、
OS PATH へ委譲」制約）、(2) 利用者に設定・文書参照・環境変更を要求しない — を同時に満たす
解決が存在しなかった。本 ADR は「bootstrap が通常環境で実体を解決・検証し、repo-local に
**リンク**として保持する」ことで両立させる。実バイナリの複製・バージョン固定は行わず、
利用者が自分のチャネル（npm 等）で codex を更新すれば、同一パスへの上書き更新である限り
リンク経由で自動追従する。codex は auth もバージョンも利用者所有の外部サービスクライアント
であり、reviewer の挙動を主に規定するモデル・reasoning effort は `agent-profiles.json` で
別途固定されているため、CLI 版の追従を許すことはテンプレートの再現性方針と両立する。

## Decision

### D1: bootstrap が codex を「解決・検証・リンク」で配備する

`cargo make bootstrap` に codex 配備ステップを追加する。ステップは**通常環境**（shim が
正常に機能する環境）で次を行う:

1. `command -v codex` の候補を**サニタイズ模擬環境**（一時ディレクトリを HOME にした
   `<候補> --version` 実行）でプローブする。通れば、その実体を配備先へリンクする。
2. プローブが失敗する候補（shim 等の環境依存エントリ）しか無い場合、codex の公式配布
   チャネルである npm に通常環境で問い合わせ、**公開インターフェースのみ**を使って
   エントリを特定する: `npm prefix -g`（documented なコマンド）が返す prefix の
   `bin/codex`（パッケージが `bin` として宣言し npm が配置する公開エントリ）。
   パッケージ内部（`node_modules` 配下の vendored バイナリ等、`bin` 宣言外のファイル）
   には触れない。特定したエントリを D2 の前置規則込みで同じプローブに通し、通った場合
   のみリンクする。この経路で harness が実行するものは、利用者の shim が最終的に実行する
   ものと同一のエントリである。
3. いずれも成立しない場合は、actionable なエラー（何をプローブしてなぜ失敗したか、
   利用者が取れる選択肢）で停止する。

配備先は repo-local（例: `.harness/tools/bin/codex`、gitignored）の**シンボリックリンク**
とする。実体の複製・ダウンロード・checksum 固定は行わない。ステップは冪等で、再実行は
リンクを最新の解決結果で張り直す。手順 2 は特定 toolchain-manager の知識ではなく
「codex 自身の配布チャネル」の知識であり、利用者がどのマネージャで node/npm を管理して
いても npm 自身がそのマネージャ経由で解決するため、`8a981fe` / ADR 1438 の
toolchain-manager-agnostic 原則と両立する。

### D2: sotp の解決規約は「repo-local リンク最優先 → PATH fallback」

codex を spawn する全経路は、project root 相対の配備リンクが存在し実行可能ならそれを使い、
なければ従来どおり OS PATH へ委譲する。この規約は入れ子（外側 fixer 内で起動される
内側 reviewer の解決）でも同一に成立する — 配備リンクは project root 相対であり、実 HOME
にも呼び出し元の PATH にも依存しないため、サニタイズ環境・外側 sandbox の下でも解決できる。

- child PATH への親ディレクトリ前置（colocated runtime の発見用）は、**リンクが指す
  解決エントリ（bootstrap が記録した公開エントリパス）の親ディレクトリ**に対して行う。
  npm の global bin ディレクトリには実体の `node` が同居しており、これを PATH 先頭に
  置くことで `#!/usr/bin/env node` が shim の node ではなく実体を先に見つける（shim
  node が PATH 先頭にある通常環境で launcher が sanitized HOME 下で死ぬ既知の失敗は、
  この前置で解消される）。エントリを最終実体まで canonicalize した親（パッケージ内部の
  `bin/` ディレクトリ）を使ってはならない — そこに runtime は同居しない。
- リンクが dangling（リンク先消失 — 例: マネージャ側の runtime バージョン更新でパスが
  変わった）の場合は「配備なし」と同義に扱い、PATH fallback を試した上で D4 の案内へ
  つなげる。黙って壊れた状態で spawn しない。

### D3: env-var オーバーライドと manager 知識の残滓を撤去する

- `review_fix_runner` の実行時 `CODEX_BIN` 参照を削除する（`8a981fe` が設けた外部注入
  契約は D1/D2 により不要になる）。ref-verify で確立済みの「env var によるバイナリ解決は
  アンチパターン」制約を fixer 系にも一般化する。テスト専用 `SOTP_CODEX_BIN`
  （`#[cfg(test)]`）は維持する。
- `dry_fix_local/env.rs` の `resolve_codex_via_asdf` と asdf 用 env 引き継ぎを削除し、
  2026-06-01 の toolchain-manager-agnostic 決定への逸脱を解消する。
- ソース `Makefile.toml` と配布 `overlay/Makefile.toml` の `CODEX_BIN="${CODEX_BIN:-…}"`
  インライン解決行を撤去し、素の `bin/sotp …` 呼び出しへ戻す。ソースと overlay の
  非対称も解消される。

### D4: サニタイズ spawn の失敗を観測可能にする

サニタイズ環境で起動した codex 子プロセスが失敗した場合、利用者向けエラーに最低限
(a) 子プロセスの exit code、(b) セッションログのパス、を含める。配備リンクが存在しない
または dangling で、PATH fallback も失敗した場合は、`cargo make bootstrap` の（再）実行に
よる再解決を案内する。「fix runner did not report a completion status」単独の報告は
不合格とする。

また、spawn の成否によらず、セッションログに**解決に使った実体パス（リンクの場合は
canonicalize 後）と `codex --version` の結果**を記録する。リンク先が古い実体のまま残存し
利用者のターミナル側 codex と乖離する「stale だが dangling でない」状態は機械検知
できないため、この記録を乖離の観測手段とする。

### D5: 初版スコープの刈り込み

初版は実障害が確認された Linux を対象とし、symlink 前提で実装する。他プラットフォーム
（symlink 制約のある環境ではコピー配備等の代替）、バージョン再現性が必要になった場合の
pin モード（checksum つき固定配備）の追加はスコープ外とし、Reassess When に送る。
配備が不可能な環境では D2 の PATH fallback と D4 の案内が下限の動作を保証する。

## Rejected Alternatives

### A. bootstrap での pinned download（実バイナリの複製固定）

公式配布物を checksum 検証つきで取得し実体を固定配備する案。バージョン再現性は得られるが、
pin 更新運用（codex リリース追随・checksum 整備）とネットワーク取得・検証の実装を
テンプレート側が恒常的に負う。codex は auth も版も利用者所有であり、挙動の再現性は
`agent-profiles.json` のモデル・effort 固定が担っているため、リンク保持による最新版追従を
優先して却下。pin モードは必要が生じた時の追加とする（D5）。

### B. in-binary の toolchain-manager 解決（asdf which の共通化）

dry-fix の `resolve_codex_via_asdf` を全経路へ広げる案。commit `8a981fe` が「インフラ層は
特定開発機のセットアップに結合しない」として意図的に削除した設計の復活であり、マネージャ
追加ごとの whack-a-mole になるため却下。逆に dry-fix 側の残滓を削除する（D3）。

### C. Makefile でのマネージャ・カスケード列挙

`asdf which || mise which || command -v` を配布 Makefile に書く案。ADR 1438 が撤去した
個人環境依存の配布物再混入であり却下。

### D. `CODEX_BIN` 契約の文書化のみ

利用者に設定負荷と文書読解負荷が残り、負荷ゼロ目標に反する。env var 解決アンチパターン
制約とも逆行するため却下。

### E. サニタイズ契約の見直し（HOME 差し替え廃止、CODEX_HOME 分離のみ）

shim は全マネージャで動くようになるが、workspace-write はネットワーク有効であり、実 HOME
を渡すと `~/.config/gh` の OAuth トークン等へ既定経路で到達可能になる。credential
isolation の実質的後退のため却下。

### F. shim-free 配置の利用者要求

`~/.local/bin/codex` 等への実体配置を利用者に求める案。マシン環境変更の要求であり
負荷ゼロ目標に反し却下。

### G. npm パッケージ内部の vendored バイナリへの直接リンク

`npm root -g` 起点で `@openai/codex` パッケージ内部（`node_modules` 配下）の platform
vendored self-contained バイナリを特定して直接リンクする案。self-contained ゆえ runtime
同居の考慮が不要になる利点はあるが、パッケージが `bin` 宣言していない**非公開の内部
レイアウト**への結合であり（npm の hoisting・パッケージ構成変更で黙って壊れる）、公式
launcher を迂回する**非公認の実行経路**を harness だけが使う状態（利用者の codex と
実行フレーバーが分岐する）を生むため却下。公開エントリ + runtime 前置（D1 手順 2 /
D2）で同じ目的を公認経路のまま達成できる。

## Consequences

### Positive

- npm + asdf を含む任意の導入形態の利用者が、設定なし・文書参照なしで初回レビューから動く
  （bootstrap は既存の必須手順）。
- 利用者が自分のチャネルで codex を更新すると、同一パスへの上書き更新である限りリンク
  経由で**自動追従**する（pin 更新の運用が発生しない）。パスが変わる更新（runtime
  バージョン変更等）は dangling link として機械検知され、bootstrap 再実行の案内で回復する。
- toolchain-manager 知識がソース・配布物の全層から消え、`8a981fe` / ADR 1438 /
  ref-verify 制約と完全に整合する。dry-fix の逸脱も解消され、sotp 側のコードは純減する。
- 入れ子 reviewer を含む全 spawn 層で解決が同一規約になる。
- 解決不能な環境でも exit code・ログパス・再解決案内で自己診断できる。

### Negative

- 利用者間・実行間での codex CLI バージョン再現性は持たない（現状の「PATH にある何か」
  と同等。挙動の主決定要因であるモデル・effort は `agent-profiles.json` が固定済み）。
- リンクは利用者マシンの絶対パスを指す per-machine 状態であり、2 種の staleness モードを
  持つ: (a) リンク先の消失（dangling — 機械検知して D4 の案内で回復）、(b) リンク先の
  旧実体が残存したまま利用者側の codex だけ更新される乖離（stale だが dangling でない —
  機械検知は不可能なので、D4 のセッションログへのパス + version 記録を観測手段とし、
  bootstrap 再実行で解消する）。なお実体の複製は作らないため「バイナリが二重になる」
  ことはなく、利用者管理領域（toolchain-manager のツリー、PATH）には一切書き込まない。
- bootstrap がプローブ・npm 問い合わせ・リンク作成の実装を負う。

### Neutral

- reviewer / ref-verify の親環境継承・OS PATH 委譲は fallback として残る（配備済み
  環境では使われない）。
- 既存の `CODEX_BIN` 依存運用は撤去に伴い無効化される。

## Reassess When

- codex CLI バージョンの再現性が実運用で必要になったとき（checksum つき pin モードの追加）。
- Linux 以外・symlink 制約のあるプラットフォーム利用者が現れたとき（コピー配備等の代替）。
- codex の公式配布形態（npm パッケージの vendored 構造等）が変わったとき（D1 手順 2 の追随）。
- codex CLI が実体パス照会または shim-safe な起動手段を公式提供したとき（解決手順の簡略化）。

## Related

- `knowledge/adr/README.md` — ADR 索引
- `knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md` — 入れ子
  reviewer の sandbox 制約（writable_roots / network_access）
- `knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md` — overlay からの
  `asdf which` 撤去（本 ADR はその原則を維持したまま暗黙契約の欠陥を解消する）
- commit `8a981fe`（2026-06-01）— review fixer の toolchain-manager-agnostic 化
  （drop asdf coupling）
- `libs/infrastructure/src/dry_check/dry_fix_local/env.rs` — D3 で削除する逸脱
  （`resolve_codex_via_asdf`）
- `libs/infrastructure/src/ref_verify/process_runner.rs` — env-var 解決アンチパターン制約の
  既確立箇所（module doc）
