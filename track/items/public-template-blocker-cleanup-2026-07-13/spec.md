<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 46, yellow: 0, red: 0 }
---

# 公開テンプレート配布前の阻害要因解消

## Goal

- [GO-01] 通常の開発作業ツリーでの template export が、Git 管理外かつ gitignore 対象の一時生成物によって阻害されず、未分類の配布候補は fail-closed のまま扱われる状態にする。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D1]
- [GO-02] 固定 tag による更新・他ホスト再導入経路を、公開前に到達可能性を確認する約束事として維持する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D2]
- [GO-03] export されるテンプレートが、新規取得直後に存在しない提供元固有の具体参照なしに自己完結して利用できる状態にする。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3]
- [GO-04] track archive の操作と表示を CLI / workflow の単一の業務ロジックおよび directory 位置から導出する状態に統一する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D4]
- [GO-05] 公開元リポジトリと exported template の双方から作業機を特定し得る絶対パスを除去し、将来の再混入を防ぐ。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5]
- [GO-06] 作業機の絶対パス混入を、公開元リポジトリと export 出力の機械検査によって CI 時点で fail-closed に検出する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6]
- [GO-07] 出荷対象に対する具体 ADR 参照および具体 track 参照の再混入を、境界 manifest 由来の字面検査で CI 時点に fail-closed 検出する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7]

## Scope

### In Scope
- [IN-01] template export とその smoke 検査で、Git 管理外かつ gitignore 対象の一時生成物を配布対象外として skip する振る舞いを実現する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D1] [tasks: T001]
- [IN-02] 固定 tag 経路の公開 remote 上での解決可能性を公開前に確認する検査を整備する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D2] [tasks: T004, T007, T008, T009, T010, T011, T021]
- [IN-03] 境界 manifest の include / overlay 分類から導かれる出荷対象を自己完結化し、存在しない提供元固有の具体参照を除去する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3] [tasks: T012, T013, T021]
- [IN-04] archive の業務ロジックを CLI と提供元非依存 workflow に集約し、command 文書と rendered view の archive 表示をこれに整合させる。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D4] [tasks: T014]
- [IN-05] Git 管理下の既存成果物を対象に、作業機を指す絶対パスを意味に応じて repo-relative、汎用表記、削除、または伏せ字へ置き換える。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5] [tasks: T003, T015, T016, T017, T018, T019, T020]
- [IN-06] 公開元リポジトリの verify ゲートと export smoke 側の走査からなる 2 つの検査面、および構造化成果物の path 永続化境界における repo-relative 強制を追加する。2 つの検査面が用いる作業機の home directory は composition root が解決し、adapter / verifier の構築時または引数として明示的に渡す。adapter / verifier は `HOME` や `USERPROFILE` などの環境変数を暗黙に読まず、home directory を解決できない場合は検査を fail-closed とする。codec 境界は home directory を入力に取らず、repo-relative でない値をすべて構築時に拒否するため、環境依存を持たない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T002, T003, T005, T007, T008, T009, T010, T011, T015, T016, T017, T018, T019, T020, T021, T022]
- [IN-07] 出荷対象集合に対する具体 ADR / track 参照の名前キー検査と、その許容・違反 fixture を追加する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006, T007, T008, T009, T010, T011, T012, T013, T021]
- [IN-08] `catalogue-spec-refs --track-id` を含む track ID を受け取る境界で、domain の `TrackId::try_new` へ検証を委譲し、検証済みの TrackId だけを下流へ渡す。 [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D1] [tasks: T007, T009, T010]

### Out of Scope
- [OS-01] Git 管理下の file だけで作った clean checkout を export の唯一の回避策とする運用。通常の開発作業ツリーでの export 振る舞いは置き換えない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D1] [tasks: T001]
- [OS-02] 固定 tag を template export smoke の初回導入導線の必須条件にすること。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D2] [tasks: T004, T021]
- [OS-03] 提供元の全 ADR または track 履歴の同梱、あるいは export 用 overlay による convention の二重管理を参照解消手段として採用すること。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3] [tasks: T012, T013]
- [OS-04] archive 完了状態を metadata の status field で管理すること、または提供元別 command 文書に手動 archive 手順を維持すること。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D4] [tasks: T014]
- [OS-05] 全 absolute path の一律禁止、absolute path の分類・waiver 制度、または D6 の home directory 配下という最低検出対象を超える検出範囲の追加。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5, knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T005, T015, T016, T017, T018, T019, T020, T021]
- [OS-06] 字面だけで正当な実行時説明と区別できない参照、削除済み file 参照、裸の符号の意味論上の不備を名前キー検査だけで判定すること。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006, T012, T013]

## Constraints
- [CN-01] export と出荷対象の検査は、独自の directory 列挙ではなく境界 manifest の分類から対象集合を導出する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3, knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T001, T002, T006, T012, T013, T021]
- [CN-02] この track で導入する公開前検査は、SoTOHE framework 自身の出荷物・コード・方法論の整合性だけを fail-closed にし、テンプレート利用者の provider / agent 設定を期待値照合の対象にしない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6, knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T004, T005, T006, T007, T008, T009, T010, T011, T021]
- [CN-03] 絶対パスの修正と検査は作業機情報を含む path に限定し、作業機情報を含まない system path、container 内 path、または一般例示 path の意味を変更しない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5, knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T002, T003, T005, T015, T016, T017, T018, T019, T020, T021, T022]
- [CN-04] 構造化成果物へ path を保存する境界は、repo-relative でない値を保存前に失敗として扱い、自由記述への混入検出は走査ゲートで補完する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5, knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T003]
- [CN-05] 名前キー検査は安定して字面判定できる具体参照に限定し、字面判定できない参照漏れは既存の出荷対象 review policy で扱う。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3, knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006, T012, T013, T021]

## Acceptance Criteria
- [ ] [AC-01] template export は、Git 管理外かつ gitignore 対象の一時生成物を入力 tree から skip し、その存在だけでは export または export smoke を失敗させない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D1] [tasks: T001]
- [ ] [AC-02] Git 管理下の file、gitignore 対象ではない生成 file、または新規の未分類 file が境界 manifest の分類外にある場合、template export は fail-closed で失敗する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D1] [tasks: T001]
- [ ] [AC-03] 公開前確認が設定済みの git_url と tag を公開 remote で解決できることを検証し、解決できない場合は公開可能と判定しない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D2] [tasks: T004, T007, T008, T009, T010, T011, T021]
- [ ] [AC-04] template export smoke は export 結果に同梱された実行中の sotp を用いる初回導入導線を検証し、固定 tag の存在をこの smoke の必須条件にしない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D2] [tasks: T021]
- [ ] [AC-05] 境界 manifest の include / overlay 分類から導出される export 出力に、利用者の新規取得直後の作業ツリーに存在しない具体 path を現行前提で参照する file が残らない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3] [tasks: T012, T013]
- [ ] [AC-06] 出荷される全 file は、具体 ADR file、具体 track、参照先を特定できない decision / constraint 符号に依存する参照を残さない。このうち convention は、規則の実行に必要な挙動・条件・例を本文だけで理解できる形にする。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D3] [tasks: T012, T013]
- [ ] [AC-07] archive workflow は archive 操作を CLI の `sotp track archive` に委譲し、同じ業務ロジックを再実装しない。提供元別 command 文書は workflow SSoT を冒頭で参照する薄い接続文書となり、metadata 直接編集、手動の directory 移動、または手動 stage file 作成の手順を含まない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D4] [tasks: T014]
- [ ] [AC-08] track directory が track/archive/ 配下にあることだけで archived 状態が導出され、metadata に archive status field を追加・更新せず、registry の Archived 表示も同じ directory 位置から導出される。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D4] [tasks: T014]
- [ ] [AC-09] Git 管理下の成果物を directory 列挙で限定せずに確認し、workspace 内を指す作業機の絶対パスは repo-relative に、workspace 外の一時領域・host 固有 path は意味を保つ必要がある場合だけ汎用表記に置き換え、不要な診断情報は削除または伏せ字化する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5] [tasks: T015, T016, T017, T018, T019, T020]
- [ ] [AC-10] 作業機情報を含まない absolute path（例: /dev/null、/bin/false、container 内 path、一般例示 path）は D5 の書き換え対象にせず、absolute path の分類または waiver 制度を導入しない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5] [tasks: T005, T015, T016, T017, T018, T019, T020, T021]
- [ ] [AC-11] 今後の成果物を書き込む経路は repo-relative path を保存し、既存成果物の一括書き換え後に同じ作業機の絶対パスを再混入させない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D5] [tasks: T003, T015, T016, T017, T018, T019, T020]
- [ ] [AC-12] 公開元リポジトリの verify ゲートは Git 管理下の全 file を走査し、home directory 配下を指す作業機の絶対パスを最低検出対象として CI で fail-closed にする。verifier が用いる home directory は composition root が解決して構築時または引数として明示的に渡し、verifier は `HOME` や `USERPROFILE` などの環境変数を暗黙に読まず、home directory を解決できない場合も fail-closed にする。対象を directory 列挙で絞り込まず、この最低対象を超える検出範囲を必須要件にしない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T005, T007, T008, T009, T010, T011, T021]
- [ ] [AC-13] template export smoke は export 出力を走査し、作業機の絶対パスを検出した場合に fail-closed で失敗する。出力走査に用いる home directory は composition root が解決して adapter / verifier の構築時または引数として明示的に渡し、adapter / verifier は `HOME` や `USERPROFILE` などの環境変数を暗黙に読まず、home directory を解決できない場合も fail-closed にする。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T002, T021, T022]
- [ ] [AC-14] path を保存する構造化成果物の codec 境界は home directory を入力に取らず、repo-relative でない値をすべて構築時に拒否するため、環境依存を持たない。自由記述に対しては AC-12 および AC-13 の走査が同じ再混入を検出する。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D6] [tasks: T002, T003]
- [ ] [AC-15] 具体参照の名前キー検査は、境界 manifest の include / overlay 分類から導出した D3 の出荷対象集合だけを走査し、独自の directory 列挙を持たず、違反時は CI で fail-closed にする。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006, T007, T008, T009, T010, T011, T021]
- [ ] [AC-16] 名前キー検査は、knowledge/adr/ 前置および .md 後置の有無にかかわらず、`\d{4}-\d{2}-\d{2}-\d{4}-[a-z0-9][a-z0-9-]*` に一致する token を具体 ADR 参照として違反にする。日付時刻だけで slug を伴わない token は違反にしない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006]
- [ ] [AC-17] 名前キー検査は、track/items/ 直後の最初の path segment が `[a-z0-9][a-z0-9-]*-\d{4}-\d{2}-\d{2}` で終わる具体 track id を違反にする。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006]
- [ ] [AC-18] 名前キー検査は knowledge/adr/ directory 参照、knowledge/adr/README.md、角括弧の placeholder segment（track/items/<id>/ 等）、単なる日付、日付 suffix を持たない slug を許容する。code fence、comment、prose の文脈は区別せず、裸の CN- 数字型符号は将来の拡張候補として検出対象に含めない。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006]
- [ ] [AC-19] 名前キー検査には、実 ADR file 名は違反、knowledge/adr/README.md は許容、track/items/<id>/ は許容、日付単独は許容、日付 suffix 付き track id は違反、の 5 fixture を含める。 [adr: knowledge/adr/2026-07-13-0818-public-template-blocker-cleanup.md#D7] [tasks: T006]
- [ ] [AC-20] `catalogue-spec-refs --track-id` を含む track ID を受け取る境界は、domain の `TrackId::try_new` に検証を委譲し、検証済みの TrackId だけを下流へ渡す。 [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D1] [tasks: T007, T009, T010]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 46  🟡 0  🔴 0

