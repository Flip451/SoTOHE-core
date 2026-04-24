# Enforce by Mechanism Convention

## Purpose

重要な project rule を CI gate / schema validation / hook / codec validation 等の機械的 mechanism で
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
- 適用外:
  - style preference (formatter / linter で十分なもの)
  - ユーザーの非構造化対話中の都度ガイダンス
  - 探索段階での一時的な判断基準

## Rules

- **新ルール提案時は対応する enforcement mechanism の設計を同 ADR / 同 track 内で示す**。
  mechanism 未整備なら ADR に Reassess When として mechanism 整備 trigger を記録する
- **既存ルールで mechanism 未整備のものは、運用データで drift 発生が確認されたら mechanism 整備を
  優先する**。単に文書で強化するのは drift 解決にならない
- **enforcement mechanism の優先順位 (fail-closed priority order)**:
  1. 型システム / schema validation (コンパイル時 or decode 時 error、最も強力)
  2. CI gate (pre-commit / `cargo make ci` / merge gate、exit code で block)
  3. hook (Claude Code hook / git hook、操作前の guard)
  4. lint / static analysis (clippy / deny / custom lint)
  5. documentation + semantic review (reviewer capability による convention 整合性確認 / harness-policy scope
     review、最も弱い — meta-level の自己参照や人間 judgment が必要な領域でのみ許容)
- **memory / prompt / ad-hoc convention のみで管理しているルールは、「運用負担 > enforcement
  benefit」になった時点で整備候補とする**
- **mechanism で強制するルールは documentation で reviewer / author が読み取れる状態にもする**
  (mechanism と documentation は両立、mechanism のみでは意図が不明になる)

## Examples

- Good: `deny.toml` と `architecture-rules.json` による layer 依存の機械的検証
  (`cargo make check-layers`、CI gate レベル)
- Good: signal 評価結果の CI gate 化 (ADR `2026-04-18-1400-tddd-ci-gate-and-signals-separation` §D2 /
  §D5、pre-commit 自動再計算 + stale 検出)
- Good: `/track:plan` state machine での gate 自動評価 + back-and-forth
  (ADR `2026-04-19-1242-plan-artifact-workflow-restructure` §D0.1)
- Good: schema-version bump で旧 schema を decode 拒否する codec
  (型システムレベル、no-backward-compat convention と組み合わせ)
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
- **人間の judgment call が必要な domain knowledge** (コード style、レビュー強度判断、設計 trade-off
  の選択等) は mechanism 化を強制しない。これらは文書 + 人間 reviewer の責務
- **mechanism 整備の cost が enforcement benefit を明確に上回る稀な規模のルール** は convention + 人手
  review で代替する (但し convention に根拠を明示)
- **本 convention 自身の enforcement** は meta-level の自己参照となるため、Rules §3 の fail-closed
  priority order の 5 段階目 (documentation + semantic review) で担保する:
  - convention 変更 (`knowledge/conventions/**`) および harness policy を定義するコマンドファイル
    (`.claude/commands/**`) は harness-policy scope (`track/review-scope.json`) の review 対象であり、
    各 track の review サイクル内で `/track:review` →
    `cargo make track-local-review -- --group harness-policy` 経由で reviewer capability
    (`.harness/config/agent-profiles.json::capabilities.reviewer`) が本 convention への違反を指摘する。
    この review は自動ではなく、track ごとの review 実行時に有効になる
  - ADR 変更 (`knowledge/adr/**`) は plan-artifacts scope の review 対象であり、semantic-review
    (contradiction / factual error / infeasibility の検出) を通じて本 convention と矛盾する ADR を
    間接的に検出できる (tier 5 の範囲内の保証)
  - **Reassess trigger (mechanism 昇格の検討条件)**: (a) ADR author が `/adr:add` 実施時に本 convention
    を cite していないことを adr-editor / reviewer が繰り返し観測した場合 (pre-merge の human observation
    — plan-artifacts semantic review は citation 不在を自動検出しない)、(b) 本 convention に違反する
    merge が通過した事例が発生した場合、(c) ADR template に `## Mechanism` セクションの強制 schema 化を
    要望する提案が出た場合 — いずれかの trigger 発生時に、ADR validator / convention structural CI check
    等の higher-tier mechanism 化を別 ADR で検討する

## Review Checklist

- [ ] 新 rule 提案に対応する enforcement mechanism が ADR / track で明示されているか
- [ ] memory / prompt / ad-hoc convention のみで依存しているルールを発見したら、整備候補として
      TODO.md / Reassess When に記録しているか
- [ ] 選択した mechanism が fail-closed priority order で可能な限り上位のものか
- [ ] mechanism 未整備 rule の整備 trigger (Reassess When) が記録されているか
- [ ] mechanism と documentation が両立しているか (mechanism のみで意図が不明になっていないか)

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引
- [workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 形式手順の最小化原則
  (本 convention と相補: 残す手順は必ず機械強制する)
- [no-backward-compat.md](./no-backward-compat.md) — schema 変更時の遡及非適用と暫定 layer 排除
  (mechanism による強制と合わせて drift 予防を二重に担保)
