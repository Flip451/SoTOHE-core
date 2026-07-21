# /track:obligation-fulfillment — テスト義務履行ループ

> Operational SSoT: `.harness/workflows/track/obligation-fulfillment.md` — provider 非依存の
> workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 /
> Tool 制約 / 報告形式のみを残す。per-record の著作規律は
> `.harness/capabilities/implementer.md` Step 3 が所有する。

## Invocation

User invokes this command as `/track:obligation-fulfillment`. 引数は不要（track は現在の
`track/<id>` ブランチから解決）。

## Claude Code invocation constraints

- **役割分担**: 著作/修復ラウンドは `implementer` capability に委譲する。briefing にはラウンド
  固有の差分のみを書き、`bin/sotp capability exec implementer --host claude --briefing-file
  <path>` で dispatch する。dispatcher が `.harness/config/agent-profiles.json` から profile
  model と provider-native skill の sandbox を内部解決し、dispatch を完了するか in-host 委譲指示を
  返す。`bin/sotp test-obligation
  evaluate` は **orchestrator（このセッション）が実行**する（host 所有 — 委譲プロバイダの
  sandbox 内では provider verifier subprocess を起動できないため）。
- ループの手順・ゲート条件・file-safety・キャッシュ有効性の規律はすべて workflow SSoT に
  従う（本 adapter では重複させない）。
- **Progress tracking**: ラウンドごとに lane counts（`bin/sotp test-obligation results`）を
  記録して報告する。

## Report format

After execution, summarize:

1. 各ラウンドの lane counts 遷移（pass/fail/pending per lane）と編集セットの規模。
2. 追加・変更したテスト（file + test name）。
3. 最終 `bin/sotp test-obligation check` の結果（exit 0 で完了）。
4. 上流 SoT への routing が必要になった義務（あれば、宛先 writer capability と理由）。
