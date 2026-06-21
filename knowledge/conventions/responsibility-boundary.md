# Responsibility Boundary: Framework Enforcement vs Consumer Configuration

## Purpose

SoTOHE はテンプレート（framework）であり、その CI ゲート / verifier が **enforce してよい領域** と **してはいけない領域** を分ける。enforce すべきは SoTOHE 自身の方法論と framework コードの整合性に限る。テンプレート利用者（fork する側）の provider / agent **設定**は利用者の責任領域であり、SoTOHE は良いデフォルトと docs を**提供**するだけで、CI で強制しない。

この分界点をはき違える（利用者の設定を SoTOHE が enforce しようとする）と、verifier が設定の正確値に結合して脆くなり、「ゲートが緑＝正しい」という偽の安心を生み、本当に守るべき drift（DRY / 設計意図）は review でしか捕まらないのに高保守なゲート群だけが残る。

## Scope

- Applies to: 新しい CI ゲート / verifier / `cargo make verify-*` / `sotp verify *` / signal を追加・拡張するとき、あるいは既存ゲートの存続を判断するとき。
- Does not apply to: 設定ファイルそのものを**提供**すること（テンプレートが `.claude/settings.json` や `.codex/*` のデフォルトを同梱し、docs で意図を説明するのは正しい。問題は「変更を CI で hard-fail させる enforcement」だけ）。

## Rules

- **SoTOHE が enforce してよい領域**（CI ゲート可）:
  - SoT Chain の整合性 — `spec.json` ↔ ADR ↔ `<layer>-types.json` ↔ `impl-plan.json` の構造/意味整合、decision grounding、生成ビュー freshness（JSON SSoT から再生成した `spec.md` / `plan.md` / `registry.md` 等が一致すること）。
  - workflow / 方法論の機構 — track phase、`/track:*` ゲート、ref-verify（SoT semantic）。
  - architecture rule — `architecture-rules.json` の層依存方向。
  - **framework 自身のコード**の品質 — `sotp` CLI と `libs/*` の fmt / clippy / test / no-panic / DRY。
- **利用者が所有する領域**（提供 + docs のみ。CI 強制しない）:
  - provider / agent 設定 — `.claude/settings.json`（permission allow/deny ＝利用者のセキュリティ姿勢）、`.codex/*`（config / rules / hooks / agents）、model / provider / skill / hook の選択。
  - signal gate 設定 — `.harness/config/signal-gates.json`（SoT Chain の各ゲートの strictness）。SoTOHE は推奨デフォルトを同梱し docs で意図を説明するが、CI で強制しない。
    - `commit_gate.impl_catalog: "interim"` は TDDD の構造的必然（カタログ宣言フェーズで Yellow `RustSourceAbsent` が出るため、commit 時に strict にするとコミット不能になる）。
    - `commit_gate.adr_user: "interim"` は SoTOHE のワークフロー選択（ADR Yellow → エスカレーションをどのコミットでも着地させられるよう commit 時は lax にし、merge 時に strict に切り替えて PR キューで保証する）。
    - テンプレート利用者は自ワークフローに応じてこれらを上書きしてよい。adr-strict バリアント（`commit_gate.adr_user: "strict"`）は `.harness/config/samples/signal-gates.adr-strict.json` を参照。
  - 利用者の domain コード（SoTOHE は architecture を提供し、利用者が中身を埋める）。
- **禁止**: 利用者所有領域の設定値・ファイル存在・allow/deny エントリ・散文の正確文言を verifier の期待リストに結合させて CI で hard-fail させること。これは越権であり、脆く、偽の安心を生む。
- 設定の良し悪し（危険な allow を入れていないか等）は、利用者自身の責任と review（人間 / LLM の判断）に委ねる。SoTOHE は危険な例とその理由を docs に書いて**警告**するが、CI で**強制**しない。

## Examples

- Good: `verify-view-freshness`（生成ビューが JSON SSoT と一致するか）、`adr-signals`（decision が grounding されているか）、`check-layers`（層依存方向）、`cargo make ci` の fmt/clippy/test。いずれも SoTOHE 自身の整合性。
- Good: `.codex/config.toml` や `.claude/settings.json` を**デフォルトとして同梱**し、`.claude/rules/10-guardrails.md` 等で意図・危険例を説明する（提供 + docs）。
- Good: `.harness/config/signal-gates.json` を**推奨デフォルトとして同梱**し、`responsibility-boundary.md` で `interim` の理由（TDDD 構造的必然 vs ワークフロー選択）を説明する（提供 + docs）。
- Bad: `verify-orchestra` のように `.claude/settings.json` の allow/deny エントリや `.codex/*` の設定値・ファイル存在・rules の allow リストを「期待リスト」と照合して CI で hard-fail させる（利用者設定の enforcement ＝越権。2026-06-13 に全廃。ADR `2026-06-13-0002-codex-orchestrator-settings-addition` 参照）。
- Bad: 人間向け doc に特定の散文スニペットが在る/無いを CI で照合する（言い換えで壊れ、偽安心）。

## Exceptions

- テンプレート開発中、SoTOHE 自身が dogfooding で使う設定（このリポジトリ自身の `.claude` / `.codex`）について、**maintainer の任意の自己規律**として lint したい場合は、CI の hard-fail ゲートではなく非ブロッキングな補助チェック（警告のみ）に留める。利用者の fork に強制が伝播しない形であること。
- 例外を採るときは ADR に記録し、本 convention を参照する。

## Review Checklist

- 新しいゲート / verifier を足すとき: 「これは SoTOHE の方法論 / framework コードの整合性か、それとも利用者の設定選択か？」を問う。後者なら**提供 + docs に留め、CI 強制しない**。
- 既存ゲートの存続判断: 利用者設定の正確値・存在・文言に結合していないか。していれば撤去候補。
- ゲートが「presence-check（在れば緑）」で「correctness」を保証していないなら、偽安心の疑い。

## Related Documents

- `knowledge/conventions/enforce-by-mechanism.md` — 機構で強制する原則（その「機構」が enforce してよい対象が本 convention の前段）。
- `knowledge/conventions/workflow-ceremony-minimization.md` — 形骸化する人工状態フィールドを作らない原則。
- `knowledge/adr/2026-06-13-0002-codex-orchestrator-settings-addition.md` — `verify-orchestra` 全廃と provide-not-enforce の決定。
- `.claude/rules/10-guardrails.md` — 危険な permission の**説明**（警告であって CI 強制ではない）。
