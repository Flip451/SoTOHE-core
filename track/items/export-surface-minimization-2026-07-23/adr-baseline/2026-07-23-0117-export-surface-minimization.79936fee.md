---
adr_id: "2026-07-23-0117-export-surface-minimization"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-22"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-22"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-22"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-22"
    status: proposed
---
# 出荷面を最小化し、workflow と出荷物の乖離クラスを閉じる

## Context

テンプレート利用プロジェクトの実走（2026-07-22）で、出荷面の過剰と欠落が同時に観測された。

**過剰の側。**
境界 manifest は `knowledge/conventions` をディレクトリ丸ごと include しており、機械計測で 29 規約中 9 本が出荷面（.harness / .claude / CLAUDE.md / overlay）から無参照だった。
参照ありでも sotp 開発固有の規約（shell-parsing 等）が新規プロジェクトへ流出している。
skills も同様で、`.claude/skills` のうち新規プロジェクトに必要なのは architecture-customizer のみである。
`track-plan` / `diagnose` スキルは正規アダプタ（commands）と workflow SSoT に対するフェーズ要約の重複コピーであり、「アダプタに論理を複製しない」規則違反の再編前遺物と確認した（skill-compliance hook が /track:plan 文字列に誤発火して要約を注入する挙動も実証済み）。
`.claude/rules` の番号プレフィックス（01/07/08/09/10）は過去削除の歯抜けで、出荷時の間引きでさらに進む。

**欠落の側。**
scaffold で `/track:adr2pr` が `cargo make adr-baseline-check-review` を実行して「Task not found」で停止した。
配布 Makefile の出荷原則——ワークフロー参照タスクのみのゼロベース採録、単発パススルー wrapper の廃止と bin/sotp 直呼びへの正規化——は ADR 2026-07-16-1438 で既決だが、その再構成の後に追加された ADR-baseline ゲート（PR #198〜#201）が既決基準との共同更新を漏らし、review workflow に wrapper 参照が持ち込まれた。
同じ漏れで、scaffold の ci-track には commit 側 byte 照合（`bin/sotp adr-baseline check-commit`）のステップも欠けており、byte 照合ゲートが実質不在である。
なおソース側の該当ゲート実装（`-local` 群）は `cargo run -p cli` でワークスペース内の sotp を実行する dogfooding イディオムであり、scaffold では `-p cli` が利用者アプリの placeholder を指すため、これを流出させるとゲート欠落ではなく誤実行になる。
つまり出荷 Makefile の原則は既決でも、workflow 参照タスクの欠落や workspace CLI 実行の流出という今回対象の乖離を検出する恒常機構が存在せず、原則制定の直後に追加されたゲートで早速 regression が実証された。

## Decision

### D1: conventions をファイル単位分類にする

境界 manifest の conventions をディレクトリ include からファイル単位の分類（汎用 = include / sotp 開発固有 = exclude）に落とす。
UnclassifiedPath の fail-closed により、以後は規約を 1 本追加するたびに出荷可否の分類が強制され、再肥大が構造的に防がれる。
規約索引は自動生成のため、export 後の scaffold の bootstrap に、出荷された部分集合に対する索引再生成を組み込む。
export 時の書き換えは、ADR 2026-07-06-1717 D4 の copy + overlay のみとする決定と Rejected Alternative E に従い行わない。

### D2: skills の出荷を最小化し、重複スキルはソースから削除する

- 出荷する .claude/skills は `architecture-customizer` のみとする。
- `track-plan` / `diagnose` スキルは exclude ではなくソースから削除する（機能は commands 側アダプタが担っており喪失なし）。skill-compliance hook の参照対応表の調整をセットで行う。
- `repomix-snapshot` / `codex-system` / `gemini-system` は配布から外す（開発習慣・provider 便宜スキルであり、capability dispatch の実体は .agents 側と `bin/sotp capability exec` にある）。skills の配布制御は `.claude/skills/.gitignore` の許可リストが既に担っており、同リストから除くことで実現する。git 追跡外の個人スキルはそもそも配布されず、本 ADR の管轄外。
- `.agents/skills` はハーネス機構（workflow / writer capability の provider-native アダプタ群）なので保持する。

### D3: rules の番号プレフィックスを撤廃する

`.claude/rules/` のファイル名から番号を外し、読み順は CLAUDE.md の列挙が所有する（conventions と同形式）。
再発番は増減のたびに歯抜けが再発するため採らない。
常時出荷する rules の必須集合は dev-environment / orchestration / guardrails 相当の 3 本とし、maintainer-checklist は exclude する。
language はこの必須集合に含めず、出荷しないか、出荷する場合は原文ではなく中立版 overlay に差し替える。
CLAUDE.md・rules 相互参照・workflows の参照更新を同 track の co-update に含める。

### D4: 既決の出荷 Makefile 原則に対する乖離 2 種を恒常検査する

配布 Makefile の採録基準と直呼び原則（ADR 2026-07-16-1438 D1 / D2）は再決定しない。
本決定は、workflow 参照タスクの欠落と workspace CLI の `cargo run` 流出の 2 種を検出する強制機構のみを新設する。
ADR 2026-07-16-1438 D1 の採録面全体と D2 の直呼び原則全体への適合性は、この 2 検査の保証範囲に含めない。

- 検査 1: 出荷 workflow（.harness/workflows/**）が参照する cargo make タスク名の集合 ⊆ exported Makefile のタスク集合。
- 検査 2: exported Makefile のタスク定義にワークスペース CLI の `cargo run` 実行（`-p cli` 形式）が含まれないこと（ソース結合イディオムの流出防止）。

検査は既存のスモークゲートに組み込む。
スモークは export 済みツリーを検分する maintainer CI のゲートであり、違反を作れるのも直せるのも maintainer だけなので、対象の 2 種をここで main 到達前に止めれば足りる。
`sotp template export` 本体は変更せず、テンプレート利用者の export には一切触れない。
既に肥大化している verify 系列（`sotp verify` サブコマンド群と ci 依存列）にも載せない。

検査の導入に伴い、Context に記した現存 regression 2 件も修復する。
これらの修復は ADR 2026-07-16-1438 D1 / D2 の適用であり、本 ADR の新規決定ではない。

## Rejected Alternatives

### A. conventions を汎用/開発固有の 2 ディレクトリに物理分割する

既存参照のリンク切れと履歴の断絶を招く。
manifest のファイル単位分類で同じ効果が得られる。

### B. track-plan / diagnose スキルを exclude のみで温存する

ソース側に「アダプタへの論理複製」違反と hook 誤発火が残り続ける。
削除が正しい。

### C. rules の再発番

次のファイル増減で歯抜けが再発し、ソースと scaffold の保持集合差で必ず番号が割れる。
番号を持たないことが構造的解である。

### D. wrapper 廃止と ci-track ゲート補完を本 ADR の決定として持つ

配布 Makefile の採録基準・直呼び原則は ADR 2026-07-16-1438 が正本であり、ここで再宣言すると決定の二重管理になる。
2 件の修復は既決事項の適用として D4 の検査導入に随伴させ、本 ADR の決定は検査 1 / 2 が対象とする乖離 2 種の検出に限定する。

### E. 検査を sotp verify サブコマンド（＋ ci 依存列）として追加する

既に肥大化している verify 系列をさらに伸ばすことになる。
既存スモークゲートへの内蔵なら、新しいゲート面はゼロで済む。

### F. export を無条件に fail-closed で停止する

fail-fast としては最強だが、`sotp template export` はテンプレート利用者が clone 直後に実行するコマンドであり、maintainer にしか直せない違反で利用者側の着手を全面ブロックしてしまう。
しわ寄せの向きが誤っている。

### G. 評価を export に内蔵し、利用者へは警告・maintainer CI ではエラーと帰結を分ける

スモークが main 到達を止める以上、利用者側で警告が火を吹く経路は実質残らない。
起こり得ない事象のために export 本体と警告レーンを複雑化するのは見合わない。

## Consequences

### Positive

- 新規プロジェクトが受け取る規約・スキル・rules が「その場で意味を持つもの」だけになり、初回の認知負荷が下がる。
- 検査は maintainer CI のスモークゲートのみに住み、workflow 参照タスクの欠落と workspace CLI の `cargo run` 流出の 2 種は main 到達前に止まる。利用者の export には変更が及ばず、verify 系列も伸びない。
- D4 の導入時に現存 regression 2 件も既決事項の適用として修復され、scaffold の commit ゲートが成立する。

### Negative

- 初回分類の編集判断コスト（参照閉包は下限であり、9 本の無参照確定分以外は編集的仕分けが要る）。
- hook 対応表・CLAUDE.md・workflows など co-update 幅が広く、複数 track への分割が要る可能性が高い。

## Reassess When

- 規約数の増加で単純分類が煩雑になったとき（用途プリセット束の導入を検討）
- scaffold 利用側から exclude 済み規約・スキルの需要が繰り返し出たとき
- workflow が cargo make タスクを新規参照する頻度が高くなり、D4 の検査が開発摩擦になったとき
- 既存の export 内 fail-closed（UnclassifiedPath 等）が利用者の export を止めるしわ寄せを見直すとき（maintainer CI 側での事前捕捉へ寄せる等）

## Related

- `knowledge/adr/2026-07-06-1717-template-extraction-boundary.md` — export を copy + overlay のみとし、programmatic transform を採らない境界決定（D1 の索引再生成を bootstrap 側に限定する根拠）
- `knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md` — 配布 Makefile の採録基準・直呼び原則の正本（D4 は対象とする乖離 2 種の検出機構）
- `.harness/config/template-boundary.json` / `overlay/`
- `knowledge/conventions/README.md` — 自動生成索引
- `.claude/skills/` / `.agents/skills/` / `.claude/rules/`
- `Makefile.toml` — adr-baseline-check-review wrapper と集約依存
- `.harness/workflows/track/review.md` — Step 1 の参照元
- `knowledge/conventions/workflow-ceremony-minimization.md` / `responsibility-boundary.md`
