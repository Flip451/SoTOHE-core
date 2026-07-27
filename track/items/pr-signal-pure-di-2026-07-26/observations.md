# Observations — pr-signal-pure-di-2026-07-26

Free-form observation log. Not a SoT artifact.

## `signal --spec-json` の trusted-root containment (P0 指摘の裁定)

infrastructure scope の review が、`SystemSignalCommandAdapter::spec_path` で `--spec-json`
override に containment 検査が無いことを P0 として報告した。review-fix-lead は
`trusted_workspace_root` + `normalize_and_guard_path` を追加して修正した。

この修正は revert し、baseline 挙動を維持することにした。根拠:

1. 移送元 `apps/cli-composition/src/signal/mod.rs::resolve_spec_json_path` に guard は存在せず、
   doc comment が逆の契約を明文化していた — "When `override_path` is `Some`, the usecase
   short-circuits and returns it verbatim without consulting the reader or `workspace_root`."
2. `--spec-json` は git checkout の外でも動かすための escape hatch として設計されている。
   `signal-gate-strictness-config-2026-06-18` track の review で、override が渡されたら
   workspace 解決を迂回するよう繰り返し要求され、現在の形になった経緯がある。
   workspace root への containment はこの目的と矛盾する。
3. spec CN-01 が CLI 引数 / stdout / stderr / exit code / 永続化結果の不変を無条件に要求する。
   従来受理していたパスを拒否するのは stderr と exit code の変更にあたる。
4. ADR D4 が外部挙動の変更を別 ADR の判断事項として留保している。

パスを選ぶのは operator 自身が打つ引数であり、sotp は operator 権限で動くため特権境界を跨がない。
将来的に override の受理範囲を狭めるなら、traversal / symlink / 相対パス解決、拒否時の
stdout・stderr・exit code、既存利用者の移行方針まで含めて別 ADR で決める。

`/track:diagnose` を 2 回起動し、いずれも `routing_target: adr` を返した。上記はユーザー裁定。

なお、同じ経路で usecase review が報告した CN-01 のエラー identity 喪失
(`SignalCommandPortError` / `SignalGateConfigError` が opaque な単一 variant で、baseline の
`[BLOCKED] cannot discover git repository: ...` / `[BLOCKED] cannot resolve spec.json from active
track: ...` を再現できない) は本 track の対象であり、type-designer による catalogue 改訂で対応する。

## T008 の task-contract attribution

`task-contract.json` の task→entry は complete relation であり、entry ごとの単一帰属を要求しない。
この track では T002/T004 が Signal boundary と adapters を初回導入し、T008 が同じ entry 群の
active-track preflight と error-contract を後続改訂する。したがって重複 attribution は、各 task が
実際に変更する catalogue entry を示す意図的な履歴であり、T008 から除去してはならない。
