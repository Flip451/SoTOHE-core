---
adr_id: 2026-07-18-0340-adr2pr-baseline-diff-comment
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01GX77vvFEu6Z5fapa3jur12:2026-07-18 adr2pr の終了処理に primary ADR の初稿（init 刻印）と最終文面の diff を PR コメントとして投稿するフェーズを追加する裁定 + chat_segment:session-01GX77vvFEu6Z5fapa3jur12:2026-07-18 Phase 0 baseline review の init 刻印 diff（来歴表定義・primary ADR source filter・空 diff 時の中間履歴処理）と guardian の decision-preserved 判定を確認し、loop 精緻化後の D1 文面を承認する裁定"
    candidate_selection: "from:[post-diff-comment-phase,pr-body-section,keep-manual-operation] chose:post-diff-comment-phase"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01GX77vvFEu6Z5fapa3jur12:2026-07-18 diff コメントは自分自身（merge 裁定者）へのメンション付きが望ましいとする裁定"
    candidate_selection: "from:[mention-pr-author-runtime,hardcode-maintainer-handle,no-mention] chose:mention-pr-author-runtime"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01GX77vvFEu6Z5fapa3jur12:2026-07-18 来歴の記録性指摘（守護者判定全文はセッション揮発）を受け、reason 要件の明文化と欠落時 fallback の両方を取り込む裁定 + chat_segment:session-01GX77vvFEu6Z5fapa3jur12:2026-07-18 Phase 0 baseline review の init 刻印 diff（起点入力を local review・外部 PR review・spec→ADR の 🔴 signal・diagnose routing に一般化）と guardian の decision-preserved 判定を確認し、loop 精緻化後の D3 文面を承認する裁定"
    candidate_selection: "from:[reason-requirement-plus-fallback,reason-requirement-only,weaken-to-surviving-records] chose:reason-requirement-plus-fallback"
    status: proposed
---
# adr2pr 終端に ADR baseline diff の PR コメント投稿フェーズを追加

## Context

`/track:adr2pr` は Phase 0 で primary ADR の init 刻印（user 承認済みの初稿）を取り、以後の in-track 編集は adr-editor → adr-diagnoser（決定保存判定）→ escalation 刻印の正規経路で積み上がる。merge 裁定者が PR を監査する時点で最も知りたい情報のひとつは「Phase 0 で承認した文面から ADR がどう変わったか」だが、現状の workflow はこれをどこにも集約して提示しない。ledger と escalation 刻印は track 内 artifact として記録されるものの、PR 上で読める形にはならず、裁定者が自分で diff を組み立てる必要がある。

track `docs-architecture-ssot-realignment-2026-07-17`（PR #200）では、pr-review が zero-findings PASS に到達した後、orchestrator が手動で init 刻印コピーと最終文面の diff（3 行）に escalation 刻印・守護者判定の来歴表を添えて PR コメントとして投稿した。この提示は merge 監査の材料としてそのまま機能したため、adr2pr の終了処理として恒久化する。

恒久化にあたり来歴の記録性を検証した結果、構成要素の永続性は一様でないことが判明した: escalation 刻印の hash・timestamp・reason と、reason を持たない non-semantic-fix 刻印の hash・timestamp は ledger（append-only、コミット対象）に機械的に残り、各刻印を導入したコミットも git 履歴から機械的に導出できる。一方、**adr-diagnoser の判定全文はどの機構にも記録されず**（capability 出力はセッション揮発）、起点入力も PR reviewer 由来は GitHub 上に永続するが、local review や signal / diagnose routing 由来はその要旨が別途記録されるとは限らない。PR #200 で来歴表を再構成できたのは、刻印者が reason に起点入力と守護者判定の要旨を書き込んでいたからであり、これは規範が保証しない運用依存の慣行だった。本 ADR はこの慣行を escalation の要件へ昇格させ（D3）、non-semantic-fix および要件以前・欠落記録の表現を定める（D1）。

## Decision

### D1: adr2pr の終端に ADR baseline diff コメント投稿フェーズを追加する

`.harness/workflows/track/adr2pr.md` に、Step 10（pr-review）が終端状態（machine PASS または user 承認済み Accepted Deviations）に到達した直後の新しいステップとして「primary ADR baseline diff コメント投稿」を追加する:

- diff の基準は track の **init 刻印コピー**（`track/items/<id>/adr-baseline/` の init-kind 記録。Phase 0 で user が承認した初稿と byte 一致）、比較対象は終端時点の `knowledge/adr/<primary>.md`。
- コメントには終端 diff に加え、track 共有 ledger から `source` が primary ADR の filename と一致する記録だけを選び、その source の init 記録より後を append 順にたどる来歴表を含める。他の ADR source に属する記録は kind が `cite` / `new-adr` / `escalation` のいずれであっても行集合から除外する。選択した `escalation` 行には D3 を満たす reason から得た起点入力と adr-diagnoser 判定の要旨、刻印 hash・timestamp、および刻印を導入したコミットを載せる。選択した `non-semantic-fix` 行には kind、刻印 hash・timestamp、および刻印を導入したコミットを載せ、「非意味的な baseline 不一致の再刻印」と明示する。kind の規範上 reason を持たないため、起点入力や編集判定の要旨を必須欄にせず「記録なし」とも扱わない。
- **fallback**: D3 要件以前の escalation 記録や欠落のある記録に対しては、残る記録の範囲で来歴を組み立て、適用される要件上の要素で復元できないものだけを推測で補わず「記録なし」と明示する。fallback の発動は投稿の失敗事由にしない。
- 終端 diff が空の場合も投稿を省略しない。上記 source 選択後に init より後の ledger 記録がなければ「終端文面は init 刻印と byte 一致、init 後の変更来歴記録なし」と確認する。記録があれば「終端文面は init 刻印と byte 一致。ただし中間変更の記録あり」と確認し、同じ来歴表で終端文面が init と再一致するまでの全記録を示す。終端 diff の空だけから「in-track 編集なし」とは断定しない。
- 投稿は `gh pr comment` による 1 コメントとし、pr-review の review-request 経路（`bin/sotp pr review-cycle`）とは独立させる。自動レビュアーへの再レビュー依頼ではないため、trigger コメントの形式を流用しない。
- このフェーズは自律実行される（Constraint 2 の例外を増やさない）。投稿失敗は commit 済み成果を損なわない非致命エラーとして報告のみ行う。

### D2: コメント先頭に PR author への mention を付ける

diff コメントの先頭に、実行時に解決した PR author（`gh pr view --json author` の login）への `@mention` を置く。merge 裁定者（track PR の作成者 = テンプレートの利用者自身）に GitHub 通知が届き、「レビューが終端に達し、ADR 差分の監査材料が揃った」ことを能動的に知らせる。特定ユーザー名はハードコードしない — テンプレート出力先でもそのまま機能させるため、宛先は実行時解決のみとする。

### D3: escalation 刻印の reason に起点入力と守護者判定の要旨を含める

primary ADR への in-track 編集で escalation 刻印を打つとき、reason には (a) 起点入力の由来と要旨、(b) adr-diagnoser の判定結果の要旨を含めることを要件とする。起点入力の由来は review finding に限定せず、少なくとも local review round、外部 PR review round、spec→ADR の 🔴 signal（該当 spec element と参照）、および diagnose routing（診断入力と ADR へ戻した理由）を区別できなければならない。これは reason の規範「自己完結の欠落説明」の具体化である — 何が欠けていたか（起点入力）と、その修正が決定を保存すると判定された事実（守護者判定）は、欠落説明を自己完結させる構成要素であり、禁止されている user 承認記録の ledger 重複には当たらない。この一文の明確化を `knowledge/conventions/pre-track-adr-authoring.md` の機構整合節に追記する。判定「全文」の保存は要件にしない — 全文はセッション成果物であり、要旨が ledger に残れば終端フェーズの来歴表は機械的に再構成できる。

## Rejected Alternatives

- **PR body への追記セクション**: body は Accepted Deviations の記録先として既に構造を持ち、round ごとの編集は自動レビュアーのパース対象を汚しやすい。時系列の監査材料は追記型のコメントの方が適する。
- **maintainer ハンドルのハードコード**: SoTOHE-core 本体では機能するが、テンプレート出力先で誤った宛先になる。実行時解決に一本化する。
- **手動運用の継続**: 今回の手動投稿が有効だったからこそ、忘れうる手作業として残すのではなく workflow の終了処理に載せる。
- **mention なし**: 通知が届かず、PR を開くまで監査材料の存在に気づけない。D2 の趣旨に反する。
- **reason 要件のみ（fallback なし）**: 要件以前の刻印や書き漏れに遭遇した時点で終端フェーズが成立しなくなる。要件は将来を保証し、fallback は過去を吸収する — 片方では足りない。
- **来歴要件を「残る記録の範囲」へ弱めるのみ**: 守護者判定の揮発という記録欠陥を恒久化し、来歴表の再構成可能性が刻印者の善意に依存し続ける。判定要旨の記録は刻印時なら一行で済む。

## Consequences

### Positive

- merge 裁定者は PR 上で「承認済み初稿からの primary ADR の全乖離とその来歴」を 1 コメントで監査できる。
- 乖離ゼロの場合も終端一致と中間履歴の有無が明示されるため、「途中で変更されなかったのか、変更後に init 文面へ戻ったのか、投稿を忘れたのか」の曖昧さが消える。
- mention により終端到達が能動通知され、merge 待ち時間が短縮される。

### Negative

- adr2pr の終端が GitHub コメント投稿権限に依存する箇所を 1 つ増やす（失敗は非致命として報告のみ）。

### Neutral

- 実装フットプリントは workflow SSoT（`.harness/workflows/track/adr2pr.md`）、両 adapter（`.claude/commands/track/adr2pr.md` / `.agents/skills/track-adr2pr/SKILL.md` の報告形式）、D3 の一文追記（`knowledge/conventions/pre-track-adr-authoring.md` の機構整合節）、`gh pr comment --body-file <path>` 以外を拒否する argv 検証 wrapper（`Makefile.toml` の `pr-audit-comment` task）、および Claude Code 経路の自律実行に必要な template-shipped recommended default allowlist（`.claude/settings.json` の `Bash(gh pr view:*)` / `Bash(cargo make pr-audit-comment:*)`）である。この wrapper は posting task であって CI gate ではなく、allowlist は consumer-owned な permission guidance であるため、Rust source / CI gate wiring に変更はない。reason の書式は自由記述のまま — D3 は含めるべき情報を定めるのであって、構造化 schema を課すのではない。
- 単発の `/track:pr-review` 実行には適用しない。diff の基準となる init 刻印は adr2pr の文脈で意味を持つため、本フェーズは adr2pr の終了処理に限定する。

## Reassess When

- PR author と merge 裁定者が分離する運用（例: bot が PR を作成する）が導入されたとき（D2 の宛先解決を見直す）。
- adr-baseline ledger の構造が変わり、来歴（escalation 刻印と judgment）の機械的な組み立て方が変わったとき。
- diff が大きすぎて 1 コメントに収まらないケースが実際に発生したとき（分割・添付等の表現を検討する）。
- 来歴の機械組み立てが reason の自由記述では不安定と判明したとき（D3 の構造化 — reason の schema 化や判定記録の専用 artifact 化 — を検討する）。

## Related

- `knowledge/conventions/pre-track-adr-authoring.md` — init 刻印・escalation 刻印・守護者判定の規範（来歴の情報源）
- `.harness/workflows/track/adr2pr.md` — 追加先の workflow SSoT
- `.harness/workflows/track/pr-review.md` — 先行する Step 10 の終端条件
