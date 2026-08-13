
## 2026-08-11: RunReviewOutput.diagnostics の grounding 欠落(User 裁定待ち)

types scope review が `RunReviewOutput` (usecase-types.json) の空 `spec_refs[]` を P1 として指摘。
type-designer の横断監査の結論: T043 の modification である `diagnostics: Vec<DiagnosticText>`
(reviewer stderr/diagnostics を run lane で表示する変更) を支持する element は凍結済み spec v1.3
に存在しない。無関係な frozen anchor を引くのは false grounding であり不可。

選択肢 (spec 凍結裁定 2026-08-11 に基づき User 裁定必須):
1. spec workflow を通じて適切な behavioral contract を追加する (凍結の例外承認が必要)
2. 未支持の catalogue modification (diagnostics 露出) を除去/再分類し、実装も追随させる

なお `ReviewRunLocalOutput` 側は正当な既存 anchor (IN-05, CN-03, AC-05) で解決済み (Blue)。

## 2026-08-11 (続報): RunReviewOutput.diagnostics — 暫定処理

調査結果: diagnostics フィールドは新機能ではなく、composition 層の既存
`CodexReviewOutcome::WithDiagnostics`(run lane セットアップ診断の stderr 表示)を
cli-via-usecase-only D1(composition-split ADR)に基づき usecase DTO へ移設した
transport-only 変更と判明。意味追加ではないため、暫定として informal_grounds
(composition-split ADR D1 transport 移設 + 本記録)で grounding し、gate を通す。
ユーザーが spec 例外追加 or 除去を裁定した場合は即差し替える(catalogue テキストのみの
可逆変更)。
