---
adr_id: "2026-07-21-1522-agent-md-capability-exec-routing"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    candidate_selection: "from:[A] chose:none (D1 は description 明記案そのもの)"
    status: proposed
---
# .claude/agents の description に capability exec 経由を明記する

## Context

orchestrator が capability を Agent tool で直接呼び出すと、`.harness/config/agent-profiles.json` の provider / model 解決を経由しない。このバイパスにより、host が Claude のときに暗黙に Opus が使われるなど、profile が指定する provider / model と異なる実行が起きがちである。capability 呼び出しの provider routing SSoT は `agent-profiles.json` であり、その解決を内包する正規経路は `bin/sotp capability exec` である。

## Decision

### D1: .claude/agents 配下の description に capability exec 経由を明記する

capability に対応する `.claude/agents/` 配下の md ファイルの description に、「この agent は `bin/sotp capability exec` を経由して呼び出す（Agent tool で直接呼び出さない）」旨を明記する。orchestrator が agent 選択時に description を参照した時点で、直接呼び出しではなく profile 解決を内包した正規経路へ誘導される。

## Rejected Alternatives

### A. hook による直接 Agent 呼び出しの機械的 block

capability 相当の agent への直接 Agent tool 呼び出しを hook で機械的に block する案。capability 相当 agent かどうかの判定が複雑で誤検知リスクが高く、まずは description 明記で足りる（効果不十分なら後で再検討可）ため却下。

## Consequences

### Positive

- agent 選択時に description が目に入り、profile 解決を経由した正しい provider / model で実行される。
- 機械実装が不要で、即日適用できる。

### Negative

- 文書ベースの拘束であり強制力は弱い（enforce-by-mechanism 階層の最下位）。

## Reassess When

- description 明記後もバイパス（直接 Agent 呼び出し）が再発したとき（hook 等の機械的 enforcement を再検討する）。
- capability dispatch の仕組み（`bin/sotp capability exec` / `agent-profiles.json`）が大きく変わったとき。

## Related

- `knowledge/adr/` — ADR 索引
- `.harness/config/agent-profiles.json` — capability → provider routing SSoT
- `knowledge/conventions/enforce-by-mechanism.md`
