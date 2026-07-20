---
adr_id: "2026-07-20-1508-adr2pr-argless-context-resolution"
decisions:
  - id: decision-1
    user_decision_ref: "chat_segment:session_01KaWuH2sp8TSFmXXax6dJ5c:2026-07-21"
    candidate_selection: "from:[A,B,optional-args-context-resolution] chose:optional-args-context-resolution"
    status: proposed
  - id: decision-2
    user_decision_ref: "chat_segment:session_01KaWuH2sp8TSFmXXax6dJ5c:2026-07-21"
    candidate_selection: "from:[C,D,confirm-then-proceed] chose:confirm-then-proceed"
    status: proposed
  - id: decision-3
    user_decision_ref: "chat_segment:session_01KaWuH2sp8TSFmXXax6dJ5c:2026-07-21"
    candidate_selection: "from:[E,candidate-selection-prompt] chose:candidate-selection-prompt"
    status: proposed
---
# /track:adr2pr の呼び出し型を引数指定から文脈自動解決に戻す

## Context

現行の `/track:adr2pr` は `<feature> --primary-adr <file>.md` の 2 引数を必須とする。
この呼び出し型は `.harness/workflows/track/adr2pr.md` と `.claude/commands/track/adr2pr.md`（および `init.md`）が定義しており、呼び出し型自体を直接定めた既存 ADR はない。

primary ADR の明示指定は ADR-baseline freeze（init 刻印による designation）導入に伴って必須化されたが、実際の運用では adr2pr 起動直前に `/adr:add` 等で対象 ADR を作成・議論しており、feature 名も primary ADR も直近の会話文脈から特定可能なことが多い。引数の手動指定は冗長で、呼び出しの摩擦になっている。

## Decision

### D1: 呼び出し型は引数任意 + 文脈自動解決とする

`/track:adr2pr` 単独で呼び出せるようにし、feature 名・primary ADR は文脈から自動解決する。明示指定（`<feature>` / `--primary-adr <file>.md`）も任意引数として残し、指定された場合はそちらを優先する。

### D2: 自動解決の結果は起動時に 1 回確認してから進行する

会話文脈（直前の `/adr:add` 等）から orchestrator が feature 名と primary ADR を推定し、解決結果を user に 1 回確認してから `/track:init` へ引き渡す。

### D3: 文脈から一意に解決できない場合は候補を提示して選択してもらう

候補が複数ある・候補が見つからない場合は、候補 ADR / feature 名を user に提示し、選択を受けてから進行する。

## Rejected Alternatives

### A. 現行 2 引数必須形式の維持

直前の会話文脈で特定可能な情報の再入力を強制し、呼び出しの摩擦が残るため却下。(vs D1)

### B. 完全引数なし化（明示指定の廃止）

文脈の薄い再開セッション等で明示指定の逃げ道がなくなるため却下。任意引数として残す。(vs D1)

### C. 一意に推定できた場合は確認なしで進行

track 初期化は branch 作成・init 刻印を伴い巻き戻しコストが高いため、起動時 1 回の確認を優先して却下。(vs D2)

### D. 機械的規則（例: 最新の未着手 ADR）による解決

会話文脈と乖離した誤選択のリスクと規則の保守コストがあるため却下。(vs D2)

### E. 曖昧時の fail-closed 停止

候補提示して選択してもらう方が再呼び出しの手間なく安全に進められるため却下。(vs D3)

## Consequences

### Positive

- `/adr:add` → `/track:adr2pr` の流れが引数の再入力なしに接続し、呼び出しの摩擦が減る
- ADR ファイル名の typo など引数指定ミスによる init 失敗がなくなる

### Negative

- 起動時に解決結果の確認 1 回分の対話が増え、呼び出し直後の完全自律性は下がる
- 自動解決ロジックの分だけ workflow SSoT / adapter の記述が複雑になる

### Neutral

- 明示引数は任意として残るため、既存の呼び出し形も引き続き有効

## Reassess When

- 起動時の解決結果確認が運用上冗長と感じられるようになった場合（却下案 C の再検討）
- 文脈自動解決の誤推定が頻発する場合（却下案 D / E の再検討）
- adr2pr を会話文脈のない環境（headless / スケジュール実行等）から起動する運用が始まった場合
- 採用プロジェクト（template 利用側）からのフィードバック

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/pre-track-adr-authoring.md` — pre-track ADR の作成タイミングと `/track:plan` 起動条件
- `.harness/workflows/track/adr2pr.md`, `.harness/workflows/track/init.md` — 呼び出し型の workflow SSoT（本 ADR の実装対象）
- `.claude/commands/track/adr2pr.md`, `.claude/commands/track/init.md` — Claude Code adapter（同上）
