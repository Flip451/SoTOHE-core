---
adr_id: "2026-08-15-1302-composition-root-pure-di-port-granularity"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:current-task:2026-08-15:pure-di-port-granularity-hearing"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:current-task:2026-08-15:pure-di-port-granularity-hearing"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:current-task:2026-08-15:pure-di-port-granularity-hearing"
    status: proposed
---
# 純 DI 化における usecase 契約の粒度を確定する

## Context

`knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md` D9 は、Interactor と Application Service を `libs/usecase` 内で役割分離すると定めたが、**契約の粒度**を決めていない。

そのため実走では、composition root の実行メソッド群をそのまま usecase 側の 1 trait へ移す設計が提案され、型レビューが「facade の作り直しであって純 DI 化ではない」として却下し、track が収束しなかった。

現行 `TrackService` はこの未決状態を体現している。1 trait に 16 の実行メソッドが並び、各メソッドは位置引数の `PathBuf` / `String` / `Option<String>` を取り、戻り値は整形済み文字列を保持する `TrackCommandOutput` である。

粒度と公開面の型を決めない限り、移行先の形が定まらず、同じ却下が各 track で繰り返される。

## Decision

### D1: 1 CLI サブコマンド = 1 ユースケース = 1 入力ポート

入力ポートは 1 ユースケースにつき 1 trait とし、実行メソッドを 1 つだけ持つ。

複数ユースケースを束ねる `*Service` 様式の入力ポートを新設しない。既存のそれは移行時に解体する。

サブコマンドの選択（引数解析結果から実行対象を決める分岐）は primary adapter の責務であり、usecase 層にも composition root にも置かない。

読み取り専用のユースケースも同じ規則に従う。問い合わせ系をまとめる緩和は本決定では採らず、必要性が生じた時点で別 ADR で判断する。

本決定は前掲イニシアチブ ADR の D9 を refine する。同 D9 の crate topology 不変は維持する。

### D2: ポートの公開面から presentation を除去する

入力は検証済みの Command 型 1 個を受け取る。位置引数で未検証の primitive を並べない。

出力は `Result<結果型, エラー型>` とする。stdout / stderr に相当する整形済み文字列を返す型を usecase の公開面に置かない。

文字列整形と exit code への写像は primary adapter（`cli_driver`）が行う。

### D3: 適用範囲は純 DI 移行が触る文脈に限り、強制は catalogue lint に委譲する

D1・D2 は、純 DI 移行が実際に改修する command 文脈の usecase ポートにのみ適用する。移行対象外のポートを本決定を理由に改修しない。

規則の検査は catalogue lint に委ねる。ポートの列挙・網羅・違反検出は lint の機構が持つ責務であり、本決定はそのための新規走査を要求しない。lint が検出しない逸脱は reviewer の判断に委ね、網羅性を ADR の義務としない。

## Rejected Alternatives

- **A. command 文脈ごとの `*Service` ポートを維持する**: 変更量は最小だが、クライアントが呼ばないメソッドへの依存（インタフェース分離原則違反）とテストダブルの肥大が残り、composition root の facade を usecase へ移設しただけになる。純 DI 化の目的を達しない。
- **B. ポート公開面に stdout / stderr 型を残す**: 整形の所在が変わらないため `cli_driver` が空洞のままになり、6 crate 分割の存在意義を損なう。
- **C. ポート粒度を source-level の専用検査で強制する**: rustdoc 走査相当の新規実装が必要で、発火が実装後の CI まで遅れる。既存 lint への委譲が実装コストと発火時期の双方で上回る。
- **D. リポジトリ全 usecase ポートへ一斉適用する**: 移行と無関係の改修が混ざり、レビュー論点と回帰原因を分離できない。

## Consequences

- Good: 移行先の形が一意に決まり、型設計フェーズで粒度論争が再発しない。
- Good: ユースケース単位でテストダブルが小さくなり、変更理由がポート単位に分離される。
- Good: `cli_driver` が整形とエラー写像の実責務を持ち、層の存在意義が回復する。
- Bad: ポート trait と Command / 結果 / エラー型の定義数が増え、初期の記述量が増える。
- Bad: composition root の配線メソッド数がサブコマンド数まで増える。合成根は全体を知る唯一の場所であり許容するが、ファイルは大きくなる。
- Neutral: CLI の外部観測可能な契約は変更しない。crate topology も変更しない。

## Reassess When

- 問い合わせ系ユースケースで Command / 結果型の定義が重複し、維持コストが実測で問題になったとき。
- catalogue lint が正当なポート定義を反復して誤検出するとき。
- CLI 以外の delivery（TUI / daemon 等）が加わり、サブコマンドとユースケースの 1 対 1 対応が崩れるとき。

## Related

- `knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md` — 純 DI 境界規則の正本
- `knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md` — 本 ADR が D9 を refine する対象
- `knowledge/conventions/type-designer-kind-selection.md` — R1 の役割別配置規則
- `.harness/catalogue-lint/config.json` — D3 の検査委譲先
