# Enforce by Mechanism Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。テンプレートは初期値として供給するが、以後の改稿・改名・削除は利用プロジェクトの裁量である。ハーネスが所有するのは、本文が参照する既存の検査・レビュー機構とワークフローの実装だけであり、どの規則をどの機構で強制するかという方針そのものはこの文書にある。

この文書を初期値として採用した後も、プロジェクトの規模や運用データに応じて内容を見直してよい。

## Purpose

このプロジェクトの重要な project rule を CI gate / schema validation / hook / codec validation 等の機械的 mechanism で
強制する。文書 / prompt / AI agent memory による指示のみに依存するルールは drift しやすく、AI agent
の記憶揺らぎ、人間の注意漏れ、repo 進化に伴う指示の陳腐化で効力を失う。重要度と drift コストが高い
rule ほど mechanism 化の優先度を上げる。

## Scope

- 適用対象:
  - アーキテクチャ layer 境界 (依存方向、pub 境界)
  - type 契約 (TDDD カタログ、signal 評価)
  - workflow phase 遷移 (track lifecycle、spec / 型 / impl plan 順序)
  - security-critical な禁止事項 (直接 git 操作、シークレット hardcode、シンボリックリンク経由攻撃)
  - 成果物の整合性 (hash drift、schema violation、参照整合性)

> **強制先**: review 観点 — harness-policy scope

- 適用外:
  - style preference (formatter / linter で十分なもの)
  - ユーザーの非構造化対話中の都度ガイダンス
  - 探索段階での一時的な判断基準

> **強制先**: review 観点 — harness-policy scope

## D1: 強制先注記

`knowledge/conventions/` 配下の規範的な要求（義務・禁止・許容条件・確認項目）には、要求の直後に
次の形式で強制先を 1 つ注記する。注記のない要求は許さない。

> **強制先**: `<taxonomy>` — `<既存の機構名、review scope、または強制しない理由>`

`<taxonomy>` は次の 4 値から選ぶ。

- `機械 lint`: 既存の lint または機械的な検査が判定する要求
- `宣言突合 (catalogue + verify)`: 宣言した成果物と verify の突合が判定する要求
- `review 観点`: reviewer capability と review scope が判定する要求
- `強制なし (明記)`: 現時点で対応する機構を置かず、その判断を明記する要求

注記には新しい機構の説明を足さず、既存の `cargo make` task、`bin/sotp` の既存 subcommand、または
review scope / workflow の名前を記載する。現時点の有限な文書集合に対する注記の完全性は、
`harness-policy` scope の review で判定する。

> **強制先**: review 観点 — harness-policy scope

## Rules

- **新ルール提案時は対応する enforcement mechanism の設計を同 ADR / 同 track 内で示す**。
  mechanism 未整備なら ADR に Reassess When として mechanism 整備 trigger を記録する
  > **強制先**: review 観点 — adr / harness-policy scope
- **既存ルールで mechanism 未整備のものは、運用データで drift 発生が確認されたら mechanism 整備を
  優先する**。単に文書で強化するのは drift 解決にならない
  > **強制先**: review 観点 — harness-policy scope
- **enforcement mechanism の優先順位 (fail-closed priority order)**:
  1. 型システム / schema validation (コンパイル時 or decode 時 error、最も強力)
  2. CI gate (pre-commit / `cargo make ci` / merge gate、exit code で block)
  3. hook (Claude Code hook / git hook、操作前の guard)
  4. lint / static analysis (clippy / deny / custom lint)
  5. documentation + semantic review (reviewer capability による convention 整合性確認 / harness-policy scope
     review、最も弱い — meta-level の自己参照や人間 judgment が必要な領域でのみ許容)
  > **強制先**: review 観点 — harness-policy scope
- **memory / prompt / ad-hoc convention のみで管理しているルールは、「運用負担 > enforcement
  benefit」になった時点で整備候補とする**
  > **強制先**: review 観点 — harness-policy scope
- **mechanism で強制するルールは documentation で reviewer / author が読み取れる状態にもする**
  (mechanism と documentation は両立、mechanism のみでは意図が不明になる)
  > **強制先**: review 観点 — harness-policy scope

## Examples

- Good: `deny.toml` と `architecture-rules.json` による layer 依存の機械的検証
  (`cargo make check-layers`、CI gate レベル)
- Good: signal 評価結果の CI gate 化（pre-commit での自動再計算と stale 検出を含む）
- Good: `/track:plan` state machine での gate 自動評価 + back-and-forth
- Good: schema-version bump で旧 schema を decode 拒否する codec
  (型システムレベルの互換性方針と組み合わせ)
- Bad: AI agent memory のみで「commit 前に X を確認」と指示し、CI gate や hook で検出していない
  (agent の context 取り違えで失効)
- Bad: review convention 文書で禁止事項を記載しても、reviewer capability の briefing に掲載するのみで
  mechanism がない (prompt engineering に依存、推論結果が変動)
- Bad: naming convention を README に書いただけで、renamed 型名が CI に引っ掛からない
  (drift 検出 zero、次の commit で破綻)

## Exceptions

- **探索段階の drafting / rapid prototyping** では mechanism 整備を後置しても良い。ただし ADR /
  convention に mechanism 整備の Reassess When を明記する (「prototype 完了時」「adoption が 2 件超えた
  時」等)
  > **強制先**: review 観点 — adr / harness-policy scope
- **人間の judgment call が必要な domain knowledge** (コード style、レビュー強度判断、設計 trade-off
  の選択等) は mechanism 化を強制しない。これらは文書 + 人間 reviewer の責務
  > **強制先**: 強制なし (明記) — domain knowledge の judgment call
- **mechanism 整備の cost が enforcement benefit を明確に上回る稀な規模のルール** は convention + 人手
  review で代替する (但し convention に根拠を明示)
  > **強制先**: review 観点 — harness-policy scope
- **本 convention 自身の enforcement** は meta-level の自己参照となるため、Rules §3 の fail-closed
  priority order の 5 段階目 (documentation + semantic review) で担保する:
  - convention 変更 (`knowledge/conventions/**`) および harness policy を定義するコマンドファイル
    (`.claude/commands/**`) は harness-policy scope (`.harness/config/review-scope.json`) の review 対象であり、
    各 track の review サイクル内で `/track:review` →
    `cargo make track-local-review -- --round-type final --group harness-policy --briefing-file tmp/reviewer-runtime/briefing-harness-policy.md` 経由で reviewer capability
    (`.harness/config/agent-profiles.json::capabilities.reviewer`) が本 convention への違反を指摘する。
    この review は自動ではなく、track ごとの review 実行時に有効になる
  > **強制先**: review 観点 — cargo make track-local-review -- --round-type final --group harness-policy --briefing-file tmp/reviewer-runtime/briefing-harness-policy.md
  - ADR 変更 (`knowledge/adr/**`) は `adr` scope の review 対象であり、semantic-review
    (decision underspecification / inconsistent decisions / rejected-alternative regression /
    research grounding mismatch / scope leakage 等の ADR decision-soundness 検出) を通じて
    本 convention と矛盾する ADR を間接的に検出できる (tier 5 の範囲内の保証)
  > **強制先**: review 観点 — adr scope
  - **Reassess trigger (mechanism 昇格の検討条件)**: (a) ADR author が `/adr:add` 実施時に本 convention
    を cite していないことを adr-editor / reviewer が繰り返し観測した場合 (pre-merge の human observation
    — `adr` scope semantic review は citation 不在を自動検出しない)、(b) 本 convention に違反する
    merge が通過した事例が発生した場合、(c) ADR template に `## Mechanism` セクションの強制 schema 化を
    要望する提案が出た場合 — いずれかの trigger 発生時に、ADR validator / convention structural CI check
    等の higher-tier mechanism 化を別 ADR で検討する
  > **強制先**: review 観点 — adr / harness-policy scope

## Review Checklist

- [ ] 新 rule 提案に対応する enforcement mechanism が ADR / track で明示されているか
  > **強制先**: review 観点 — harness-policy scope
- [ ] memory / prompt / ad-hoc convention のみで依存しているルールを発見したら、整備候補として
      Reassess When に記録しているか
  > **強制先**: review 観点 — harness-policy scope
- [ ] 選択した mechanism が fail-closed priority order で可能な限り上位のものか
  > **強制先**: review 観点 — harness-policy scope
- [ ] mechanism 未整備 rule の整備 trigger (Reassess When) が記録されているか
  > **強制先**: review 観点 — harness-policy scope
- [ ] mechanism と documentation が両立しているか (mechanism のみで意図が不明になっていないか)
  > **強制先**: review 観点 — harness-policy scope

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引
