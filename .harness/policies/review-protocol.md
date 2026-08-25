# Policy: Review Protocol

## Purpose

reviewer capability によるレビューサイクルの規律を定める。reviewer は capability として dispatch される独立の判定者であり、orchestrator 自身の inline review はその代替にならない。この一点が守られないと、以降のすべてのゲートが自己申告になる。

## Scope

- 適用対象: レビューを誰が実行できるか、いつ完了とみなせるか、返された verdict と finding をどう扱うか。
- 適用外:
  - reviewer / fixer の provider 解決 — `.harness/config/agent-profiles.json` の `capabilities.reviewer` および `capabilities.review-fix-lead`
  - レビューサイクルの手順（scope の決定、ADR baseline の事前チェック、round の段階と escalation、fixer の terminal status、単一 scope 再入ラウンド） — `.harness/workflows/track/review.md`
  - コミット時のゲート構成 — `.harness/workflows/track/commit.md`
  - PR レビューの手順と Accepted Deviations の記載先・フォーマット — `.harness/workflows/track/pr-review.md`
  - scope ごとの diff 上限値 — `.harness/config/review-scope.json`
  - ADR baseline record の designation・検証・復旧と、それが signal gate から独立であること — `.harness/policies/track-lifecycle.md` の ADR baseline lifecycle

## Rules

### 長時間ゲートの待機

reviewer dispatch と review workflow の長時間処理は 1 回の blocking call として待ち、terminal result を 1 回だけ読む。host が call を background 化した場合は、1 回の完了通知後に result を読む。ログの polling、status probe の反復、fire-and-forget launch は行わず、内部の review loop は workflow が所有する。`bin/sotp test-obligation evaluate` は review や commit のゲートではなく、obligation repair における orchestrator host の同期 step に限る。

### レビューを経ずに merge させないこと

守るべき不変条件は「変更が reviewer capability の判定を経ずに merge へ到達しない」ことである。「コミットの前に必ずローカルラウンドがある」はその既定形にすぎず、workflow はもう一つの経路を持つ。

- **ローカルレビュー経路（既定）**: コミット前に全 required scope を `zero_findings` にする。実装コミットも計画 artifact のコミットも同じ扱い。この形はゲートで担保されている — commit workflow は guarded wrapper の前提として `bin/sotp review check-approved` の exit 0 を要求し、ローカルラウンドが 1 度でも記録された後は NotStarted bypass が失効するため、以降のコミットは必ずこのゲートを通る。
- **PR レビュー経路**: PR reviewer は push 済みのコミットしか読めないため、この経路では最初のコミットが判定より先に立つ。review workflow の NotStarted bypass はこの経路のために `check-approved` を exit 0 にする。bypass の条件は review workflow が所有する。**この経路でのコミットは暫定であって承認済みではない** — pr-review workflow が終端に達するまで、その変更は未判定として扱う。終端は 2 つあり、どちらも pr-review workflow が所有する: 明示的な zero-findings、または **user が承認した Accepted Deviations**。後者も正規の終端であって、未判定のまま残るわけではない。

bypass が動かすのは判定の**時期**だけで、判定の要否ではない。

### 機械強制されていない点（明示的例外）

レビュー承認を検査するゲートは commit の `check-approved` 一点しかない。`bin/sotp pr wait-and-merge` が実行するのはタスク完了ガードと signal gate であって、review 承認ではない。したがって PR レビュー経路を選んだトラックでは、merge までの間この不変条件を担保するものは規律だけになる。ゲートを追加せず規律で代替することを選んでいるので、その根拠と再検討条件を残す。

- **根拠**: 塞ぐには merge 側へ「PR レビューが終端に達した」ことの受領記録を持ち込む必要があり、それは commit / merge lifecycle の設計変更になる。一方この失効経路が開くのはローカルラウンドを 1 度も記録していないトラックに限られ、1 ラウンド記録された時点で恒久的に閉じる。実測でも、merge 済み 102 トラックのうち `review.json` を持たないものは **0 件** — ローカルレビューを 1 度も経ずに merge に到達したトラックは無い（2026-07-28 時点の計測。bypass が最初のコミットで一時的に使われた可能性までは、この観測では否定できない）。
- **再検討条件**: (a) `review.json` を持たないトラックが merge された時、または (b) pr-review の終端（明示的 zero-findings / user 承認済み Accepted Deviations）に達していない PR が merge された時。いずれも merge 済みトラックの走査で観測できる。

### reviewer の独立性

- orchestrator の inline review（self-review）を reviewer capability の代替にしてはならない
- self-review で `zero_findings` を宣言してコミットに進むことは禁止
- reviewer が verdict を返せなかった場合はリトライし、それでも返らなければユーザーにエスカレーションする。リトライ回数と失敗時の分岐は review workflow が所有する

### 完了とみなせる条件

- **ローカルレビュー経路**: reviewer が `zero_findings` を返すまでラウンドを継続する。この経路に deviation 終端は無い — `check-approved` が受け付ける承認は通常承認と NotStarted bypass だけで、「deviation 付き承認」という状態が存在しない。受け入れ済みの deviation は briefing の Known Accepted Deviations 節として**事前に**渡し、reviewer がそれを除いた上で `zero_findings` を返す形にする
- **PR レビュー経路**: 終端は明示的な zero-findings、または user が承認した Accepted Deviations の 2 つ。どちらも pr-review workflow が所有する
- どちらの経路でも、修正後に「たぶん通るだろう」で完了を宣言してはならない。修正が入ったら必ずもう一度 reviewer に判定させる

### verdict の改竄禁止

- reviewer が返した verdict をそのまま記録する
- out-of-scope の finding があっても verdict を書き換えない
- 対処できない finding がある場合はユーザーに相談する

### finding の disposition

やってはいけないこと:

- P1 finding を「pre-existing」「out of scope」として勝手に棄却しない → ユーザーに確認
- finding を修正せずに accepted list に追加して回避しない
- reviewer finding を「幻覚だろう」と推測で棄却しない → 必ずソースを確認してから判断する

やるべきこと:

- finding は原則として修正する
- P1 deviation の受け入れはユーザー承認が必要
- テスト失敗を「既存の問題」として片付けない。そう主張するにはベースラインの実測が要る: 設定済み base branch を、**その変更を一切含まないツリー**で走らせること。untracked な新規ファイルもツリーから外れていなければならない

  baseline の測定は、現在の作業ツリーに触れない base branch の独立した clean checkout（測定対象の base commit に固定）で行う。in-place の guarded stash push / pop は baseline の往復手段に使わない。この wrapper は push が作成した stash の commit OID を記録し、pop はその記録された OID だけを適用して無関係な stash entry には触れないが、`--index` 相当の staged 状態の復元は行わないため、staged 状態の損失は防げない。直接の `git stash` / `git switch` は guard の対象で代替にならない。独立した clean checkout を用意できないなら、「既存の問題」と断定せず未分類のまま報告する

### PR finding の修正経路

PR review で actionable finding が返った場合、orchestrator は finding ごとに comment、対象 path / line、track context、requested correction を含む focused briefing を作り、実装変更は `implementer`、review-scope の修正は `review-fix-lead` へ委譲する。親コンテキストでの inline edit は通常経路にしてはならず、委譲先が scoped change を適用して completion を返すまで修正済みと扱わない。

委譲先の completion 後、orchestrator は local review workflow を `zero_findings` まで収束させ、`commit` workflow で修正をコミットしてから PR review を再実行する。委譲が失敗した場合だけ親の直接編集を recovery として許すが、これは non-ADR finding に限る。`knowledge/adr/*.md` の編集を要する finding は親も `review-fix-lead` も決して適用せず、review workflow SSoT の `ADR-scope repair lane` section に従って guardian lane へ route する。その lane の完了後も同じ local review の収束と `commit` workflow を経てから再レビューする。

### レビュー対象サイズ

1 ラウンドの diff は、reviewer が finding をどの変更に帰属させられる範囲に収める。上限値そのものは `.harness/config/review-scope.json` の scope ごとの diff ceiling が所有し、コミット単位（バッチ）の切り方は full-cycle workflow がその値から決める。本書はどちらも再記述しない。

規律として残るのは上限に達したときの扱いである: ラウンドが収束しない、あるいはタスク単体が上限を超えるとき、上限値を緩めて通すのではなくタスク分割を検討する。上限は reviewer の判定能力の代理指標であって、調整可能なコストではない。

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR 索引。本書の原典となる ADR はこの索引から辿る
- [.harness/workflows/track/review.md](../workflows/track/review.md) — レビューサイクルの手順 SSoT
- [.harness/workflows/track/pr-review.md](../workflows/track/pr-review.md) — PR レビューの手順と Accepted Deviations
