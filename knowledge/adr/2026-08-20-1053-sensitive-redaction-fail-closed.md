---
adr_id: "2026-08-20-1053-sensitive-redaction-fail-closed"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:grok-adr2pr:2026-08-20 Phase 0 収束文面承認"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:grok-adr2pr:2026-08-20 Phase 0 収束文面承認"
    status: proposed
---
# シークレット秘匿の正規表現を fail-closed にする

## Context

秘匿処理の静的正規表現が、panic 禁止 lint の回避として `LazyLock<Option<Regex>>` + `.ok()` で構築されている。パターンにタイポが入ると redaction が無音で丸ごと無効化され、シークレットが平文で出力に流れる。コメントは「fail-safe」と記すが実態は fail-open であり、セキュリティ境界の原則に反する。sotp バイナリの挙動として consumer にもそのまま出荷される。

## Decision

### D1: 静的正規表現の構築失敗を無音の無効化にしない

静的リテラルの正規表現は `LazyLock<Regex>` とし、当該行に限定した allow 注釈つきの `expect` で構築する。不正な静的パターンはプログラミングエラーであり、秘匿の無音停止より fail-stop が正しい。対象集合は秘匿境界の静的リテラル正規表現に限り、完備は `Regex` 型（`Option<Regex>` ではない）への委譲と、追加リテラルも同一の構築に従うことで取る。fail-open は型として表現できない。これらの静的リテラルには構築検証を併設し、production で `expect` が初出検出になる経路を残さない。

### D2: セキュリティ境界の fail-open 禁止を security 規約に明記する

`security.md` に「セキュリティ境界(秘匿・検証・権限判定)では機能の無音縮退を禁止し、構築・初期化の失敗は停止させる」を追加する。

## Rejected Alternatives

- **`Option` を維持し失敗時にログ警告を出す**: 警告は読まれないことを前提に設計すべきであり、秘匿の無効化と引き換えにできる通知手段ではない。
- **起動時の一括検証コマンドを新設する**: 静的リテラルの構築と構築検証が fail-stop を担うため、起動専用の検証実行面は増やさない。

## Consequences

- 良: パターン誤りは構築時に fail-stop し、シークレットの平文流出経路が閉じる。
- 中立: panic 禁止 lint への限定 allow が 1 箇所増える（正当な例外として注釈で自己記述する）。

## Reassess When

- 秘匿パターンが operator 設定など動的供給になったとき（動的パターンは expect 不可 — 構築失敗を起動時エラーとして報告する別設計が要る）。
