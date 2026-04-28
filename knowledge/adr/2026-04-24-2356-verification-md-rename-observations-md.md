---
adr_id: 2026-04-24-2356-verification-md-rename-observations-md
decisions:
  - id: 2026-04-24-2356-verification-md-rename-observations-md_grandfathered
    status: accepted
    grandfathered: true
---
# verification.md を observations.md に改名 — 役割を手動観測ログに限定

## Context

`/track:plan` を Phase 0-3 (`/track:init` / `/track:spec-design` / `/track:type-design` / `/track:impl-plan`) に再構築した結果 (親 ADR `2026-04-19-1242-plan-artifact-workflow-restructure.md`、子 ADR `2026-04-22-0829-plan-command-structural-refinements.md`)、`verification.md` はどの phase command でも生成されない孤児ファイルになった。一方で `verify-latest-track` (`libs/infrastructure/src/verify/latest_track.rs`) は依然として `verification.md` の存在 + scaffold placeholder 不在を必須化しており、`cargo make ci` を通すには手書きが要求される状態が残っていた。

実在する `verification.md` を読み返すと、内容が 2 つの役割に分裂していた:

- 役割 A: acceptance criteria 充足宣言 (AC checklist の手転記、結果 pass/fail、verified_at)
- 役割 B: 機械検証不能な手動観測ログ (rustdoc wall time、TypeGraph ノード数、dogfooding 結果、UX 確認、scope 別ファイル数 など)

`knowledge/conventions/workflow-ceremony-minimization.md` は (1)「成果物レビューは事後方式」で signal 評価 + 実成果物事後レビューを正とする、(2)「人工的な状態フィールドを作らない」、(3)「file 存在 = phase 状態」の 3 原則を規定している。役割 A は spec.json の signals (🔵🟡🔴) + review.json の zero_findings + `impl-plan.json` の task `done` + `commit_hash` で機械的に証明可能であり、`verification.md` への手転記は形骸化 ceremony の典型例だった。

直前の track 成果レビューでも「AC 充足の詳細は spec.json signals と review.json (zero_findings) を正とする。verification.md 自体の役割整理は workflow-ceremony-minimization の思想に沿って follow-up track で ADR + workflow doc の整合を取る予定」という認識が共有されており、本 ADR はその follow-up である。

## Decision

### D1: 役割 A (AC 充足宣言) の廃止

`verification.md` から acceptance criteria 充足宣言の役割を完全に剝がす。AC 充足の正は次の 3 つに移譲する。

- spec.json の signals (🔵🟡🔴) — 各 AC の状態評価
- review.json の zero_findings — review サイクルでの全 finding 解消
- `impl-plan.json` の task `done` + `commit_hash` — 実装と commit の対応

役割 A 相当の「結果 pass/fail」「verified_at」「scope verified」などの記述は新ファイルに引き継がない。

### D2: 役割 B (手動観測ログ) を `observations.md` に移行

機械検証不能な手動観測ログ (実測値、dogfooding 結果、UX 確認、運用観測など) は新ファイル `track/items/<id>/observations.md` に記録する。`verification.md` という名前は新規 track では使わない。

改名の主要動機: 既存の `verification.md` 参照を grep で機械的に洗い出して全置換 / 削除でき、漏れがないか検証可能となる。意味だけ縮退させて名前を維持すると、参照箇所の意味が「旧 = 役割 A+B」「新 = 役割 B のみ」に分裂し、機械的判定ができなくなる。

`observations.md` 内のフォーマットは自由。観測対象 / 観測手順 / 観測値 / 日時 などを項目に含めるかは作成者の裁量とし、scaffold は提供しない。

### D3: CI gate から完全除外

`verify-latest-track` (`libs/infrastructure/src/verify/latest_track.rs`) から `verification.md` (および `observations.md`) の必須化と scaffold placeholder 検出を完全に削除する。

- `VERIFICATION_SCAFFOLD_LINES` の static set とそれを使う `scaffold_placeholder_lines` / `validate_verification_file` を撤去
- `cargo make ci` 経由の `verify-latest-track-local` のファイル内容検証は `spec.md` / `spec.json` / `plan.md` のみに縮退 (最新 track 選択に使う `metadata.json` / `impl-plan.json` の読み込みは引き続き実施)
- `observations.md` は「ファイルが存在しない = 観測なし」として `workflow-ceremony-minimization` の「file 存在 = phase 状態」原則と整合させる
- 将来 `observations.md` に対しても scaffold check / 必須化 を新設しない

### D4: 作成タイミング — implementer 裁量 + spec 明示の観測要求

`observations.md` は次のいずれかの場合にのみ作成する。それ以外の track では作成しない。

- (a) 実装中に implementer が「機械検証不能な観測値が出た」と判断した場合 (裁量)
- (b) `spec.json` の `acceptance_criteria` に「〜を実測して `observations.md` に記録する」と明示された項目がある場合 (必須)

(b) は spec-designer が spec を書く段階で観測要求を AC に組み込んだ track のみが対象。両者は排他ではなく両立しうる (spec 明示の観測項目に加え、実装中に追加観測を裁量で記録するケース)。

`/track:implement` / `/track:full-cycle` の手順記述は「無条件で `verification.md` を update」から「上記 (a)(b) のいずれかに該当する場合 `observations.md` を作成 / 追記」に書き換える。

### D5: 過去 track の `verification.md` は historical artifact として不変

過去の `track/items/*/verification.md` および `track/archive/*/verification.md` は当時の運用 (役割 A + B 混在) で書かれた歴史資料として原型保存する。次のいずれも実施しない。

- batch rename (verification.md → observations.md)
- 内容刈り込み (役割 A 部分の削除)
- observations.md への移行 migration

理由: 過去 `verification.md` の大半は役割 A (AC checklist 転記) であり、役割 B 専用の `observations.md` への rename は意味不一致を発生させる。git blame / 過去 ADR / 過去 commit message / 過去 review.json などからの `verification.md` 文字列参照を温存することも目的。

将来 `track/items/<id>/` 配下に `verification.md` が新規に作成されることはない (新ルール下では `observations.md` のみ)。

## Rejected Alternatives

### A. 名前を維持して意味だけ役割 B に縮退

`verification.md` という名前のまま役割を観測ログに縮退する。

却下理由: 既存 `verification.md` 参照箇所 (code / commands / docs / README / 過去 track) を grep で洗い出しても、参照の意味が「旧 = 役割 A+B」「新 = 役割 B のみ」に分裂し、機械的な書き換え判定ができない。改名すれば `verification.md` 参照 = 旧運用の遺物 / `observations.md` 参照 = 新運用 と一目で区別でき、移行作業が grep-based に効率化する (D2 の主要動機)。

### B. CI gate を optional 化 (存在時のみ scaffold check)

`verify-latest-track` を「`observations.md` があれば scaffold check + empty reject、なければ skip」に変更する。

却下理由: scaffold check の存在自体が「verification.md は本来書くべきもの」という旧運用の名残であり、`workflow-ceremony-minimization` の「形式的手順の追加は justification 必須」原則に反する。役割 B は自由形式の手動観測ログであり、scaffold を強制する根拠がない。完全除外 (D3) が原則整合的。

### C. Phase 4 として scaffold 自動生成

`/track:impl-plan` (Phase 3) または `/track:implement` で `observations.md` の scaffold を自動生成する。

却下理由: scaffold が形骸化する典型シナリオ (空のテンプレートを残したまま commit、CI gate がそれを reject、手で埋めるが内容なし)。`workflow-ceremony-minimization` の「成果物レビューは事後方式」「人工的な状態フィールドを作らない」原則と逆行する。観測値が出てから書く implementer 裁量 (D4 (a)) が思想整合的。

### D. 過去 track も batch rename (verification.md → observations.md)

過去 track の `verification.md` も一括で `observations.md` に rename する。

却下理由: (1) 過去 `verification.md` の大半は役割 A (AC checklist 転記) であり、役割 B 専用の `observations.md` への rename は意味不一致を発生させる。(2) git history が rename detection 越しになり追跡性が低下。(3) 過去 ADR / 過去 commit message / 過去 review.json / 過去 strategy doc などからの `verification.md` 文字列参照が大量に残り、追加の整合作業が膨らむ。歴史資料として原型保存する D5 を採用。

### E. verification.md の役割を残しつつ役割 A も維持 (現状維持 + 孤児解消だけ)

現状の `verification.md` (役割 A + B) を維持し、`/track:plan` Phase 0-3 のどこかに作成経路だけ追加する。

却下理由: 役割 A は spec.json signals + review.json + commit-hash で既に機械証明されており、手転記は二重記録 (signal vs verification.md) で不整合リスクを生む。`workflow-ceremony-minimization` が定める「成果物レビューは事後方式」原則の徹底という本 ADR の主目的を達成できない。

## Consequences

### Positive

- `/track:plan` Phase 0-3 と `verification.md` 作成経路の不整合が解消し、孤児状態が消える
- AC 充足の正が「signal + review.json + commit-hash」に一本化され、二重記録 (signal vs verification.md) の不整合リスクが消える
- ceremony が削減され、観測がない track では `observations.md` を書く義務が消える
- `verify-latest-track` のロジックが縮退し、`VERIFICATION_SCAFFOLD_LINES` および関連テスト (`libs/infrastructure/src/verify/latest_track.rs`) が削除可能になる
- `workflow-ceremony-minimization` の「file 存在 = phase 状態」原則が `observations.md` にも適用される
- 改名により既存参照 (`verification.md`) の grep-based 洗い出しが完全に効くようになり、移行作業の機械的検証が可能になる

### Negative

- 過去 track の `verification.md` と新規 track の `observations.md` がリポジトリ内で共存し、初見の読者には移行期の経緯説明が必要
- `/track:commit` の git note 生成が `observations.md` を optional source として扱うため、過去比 note の情報量が縮退するケースがある (`spec.md` / `plan.md` のみで生成)
- 「optional だから書かない」傾向で、本来観測記録すべき track でも省略される運用劣化リスク。spec-designer が AC に観測要求を明示する (D4 (b)) ことで部分的に対処するが、実装中に発生する想定外観測 (D4 (a)) は implementer の判断に依存
- 移行作業として `verify-latest-track` のコード削除 / commands 文言更新 / docs 更新 / 過去 track 参照の更新が必要 (本 ADR を materialize する track で実施)

### Neutral

- `VERIFICATION_SCAFFOLD_LINES` の日本語 scaffold ("検証範囲" / "手動検証手順" など) も同時に廃止される。日本語 scaffold 検出機能を独立トラックで導入した経緯 (`track/archive/ci-verification-hardening-2026-03-12`) は historical record として残る
- `START_HERE_HUMAN.md` および `.claude/rules/` の `verification.md` 参照は削除または `observations.md` 文脈に書き換わるが、運用フロー自体の変化は (役割 A 廃止以外には) 観測がある track でも従来と同等

## Reassess When

- signal + review.json + commit-hash の組み合わせでは AC 充足を証明できないケースが発見され、役割 A 相当の総合宣言が再び必要になった場合
- 機械検証不能な手動観測を signal や review.json などの構造化アーティファクトに格納する仕組みが整備され、`observations.md` の自由形式 markdown が冗長になった場合
- `observations.md` が「実装フェーズで毎回書かれる」運用に偏り、optional 化が機能していない場合 (作成必須化または scaffold 復活への retreat 検討)
- `/track:plan` の Phase 構造が変化し、Phase 4 などで観測ログを構造化する必要が出た場合

## Related

- `knowledge/conventions/workflow-ceremony-minimization.md` — 事後レビュー方式 / 「file 存在 = phase 状態」原則の根拠
- `knowledge/conventions/pre-track-adr-authoring.md` — 本 ADR の lifecycle (track 前段階での authoring)
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — Phase 0-3 構造を導入した親 ADR (孤児状態の発生源)
- `knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md` — Phase 命令の責務細分化 (verification.md 作成経路欠如を確認した子 ADR)
- `track/workflow.md` — 「verification.md」セクション (移行で改訂対象)
- `libs/infrastructure/src/verify/latest_track.rs` — `VERIFICATION_SCAFFOLD_LINES` / `validate_verification_file` (移行で削除対象)
