---
adr_id: 2026-09-08-1610-upstream-handoff-hardening
decisions:
  - id: D1
    user_decision_ref: "chat:2026-09-09:upstream-handoff:指摘を選別してADRからPRを作成し、ADRは指摘内容から自律的に作成するとの委任"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-09-09:upstream-handoff:指摘を選別してADRからPRを作成し、ADRは指摘内容から自律的に作成するとの委任"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-09-09:upstream-handoff:指摘を選別してADRからPRを作成し、ADRは指摘内容から自律的に作成するとの委任"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-09-09:upstream-handoff:指摘を選別してADRからPRを作成し、ADRは指摘内容から自律的に作成するとの委任"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-09-09:upstream-handoff:指摘を選別してADRからPRを作成し、ADRは指摘内容から自律的に作成するとの委任"
    status: proposed
  - id: D6
    user_decision_ref: "chat:2026-09-09:upstream-handoff:指摘を選別してADRからPRを作成し、ADRは指摘内容から自律的に作成するとの委任"
    status: proposed
---
# consumer 運用で見つかった共通ハーネスの責務・復旧・検証契約を整える

## Context

agent-router の上流同期後の運用から、手順の重複、provider 委譲の矛盾、完了状態からの修復経路、検証器の責務越境、失敗診断の欠落が報告された。上流の現行ソースと照合したところ、文書と機構の不一致が残っていた。個別の変更で新しく発生した回帰とは断定していない。

特に複合 anchor に対する純粋なメモリ上の registry の評価で、異名 session の共存を確認するテストを認めながら、別の保存 adapter が担う保存場所まで確認するよう要求した Fail が観測された。既存の判定契約は entry 局所であるが、較正は既知の不合格例を中心としており、正しく限定した合格例を確認できない。また、モデル設定だけを変えても、同じ判定入力と prompt fingerprint に結び付く Fail は再利用される。

以下の判断は、ユーザーの「指摘内容から自律的に ADR を作成する」という委任に基づく。個々の選択肢についてヒアリングで回答を得たという記録ではない。

## Decision

### D1: 共通手順の正本と provider adapter の境界を既存の所有権に揃える

H01〜H04、H14、H15 を対象とする。共通の手順、状態遷移、入力取得、再開条件は `.harness/workflows/` に集約し、adapter には呼び出し面、provider 固有配線、ツール制約、報告形式を残す。既存の手順を移す際は、入力確認や裁定境界そのものを削除しない。

Codex root からの汎用 capability 呼び出しは `--host` を省略し、設定済み provider の subprocess を使う。Claude の実装 adapter は dispatcher を先に呼び、`delegate-in-host` を返した場合だけ in-host の実装担当を起動する。phase writer は引き続き phase entry が起動する。rollback 診断の `impl` 経路は implementer への委譲を通常経路とする。

PreCompact の再開案内は CLI summary と正本文書への参照を使う。Claude agent 一覧は「利用可能な adapter」を説明し、現在の provider 割り当てや特定の担当数を固定しない。実際の割り当ては既存の profile を参照する。

### D2: 配布既定値と型宣言の案内を現行契約に整合させる

H05、H06、H12 を対象とする。Claude の配布用 allowlist に phase、test-obligation、catalog、ref-verify の既存コマンド系統を接続する。利用者の設定値を CI で固定する契約にはしない。

type-designer の enrollment 参照は、その成果物を実際に所有する type-design workflow の終端手順を指す。通常の inherent method は `TypeEntry.methods` に置き、schema がサポートする top-level `inherent_impls` の用途と区別する。同じ宣言を両方に要求せず、既存の top-level 形式のサポートは維持する。新たな schema 形式は追加しない。

### D3: PR レビューの共通方法論と完了表示を分離する

H07、H08 を対象とする。共通の PR レビュー方法論は framework 所有の `.harness/prompts/pr-reviewer.md` に置き、custom prompt は consumer の focus、severity と共通方法論への参照を保持する。レビュー入口から共通方法論へ到達できるようにし、root の常時入力へ本文を追加しない。

明示的な指摘なしと、ユーザーが承認した Accepted Deviations は別の完了結果として表示する。後者を `zero findings` と表示しない。既存の逸脱承認要件は維持する。

### D4: Done 後の実在する残作業は正規タスクとして復旧し、再計画後は実装へ戻す

H09、H13 を対象とする。test-obligation の Done / Archived 凍結を撤去しない。同じ track の open PR に実在する残作業が発見された場合は、復旧を統括する workflow が PR 状態と現在ブランチの対応を読み取り確認し、既存のタスク追加 API で残作業を記録する。状態は task 群から再導出し、再計画・義務再導出を通常経路で行う。完了履歴をダミータスクや無意味な状態往復で書き換えない。

merge 済み、archived、または閉じられた PR の修正は新しい corrective track に分離する。PR 状態を確認できない場合は既存 track を再開したと仮定せず、原因を報告する。これは workflow の復旧判断であり、タスク追加や derive のたびに GitHub 照会する新たな機構にはしない。通常の PR 作成前の開発手順にはこの PR 復旧条件を課さない。

PR の writer 修正後は依存順に下流を再収束させ、その結果に未完了 task があれば通常の full-cycle へ戻る。実装、義務検証、レビュー、commit、履歴の記録を完了してから PR を再審査する。未完了作業がなければ通常の修正レビューと commit へ進む。この分岐を workflow と関連 policy で一致させる。

### D5: reviewer の失敗は安全な診断を保ったまま上位へ伝える

H10 を対象とする。subprocess の失敗を情報のない `ReviewerAbort` に潰さず、provider、取得できた終了コード、失敗を区別できる安全な理由を上位 CLI へ伝える。利用者中断、timeout、出力形式不正と区別し、失敗から成功 verdict を生成しない。

自由文の診断を表示する場合は既存の秘匿境界を通し、長さを制限する。任意の subprocess 出力をそのまま error、debug、ログへ追加しない。秘匿できる根拠がない部分は固定された分類と終了コードへ縮退させる。新しい診断ログ保存機構や生の秘密を含むログ参照は追加しない。

### D6: 複合 anchor の局所判定を正例・負例で較正する

H11 を対象とする。判定対象は既存の obligation item と entry declaration が担う、引用 anchor 内の振る舞いである。anchor 全文は渡したまま、対象 method / entry の識別と責務を入力から確認できるようにする。複合 anchor の別 entry 所有部分を Fail の根拠にしない。一方、対象が所有する中心的な振る舞いが未検証なら既存の Fail 類型を適用する。他 entry の検証済み状態を仮定したり、存在を探索したりする新たな横断検査は導入しない。

較正には、メモリ上の独立した名前の共存を所有する対象と、保存場所を所有する対象について、それぞれ対象部分が検証済みの合格例と未検証の不合格例を用意する。既存の矛盾・すり替え・中心部未検証の検出は維持する。正例を常に Fail にする判定器も健全と扱わない。既存の較正を無効にする設定は維持し、有効時の追加判定コストを明示する。

prompt の実質的な変更は通常の fingerprint 更新で既存判定を失効させる。判定入力の構成を変更する場合も、その有効性を cache 同一性に反映する。モデル名変更による再判定、force フラグ、cache 手編集・削除、無意味な hash 変更、有効な参照の削除、責務越境した実装や不適切な waiver を回避策にしない。

構造的な回帰試験と、実際の設定 provider を使う較正結果を別々に報告する。有限の較正例が未知の複合 anchor を完全に判定できる証明だとは扱わない。

## Rejected Alternatives

### A: consumer の作業ツリーを一括移植する

consumer 固有の source、bindings、catalogue、cache、provider 同期や既存の未コミット変更が混在するため採用しない。共通の正規ソースに必要な変更を個別に反映する。

### B: Done 凍結を解除する、または PR 状態を全書き込み API の前提にする

前者は完了記録の保護を弱め、後者はローカル操作へネットワーク依存を広げる。実在する残作業の正規登録と、PR 復旧 workflow の判断に限定する。

### C: Fail の一例を無条件に Pass へ訂正する

原因は prompt の曖昧さ、入力の不足、モデルの誤判断のいずれかに確定していない。判定契約と較正で境界を検証し、正規の再評価で結果を得る。

### D: stderr の全文を上位 error に含める

認証情報などを露出するため採用しない。安全な分類・終了コードと秘匿・有界化された情報に限定する。

## Consequences

- 良: provider 選択と実行担当の責務が一致し、修復作業が通常の計画・実装・検証へ戻る。
- 良: 複合 anchor の過剰要求と、局所責務の検証不足を別々に検出できる。
- 負: 較正有効時は追加の provider 呼び出しが発生し、応答時間と費用が増える。
- 負: PR 復旧時はリモート状態を取得できなければその経路を進められない。一般 API の保護は従来の branch / lifecycle guard に依存する。
- 中立: H16 の Unix ホスト条件は既に整備されているため変更しない。consumer 製品の Windows 対応は対象外である。
- 中立: 出荷元の選択は既存 template boundary と export 機構に委ね、配布成果物だけの編集や内容を固定する文字列テストは増やさない。
- 中立: consumer への再取り込みと再評価は別担当の操作であり、上流 PR の完了だけで consumer の Fail 解消を宣言しない。

## Reassess When

- 明示した局所責務と較正例でも、責務越境した判定が継続して観測されるとき。
- 実在する修正 task を表現できない正当な Done 後修復が見つかったとき。
- PR 状態確認を workflow で行う保護が実運用で不十分だと確認されたとき。
- provider の診断形式が変わり、既存の秘匿境界では安全な理由を抽出できなくなったとき。

## Related

- [Claude/Codex 運用文書の正本化](2026-06-30-0425-harness-workflow-ssot-adapters.md) — D1 は既存の所有権を維持する。
- [省略 host の subprocess dispatch](2026-08-03-1010-capability-exec-omitted-host-dispatch.md) — D1 はこの呼び出し契約を adapter に整合させる。
- [テスト義務ゲート](2026-07-02-0359-test-obligation-and-fulfillment-gate.md) — D6 は局所判定と fingerprint の決定を維持し、合格例の較正を補う。
- [method 単位の anchor 所有権](2026-08-13-1720-test-obligation-method-anchor-ownership.md) — D6 は所有 anchor 内での対象責務の判定を明確にする。
- [現在ブランチに紐付く書き込み保護](2026-05-26-0518-active-track-write-guard.md) — D4 は既存の branch 保護を変更せず、PR 復旧の運用条件を補う。
- [秘匿の fail-closed 契約](2026-08-20-1053-sensitive-redaction-fail-closed.md) — D5 の診断表示に適用する。
