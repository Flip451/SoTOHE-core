# 引き継ぎ指摘の選別と ADR への対応

この資料は2026-09-09に受領した consumer 運用報告の内容を、元の一時資料がなくても参照できるように記録する。外部指摘番号はこの表内で定義する。設計判断の正本は `knowledge/adr/2026-09-08-1610-upstream-handoff-hardening.md` であり、ADR 自体はこの表や外部指摘番号に依存しない。

照合した上流 develop は `439d69f372566e9f60e17dd4c483f3ba9ed10a2d`、origin/develop と一致していた。開始時の作業ツリーは clean。下記の行番号は照合時点の参考値である。

| 外部 ID | 問題の内容 | 選別と上流根拠 | ADR |
| --- | --- | --- | --- |
| H01 | 共通手順・状態遷移・入力取得が provider adapter に重複 | 修正。`.agents/skills/track-plan/SKILL.md:26` 等を `.harness/workflows/track/` の正本へ整合 | D1 |
| H02 | Codex root の host 省略規約と adapter の host 明示が矛盾 | 修正。`.codex/instructions.md:94` と track-plan / adr2pr / rollback adapter を整合 | D1 |
| H03 | Claude implement adapter が configured provider を迂回 | 修正。`.claude/commands/track/implement.md:19` は解決前に Agent Teams を要求。dispatcher の結果に従わせる | D1 |
| H04 | rollback の impl 経路が親の source 直接編集を通常手順にする | 修正。`.harness/capabilities/rollback-diagnoser.md:96` と diagnose workflow を implementer 委譲に整合 | D1 |
| H05 | Claude 配布既定 allowlist に phase / test-obligation / catalog / ref-verify がない | 修正。`.claude/settings.json:90`。利用者の値を固定する CI は追加しない | D2 |
| H06 | type-designer の enrollment 参照が旧手順番号を指す | 修正。capability の483行は Step 4、type-design workflow の実体は Step 5 | D2 |
| H07 | 共通 PR レビュー方法論が consumer custom prompt に置かれている | 新規実装。`.harness/prompts/pr-reviewer.md` は現行ツリーに存在せず、`.harness/custom/review-prompts/pr-review.md` に方法論が残っている。framework-owned prompt の新設と custom の focus / severity・参照への分離が必要 | D3 |
| H08 | Accepted Deviations による完了を zero findings と誤表示 | 修正。`.agents/skills/track-pr-review/SKILL.md:62` | D3 |
| H09 | Done で catalogue 修正後、義務再導出が TrackFrozen で止まる | 復旧手順を設計。derive は Done / Archived を拒否し、`libs/usecase/src/task_ops.rs:416` の add_task は実作業を追加して状態を再導出できる。凍結ガードは維持 | D4 |
| H10 | reviewer subprocess の失敗理由が上位 CLI で失われる | 新規実装。`libs/infrastructure/src/review_v2/claude_reviewer.rs:268` が ProcessFailed を情報なしの ReviewerAbort に変換 | D5 |
| H11 | 複合 anchor の別 entry の責務まで検証器が要求する | 新規実装。fulfillment prompt は局所性を明記済みだが、evaluate/calibration.rs は既知 Fail 用。正例・負例と入力責務を確認 | D6 |
| H12 | 通常 inherent methods の配置案内と schema が矛盾 | 部分解消・残部修正。schema は TypeEntry.methods を通常形として案内済み。type-designer capability の385・407行は全 impl の top-level 宣言を要求。調査担当の解消済み判断をこの反証で訂正 | D2 |
| H13 | PR 修正による再計画で未完了 task ができても review / commit へ直行する | 修正。pr-review workflow と review-protocol の双方で未完了作業を通常 full-cycle へ戻す | D4 |
| H14 | PreCompact が古い bulk-read 指示を再注入する | 修正。`.claude/settings.json:57` を summary / pointer に整合 | D1 |
| H15 | Claude agent README が現在ルーティング中の担当一覧のように読める | 汎用部分のみ修正。利用可能な adapter を説明し、特定時点の18件 Codex や dormant 状態は固定しない | D1 |
| H16 | Unix ホスト条件が consumer 環境宣言に未反映 | 上流は解消済み。`knowledge/conventions/environment-assumptions.md:42` に限定したホスト条件がある。製品の Windows 対応は対象外 | 対象外 |

## 検証器の観測証拠

consumer の最終観測は411 Pass / 1 Fail。対象は `SessionRegistry × method:empty × CN-07`。

引用仕様は「session 名は Profile と独立であり、同一 Profile の並行 Session を許す。registry はプロジェクトローカルとし、ユーザーグローバルには置かない」。domain の SessionRegistry は BTreeMap を持ち、empty は空の map を返す純メモリ操作である。保存は infrastructure の FileSessionRegistry などが担当する。

結び付け済みのテストは、空の records、未知名の resolve、同じ Profile の異名 Session の独立した共存、同名登録時の競合を確認していた。保存された Fail は名前の共存が確認済みであることを認めながら、project-local 保存の未検証を要求していた。分類は central_unverified。これは既存の entry-local 判定規定と整合しない。

独立診断は consumer の ADR → spec → catalogue → plan の責務分担を確認し、consumer 設計の rollback 不要、上流 verifier の実装修正を推奨した。その診断は無条件 Pass の根拠ではない。prompt の曖昧さ、入力不足、モデルの誤判断のどれが主因かは未確定であり、同じ Fail が再サンプリングで再現する保証はない。

既存 verdict は tests / declaration / anchor の hash と prompt fingerprint に結び付き、モデル設定変更だけでは再評価されない。cache の手編集・削除、無意味な hash 変更、不適切な waiver、有効な参照削除、domain への保存責務追加は回避策にしない。構造試験と実 provider 較正の結果は別々に記録する。

## 対象外

- E01: Claude 組織設定の HTTP 403。認証・契約設定の問題であり、診断性の欠落とは分ける。
- E02: nested Grok → Codex の namespace 権限エラー。原因未確定。sandbox を無効にしない。
- E03: provider が開始宣言だけを返して規定結果を出さない事例。原因未確定。出力契約を緩めない。
- E04: 複合 shell コマンドの拒否。意図された command policy を一括撤去しない。
- consumer 固有の source / catalogue / bindings / waivers / cache / 作業履歴 / stash と、取り込み済み provider 設定は移植しない。

## 出荷と引き継ぎ

正規編集先は上表の repository source。出荷面は既存 template boundary と export 機構に従う。consumer 側のレビュー承認は上流 CI・レビューを代替しない。上流 PR 後は revision と正規バイナリ更新方法を引き継ぎ、consumer の再取り込み・evaluate/check はその担当へ渡す。上流の完了だけで consumer の Fail 解消を宣言しない。
