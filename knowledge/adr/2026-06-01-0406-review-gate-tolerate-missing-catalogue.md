---
adr_id: 2026-06-01-0406-review-gate-tolerate-missing-catalogue
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-01:reviewer-runs-without-type-designer-output"
    status: proposed
---
# review / commit ゲートを型カタログ未生成の段階でも通す — active-gate のシグナル評価を欠損入力に寛容にする

## Context

`track-active-gate`(`Makefile.toml`)は `bin/sotp track type-signals && bin/sotp track catalogue-spec-signals && bin/sotp track views sync` というシェルチェーンで、`track-local-review`(reviewer 起動)と `track-commit-message`(commit ゲート)の両方が共有する prerequisite になっている。

このうち先頭の `type-signals` は、対象トラックの層カタログ(例: `domain-types.json`)が存在しないとハードエラーで非ゼロ終了する。型カタログは Phase 2(type-design)で初めて生成されるため、Phase 0(init / ADR baseline)や Phase 1(spec)の段階では存在しない。実際に init 直後の ADR baseline で reviewer を起動したところ、`type-signals evaluation failed for layer 'domain': failed to read catalogue ... domain-types.json: No such file or directory` でチェーンが中断した。

`track-local-review` と `track-commit-message` はどちらも `track-active-gate` に依存しているため、型カタログ生成前は **reviewer も commit ゲートも実行できない**。これは pre-track-adr-authoring が想定する「init 直後に review → commit で ADR を初回 commit する」標準フローを塞いでいる。

一方、同じチェーンの `views sync` はカタログ不在を warning で skip し(例: `contract-map.md` のレンダリングを skip)、ハードエラーにはしない。つまり同一ゲート内で、欠損カタログに対して `type-signals` だけが寛容でないという不整合がある。

## Decision

### D1: active-gate のシグナル評価を欠損入力に寛容にする

`track-active-gate` のシグナル再生成ステップ(`type-signals`、および同じく未生成の上流成果物を読むシグナルステップ)は、評価対象の入力(層カタログ等)が存在しない層・フェーズでは評価を skip(no-op + warning)し、非ゼロ終了しない。挙動を、同じチェーンで既にカタログ不在を skip している `views sync` の寛容さに揃える。

これにより `track-local-review` と `track-commit-message` が型カタログ生成前(Phase 0 / 1)でも成功し、「init → review → commit」の標準フローが通る。

厳格性は維持する。カタログが存在する層では従来どおりシグナルを評価し、🔴 は引き続きゲートを block する。skip は「入力そのものが存在しない」場合に限定し、入力があるのに評価を省くことはしない(fail-open を作らない)。

具体的な修正対象コード(type-signals 評価器の実装位置 / active-gate チェーンの構成)は track 実装で確定する。

## Rejected Alternatives

### A. レビュー/commit から active-gate 依存を丸ごと外す

`track-local-review` / `track-commit-message` から `track-active-gate` 依存自体を削除する案。ゲート全体を bypass すると、カタログが存在する後続フェーズでもシグナル再生成が走らず、reviewer / commit が古いシグナル状態を見る(hash mismatch / fail-open)。skip は「入力不在の層」に限定すべきで、ゲート全体の無効化は危険。却下。

### B. Phase 0 専用の別レビュー経路を新設する

フェーズごとに別コマンド分岐を増やす案。`views sync` と同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。却下。

## Consequences

### Positive

- 型カタログ生成前でも review / commit ゲートが通り、pre-track-adr の標準フロー(init → review → commit)が機能する。
- チェーン内のシグナルステップの欠損入力ハンドリングが `views sync` と一貫する。

### Negative

- 「入力不在なら skip」の境界判定を誤ると、本来評価すべき層を取りこぼす fail-open リスクがある。実装では「カタログが実在するか」を厳密に判定し、存在する層は必ず評価する。

### Neutral

- カタログが揃った後(Phase 2 以降)のゲート挙動は不変。

## Reassess When

- フェーズ構成や TDDD パイプラインが変わり、シグナル評価が前提とする成果物が増減した場合。
- skip 判定が原因でシグナルの取りこぼし(fail-open)が観測された場合。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/pre-track-adr-authoring.md` — init 直後の ADR 初回 commit フロー
- `Makefile.toml` の `track-active-gate` / `track-local-review` / `track-commit-message`
