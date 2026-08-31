---
adr_id: "2026-08-29-1030-gate-log-name-feasibility"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-31:delta ADRを承認します。青信号にしてください。"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-08-31:delta ADRを承認します。青信号にしてください。"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-08-31:delta ADRを承認します。青信号にしてください。"
    status: proposed
---
# ゲートログ名の実現可能性は永続化境界が担う

## Context

ゲートのサマリ出力では、子プロセスの実行結果と完全ログの保存先を対応付ける。ログ名の符号化、ファイルシステムのコンポーネント長制限、既存ファイルとの衝突回避は、保存先を選択して実際に作成する永続化処理の性質である。一方、`GateRunCommand` の構築時にこの制限を所有させると、保存先を持たない usecase 層がアダプター固有の制限を判断することになる。

子プロセスを起動する前に長すぎるラベルを拒否する必要がある場合、実行後の永続化失敗で子プロセスの実際の状態を覆い隠してはならない。

## Decision

### D1: ログ名の実現可能性と予約は永続化ポートで扱う

ファイル名レイアウトとファイルシステムのコンポーネント長制限の実現可能性は、`GateLogPersistencePort` とそのアダプターが所有する。`GateRunCommand` の構築はこの制限を所有しない。

子プロセス起動前の拒否が必要なときは、ポートが一意なログパスを選択して作成する予約・準備操作を公開する。`execute` は予約済みパスを用いて子プロセスを実行し、その内容を当該パスへ書き込む。これにより、名前の実現不可能性は起動前に報告され、起動済みの子プロセスの終了状態は保存失敗によって置換されない。

### D2: 永続化の最終公開時に保存先を trusted root 内で再検証する

予約時の確認だけで保存先の包含を保証しない。`persist` は内容を一時領域へ書き、trusted root 配下であることを最終段階として再検証したパスへの publish を最後に行う。予約後に親ディレクトリが移動または置換され、この publish を trusted root 内で完了できなくなった場合は、trusted root 外へ書き込んだ inode を指す語彙上のパスを返さず、`Unavailable` または書込みエラーとして失敗させる。

この境界は、予約から永続化までディレクトリが移動しないという未宣言の実行環境仮定には依存しない。

### D3: 予約は `persist` により消費し、未消費 token の破棄は呼出し側の欠陥とする

`persist` は `GateLogReservation` を消費する。cancel API は設けない。`persist` せずに token を drop することは呼出し側の欠陥であり、アダプターは予約済み名前を TOCTOU に unlink してはならない。`reserve` が作成した空の exclusive file は、後続の一意名再試行まで残ってよい。

一つの `GateRunInteractor::execute` が保持できる live reservation は最大一つとする。アダプターは数値による pending-reservation 上限を新設せず、adapter shutdown 時にも未消費 reservation を暗黙に reclaim する契約を設けない。

## Rejected Alternatives

- **`GateRunCommand` の構築時にエンコード後の名前を検査する**: 保存先とファイルシステム制約を知らない usecase 層へ、境界固有の責務を持ち込む。
- **子プロセス終了後に初めて保存先を作成する**: 名前の失敗が実行結果より後に発生し、子プロセスの実際の状態を覆い隠し得る。
- **予約から永続化まで親ディレクトリが移動しないと仮定する**: 実行環境に属する未宣言の前提であり、予約後の包含を強制できない。
- **token の drop や adapter shutdown で予約済み名を削除する**: 名前の再利用との競合を生み、TOCTOU 安全性を保証できない。

## Consequences

- 良: パス選択・衝突回避・コンポーネント長の判断が永続化境界にまとまり、早期拒否も実現できる。
- 良: 最終公開時の再検証により、予約後のディレクトリ移動または置換が trusted root 外への永続化として成功することを防ぐ。
- 負: 永続化ポートに予約済みパスを表す操作と状態を追加する必要がある。
- 負: 未消費 reservation が作った空ファイルは残り、後続の一意名再試行で扱われる。
- 中立: 実行処理は予約済みパスを入力として受け取り、ログ内容の書込みを継続する。
- 中立: reservation の回収を目的とする cancel、shutdown、または数値上限の契約は設けない。

## Reassess When

- ログ保存先がファイルシステム以外へ拡張され、コンポーネント長という制約が共通でなくなったとき。
- 予約と実行の間に必要な排他または清掃の性質が変わったとき。
- trusted root 内への最終公開を保証できない保存先が必要になったとき。

## Related

- Refines [2026-08-25-0425-gate-output-summary-contract.md#D1](2026-08-25-0425-gate-output-summary-contract.md#d1).
