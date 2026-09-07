---
adr_id: 2026-09-05-0600-hook-explicit-host-input-contract
decisions:
  - id: D1
    user_decision_ref: "chat_segment:2026-09-05:hook-explicit-host-input-contract:phase0-converged-adr:承認"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:2026-09-05:hook-explicit-host-input-contract:phase0-converged-adr:承認"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:2026-09-05:hook-explicit-host-input-contract:phase0-converged-adr:承認"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:2026-09-05:hook-explicit-host-input-contract:phase0-converged-adr:承認"
    status: proposed
---
# hook 接続元の host 明示と入力契約の選択

## Context

Grok の PreToolUse で `tool_name` と `toolName` の両方を含む入力が拒否され、シェル操作が実行前に止まる障害が報告された。実際の入力全体は未採取だが、既存の `parse_hook_envelope` は両キーの存在を検出すると写像不能として拒否する。Claude 形式と Grok 形式の選択をキーの排他的存在に依存させたため、互換フィールドの追加が送信元判定を壊した。

hook の接続設定は接続元を既に知っている。その情報を引数として渡し、入力形式の特徴から接続元を推定する必要をなくす。ユーザーは host の明示必須化を選び、対象を `skill-compliance` にも広げた。

## Decision

### D1: agent hook の接続元は host 引数で明示する

PreToolUse と UserPromptSubmit の `sotp hook dispatch` は `--host claude|codex|grok` を必須とする。未指定と不正値は CLI エラーとして終了コード 2 を返す。Claude の接続設定、Codex と Grok の wrapper は固定の host を渡す。標準の引数順は `hook dispatch --host grok <hook-id>` とする。

JSON のキーから host を自動判定しない。host は入力契約の選択情報であり、送信元の認証や権限付与を意味しない。Git process hook は JSON の agent 封筒を読まない別経路として従来の位置引数を維持し、host 指定との組み合わせを拒否する。

### D2: Rust の入力境界で指定 host の契約を解釈する

Grok の PreToolUse は `toolName` と `toolInput` を読み、Claude 互換の別名フィールドを無視する。別名との一致は要求しない。Grok の既存の terminal と `search_replace` の写像を維持し、未知の Grok ツール、必須値の欠落や型不正は拒否する。選択した形式を解釈できないとき、別形式へフォールバックしない。

Claude と Codex の PreToolUse は既存の snake_case 入力処理を共有し、Codex の `apply_patch` 処理を維持する。正規化後の共通ハンドラーとガードポリシーを維持し、wrapper で JSON を加工しない。

本決定は [Grok provider binding](2026-08-14-1225-grok-provider-binding.md) の D9 を、接続元の選択と入力契約の境界について精緻化する。同決定の Grok 入力の正規化、共通ハンドラーへの合流、写像不能時の拒否を維持する。

### D3: skill-compliance にも host を要求し advisory 動作を維持する

`skill-compliance` にも host を明示する。各 host の現在の共通 `prompt` 処理を再利用し、入力パース失敗時の advisory 動作を維持する。接続設定の `|| exit 0` を維持し、host 指定ミスや CLI 起動失敗でも UserPromptSubmit をブロックしない。CLI の入力エラーと接続設定が保障する advisory 動作を区別する。

### D4: 新バイナリと接続設定を一組で導入する

シェルを実行できる環境で `cargo make build-sotp` により新バイナリを構築し、新接続設定と一組で切り替える。旧バイナリ向けの自動判定や wrapper の JSON 加工による互換経路は追加しない。

## Rejected Alternatives

- A: `toolName` があれば Grok と推定する。互換フィールドなどの入力形式変更に送信元判定が左右されるため採用しない。
- B: host 未指定時の自動判定を恒久的に残す。接続元が既知の入口で明示する契約を選んだため採用しない。
- C: wrapper で Claude 互換フィールドを除去して旧バイナリを使う。新バイナリと接続設定を一組で導入する方式を選び、入力解釈を Rust の境界に集約するため採用しない。
- D: `skill-compliance` を host 必須化の対象外にする。ユーザーが UserPromptSubmit についても接続元を明示する方針を選んだため採用しない。

## Consequences

- 正: 互換フィールドが追加されても接続元の選択が変わらず、両形式が併存した場合にどちらの値を検査するかが明確になる。
- 正: 共通ハンドラーへの正規化と既存ガードを再利用できる。
- 負: host 未指定の agent hook 呼び出しは互換性を失い、接続設定とバイナリの移行が必要になる。
- 中立: `skill-compliance` の接続設定は CLI エラーを吸収するため、指定ミスは操作の停止ではなく直接呼び出しの検証で検出する。

## Reassess When

- 接続設定で接続元を固定して特定できなくなる、または host が送信元の認証や権限付与の根拠を必要とする場合。
- Claude または Codex の snake_case 入力、あるいは Grok の `toolName` と `toolInput` の入力契約を、選択済み host ごとに正確に解釈できなくなる場合。
- Git process hook が JSON の agent 封筒を解釈する必要が生じる場合。
- 新バイナリと接続設定を一組で切り替えられない配布条件が生じる場合。

## Related

- [grok を第三の provider binding として追加する](2026-08-14-1225-grok-provider-binding.md) D9 — 入力契約の選択を精緻化する対象。
