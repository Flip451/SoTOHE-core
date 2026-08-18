---
adr_id: "2026-08-02-0151-multi-provider-capability-routing"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-08-02"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-08-02"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-08-02"
    status: proposed
---
# capability routing を中華系プロバイダーへ拡張可能にする

## Context

SoTOHE はテンプレートであり、capability → provider routing（`.harness/config/agent-profiles.json`）の選択肢を広げることは template consumer の採用可能性に直結する。DeepSeek / GLM（Z.ai）/ Qwen（DashScope）はいずれも OpenAI 互換 API を提供し、Anthropic 互換エンドポイントも 3 社とも存在する（DeepSeek: 公式 Codex CLI 設定文書あり / GLM: `api.z.ai/api/anthropic` / Qwen: DashScope claude-code-proxy — ただし Qwen には Claude Code 新版の mid-conversation system message を拒否する既知 gap がある）。

現状の sotp は provider=codex のとき `-m <model>` しか発行せず、Codex CLI の custom model provider（`model_providers.<id>` + `base_url`）を capability 単位で選択できない。混在 routing（一部レーンを外部プロバイダーへ）には dispatch 側の拡張が必要である。

## Decision

### D1: capability profile に `model_provider` を追加し、Codex custom provider 経路を第一級にする

capability profile schema に optional な `model_provider` フィールドを追加し、指定時は `--config model_provider="<id>"` を発行する。provider 名は Codex `config.toml` の `model_providers` 宣言と対応し、sotp はその意味論を解釈しない（値の pass-through + 非空検証のみ）。sotp の provider 名は `codex` のまま維持されるため、typed pipeline の provider gate はそのまま機能する。capability ごとの混在 routing（例: fast lane のみ外部プロバイダー）を可能にする。

### D2: 対応プロバイダーの設定例を template として同梱し、検証済み状態を文書化する

DeepSeek / GLM / Qwen の Codex custom provider 設定例（`config.toml` 断片）と、Anthropic 互換経路の設定・注意点（per-subprocess env 注入の必要性、delegate-in-host は redirect されないこと、Qwen の system message gap）を consumer 向け文書として同梱する。各プロバイダーの typed-pipeline 準拠（verdict envelope / structured output）は未検証である旨を明記し、検証済みになったものから文書上のステータスを更新する。

### D3: データ所在の判断は consumer-owned とする

ソースコード・briefing・diff が外部（PRC ホスト含む）サーバーへ送信される判断は、permissions allowlist と同じ所有権構図とする: SoTOHE は選択肢と文書化されたリスクを提供し、採否は consumer の責任。CI による強制は行わず、既定 profile は外部プロバイダーを指さない。

## Rejected Alternatives

### A: グローバル env（`ANTHROPIC_BASE_URL` 等）による一括 redirect

orchestrator 自身のセッションまで redirect され、delegate-in-host 経路は逆に redirect されない、という非対称な事故面を持つため、正規経路としては却下（per-subprocess 注入の設定例として文書化のみ）。

### B: sotp に各プロバイダーの API クライアントを直接実装する

Codex CLI / Claude CLI が既に互換レイヤを提供しており、sotp 側の実装・保守が純増になるため却下。sotp は routing 宣言の pass-through に徹する。

### C: open weights の self-host 経路

多 GPU serving の運用負担が template の範囲を超えるため現時点では対象外（将来の Reassess 対象）。

## Consequences

- 良: template consumer が capability 単位でプロバイダーを選べるようになり、subscription レート枠の逃し弁（高頻度レーンの従量オフロード）としても機能する。
- 良: capability インターフェース契約（別 ADR）の「引き出し方 = provider アダプタ層」の設計と整合し、G24 実装の provider binding 拡張点を先に具体化する形になる。
- 負: 対応プロバイダーの互換性追跡（endpoint 仕様変更・gap の解消状況）という継続的な文書保守が生まれる。
- 中立: 品質・準拠検証（typed pipeline の verdict envelope 等）はプロバイダーごとの trial が必要で、既存の retry → fail-closed 規則の下で行う。

## Reassess When

- Qwen の Anthropic 互換 gap が解消され、Claude 経路を推奨可能になったとき。
- いずれかのプロバイダーが open weights の実用的な self-host 経路を提供し、データ所在の懸念自体を解消できるとき。
- capability インターフェース契約（G24）の実装が provider binding を宣言化し、本 ADR の pass-through 方式を置き換えるとき。
