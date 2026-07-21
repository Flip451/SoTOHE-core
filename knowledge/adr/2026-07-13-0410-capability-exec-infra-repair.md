---
adr_id: 2026-07-13-0410-capability-exec-infra-repair
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 unified_exec を無効化して blocking exec 前提を回復する裁定"
    candidate_selection: "from:[disable-unified-exec-blocking,instruct-running-session-wait] chose:disable-unified-exec-blocking"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 CLI flag を追加し、タイムアウトを指定しない限り時間上限なく動き続けるよう修正する指示"
    candidate_selection: "from:[cli-flag-unlimited-default,raise-constant-only,profile-config-field] chose:cli-flag-unlimited-default"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 層名ハードコード回避の確認と翻訳一元化を最善の修正として実施する裁定"
    candidate_selection: "from:[centralized-cargo-metadata-resolution,import-only-local-patch,config-rustdoc-root-field] chose:centralized-cargo-metadata-resolution"
    status: proposed
---
# 外部 provider 実行基盤の修復

## Context

capability 実行基盤（外部 provider CLI を subprocess として起動する経路）で、次の 3 つの障害が同日に連鎖して露出した。

1. provider CLI の shell tool が、待機時間の上限を超えた長時間コマンドを完了前に「実行中 session」として agent へ返す挙動（PTY-backed exec）を既定で持つようになり、reviewer round を起動した fixer agent が「実行は返ったが結果が無い」状態を誤読して失敗報告する回帰が起きた。capability wrapper（reviewer / fixer dispatch、`cargo make` の gate 群）は、数分単位のコマンドの exit code と stdout が同一 tool call 内で返る blocking 挙動を前提に設計されている。
2. `sotp capability exec` の provider process timeout がコード内定数の 600 秒で、CLI からも設定からも変更できなかった。複数 file の refactor と検証を行う implementer 級の作業は 10 分を普通に超えるため、作業途中で process が kill され、中途状態の working tree が残った。reviewer 経路（`sotp review local`）が 1800 秒 default + CLI flag 可変であるのと非対称だった。
3. `sotp catalog import` の識別照合が「package 名 = rustdoc crate root 名」という lib crate でしか成り立たない前提を持ち、bin-only crate（package 名と `[[bin]]` 名が異なる crate）で構造的に失敗した。catalogue の識別体系は package 名 root、rustdoc JSON の module path は target 名 root という 2 つの命名域があり、その翻訳の知識が横断点ごとに独立実装されていた — rustdoc 実行側は cargo metadata による動的解決、signal 評価器は literal のハードコード対応表、import 照合は未実装、という分散である。

## Decision

### D1: capability wrapper 実行系は blocking shell-tool 前提を維持する

provider CLI の project 設定で PTY-backed exec（実行中 session を返す挙動）を無効化し、shell tool を blocking 挙動に固定する。capability wrapper の実行契約は「コマンドの exit code と stdout が同一 tool call 内で agent に返る」こととする。

blocking 化により、timeout 不足は明示的なエラー（exit 124 と経過時間）として agent に返り、agent は timeout パラメータを付けて自己修正できる。実行中 session の誤読のような silent な失敗より安全側である。

長時間実行が正当なコマンドについては、呼び出し側 agent が shell tool の timeout パラメータを明示する。reviewer 呼び出しを行う capability の運用文書には、reviewer subprocess の正当な実行時間（前段の gate 実行を含む）を踏まえた timeout 下限を明文化する。

### D2: capability exec の provider process timeout は CLI flag で制御し、未指定は無期限とする

`sotp capability exec` に `--timeout-seconds <N>` flag を追加する。flag 未指定のときは時間上限を設けず、provider process の完了まで待ち続ける。コード内定数による一律 timeout は廃止する。

timeout 値は正の整数のみを受け付け、0 は入力検証で拒否する。値は CLI 引数境界から usecase の検証済み型を経由して subprocess 待機処理まで型付きで伝搬させる。

この方式は reviewer 経路（`sotp review local` の `--timeout-seconds`）と対称であり、dispatch ごとに「この作業は長い」という呼び出し側の意図を表現できる。

### D3: package↔rustdoc-root 翻訳は cargo metadata による動的解決の単一機構に一元化する

package 名から rustdoc crate root 名への翻訳を、cargo metadata を情報源とする単一の解決機構に集約する。解決機構は、対象 package の `targets` から lib target があればその唯一の target を選び、lib target が無ければ唯一の bin target を選ぶ。bin target が複数ある場合は package の `default_run` がそのいずれかを指すときだけ選び、`default_run` が無い・いずれにも一致しない・target 名を取得できない場合は metadata の列挙順に依存せず fail-closed とする。

rustdoc 実行に渡す target 名と rustdoc crate root は、選択した target の metadata 上の `name` から導出する。crate root はその `name` の `-` を `_` に正規化した値とし、package 名から推測しない。したがって `[lib] name` が package 名と異なる lib crate も、ハイフンを含む lib / bin target も同じ規則で扱う。rustdoc 識別に触れる全ての箇所 — schema export の rustdoc 実行、`catalog import` の型照合、signal 評価器の identity key 正規化 — はこの機構を共有する。

利用者向けの型 path は catalogue の識別体系どおり package 名 root（例: `cli::commands::…`）を維持し、rustdoc との照合時にのみ root segment を翻訳する。

crate 名や bin 名の literal 対応表をコードに書くことは禁止する。翻訳のための新しい設定 field も導入しない — cargo metadata が既に保持する事実の二重管理になるためである。これにより crate / bin を rename した template 利用者の環境でも翻訳が追従する。

## Rejected Alternatives

### A. PTY-backed exec を有効のまま、agent に実行中 session の待機を指示する

capability 文書への指示追加だけで対処する案。model の遵守に依存する非決定的な対処であり、実際に誤読による失敗が発生した後では機構側の保証が必要。fail-loud な blocking 挙動に固定する方が安全なため却下。

### B. provider process timeout の定数を引き上げるだけにする

600 秒を 1800 秒等へ引き上げれば当面の失敗は減るが、capability ごと・dispatch ごとの適正時間を表現できず、より長い作業で同じ失敗が再発する。呼び出し側制御へ移すため却下。

### C. timeout を capability profile の設定 field にする

capability 単位の既定値は表現できるが、同じ capability でも作業ごとに所要時間は大きく変わるため、dispatch ごとの意図を表現できない。reviewer 経路の CLI flag 方式との対称性も失われるため却下。CLI flag が導入された後に profile 既定値が別途必要になれば、その時点で追加を再検討する。

### D. import 照合だけを局所修正し、評価器の literal 対応表を残す

今回の失敗箇所だけを直す最小修正。しかし「翻訳知識が横断点ごとに独立実装される」という根本原因が残り、literal 対応表は bin 名を rename した利用者環境で壊れる。次に rustdoc 識別へ触れる実装が同じ前提を再発明する経路も閉じないため却下。

### E. rustdoc root 名を設定 file の field として持つ

architecture-rules.json 等に翻訳結果を書く案。cargo metadata が既に知っている事実の二重管理となり、bin 名変更時に設定との drift が生じるため却下。

## Consequences

### Positive

- reviewer / fixer round が結果を記録できないまま失敗する回帰が解消し、timeout 不足は明示エラーとして agent が自己修正できる。
- implementer 級の長時間 capability dispatch が完了まで実行できる。timeout は呼び出し側が dispatch ごとに明示制御できる。
- bin-only crate の既存型 import（reference / modify / delete）が可能になり、該当層の型カタログ作成が正常化する。
- crate / bin を rename した template 利用者環境でも、識別翻訳が動的解決で追従する。

### Negative

- timeout 未指定の dispatch は hang した provider process を自力では終了しないため、呼び出し側（orchestrator）が進行を監視する責務を持つ。
- 翻訳解決のために cargo metadata の呼び出しが増える。
- provider CLI の将来の挙動変更（PTY-backed exec の仕様変化等）に対して、blocking 前提の設定を再評価する必要がある。

### Neutral

- reviewer 経路（`sotp review local` の 1800 秒 default + `--timeout-seconds`）は変更しない。
- catalogue の識別体系（package 名 root、bare 型名 + module_path）は変更しない。

## Reassess When

- provider CLI が実行中 session の完了待機を deterministic に扱える機構を提供し、blocking 固定より優位になったとき。
- timeout 未指定の無期限待機によって orphan process が実運用の問題として観測されたとき。
- rustdoc JSON の crate root 命名規則が変わったとき。
- 1 つの package が複数の bin target を持つ構成が workspace に現れ、bin 選択の解決規則を拡張する必要が出たとき。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/enforce-by-mechanism.md` — 機構による強制の優先
- `knowledge/conventions/prefer-type-safe-abstractions.md` — 型付き伝搬の方針
- `.harness/config/agent-profiles.json` — capability → provider routing の SSoT
