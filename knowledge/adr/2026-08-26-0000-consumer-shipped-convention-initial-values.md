---
adr_id: "2026-08-26-0000-consumer-shipped-convention-initial-values"
decisions:
  - id: D1
    review_finding_ref: "ref_verify:chain1:2026-08-26:AC-17-AC-18-overlay-shipping"
    status: proposed
---
# consumer 所有の初期規約にも改訂済みの規範を出荷する

## Context

`knowledge/conventions/` は consumer 所有の export 境界であり、テンプレートは
`overlay/knowledge/conventions/` を consumer が改稿できる初期値として供給する。
一方、規約を機構と突き合わせる決定は workspace 側の規約に規範的な要求と強制機構の
対応注記を加え、環境前提のための枠も定めた。初期値がこの改訂と異なれば、新しく作られる
consumer は同じ規範と強制機構の説明を受け取れない。

consumer 所有は、出力時の初期値を consumer が改稿できることを意味する。出力時に
テンプレートが自身の改訂済み規範を初期値へ反映しない理由にはならない。

## Decision

### D1: consumer 所有の初期規約は workspace の改訂済み規範と対応させる

`overlay/knowledge/conventions/` の初期値は、対応する workspace の規約と同じ規範的な
要求および各要求の強制機構の対応注記を含む。相違は consumer が読むための所有権説明、
プロジェクト固有の値を置かない説明、その他の consumer 向けの表現に限る。規範的な要求
または強制機構の対応を省略・変更してはならない。

環境前提の初期値は、宣言の枠と記入指針だけを含める。platform、protocol、encoding、
resource limit、concurrency model その他のプロジェクト固有の前提を既定値として含めない。

新しいテンプレート出力は、この対応を満たす改訂済みの初期規約集合を出荷する。出力後の
consumer は初期値を自らの責任で改稿、改名、削除できる。

出荷する初期値には、現在改訂された structure-required port の必要性テストの例外と、
検証済み Command / Query の規則も含める。

対応を確認する対象は、workspace で改訂または追加した規約と対になる
`overlay/knowledge/conventions/` の有限の文書対である。完全性はこの有限の文書対の
レビューで判断し、出荷するファイルの被覆は `bin/sotp template check-convention-shipping`
で確認する。機械的な内容差分比較や、将来追加される文書についての証明は求めない。

## Rejected Alternatives

- **workspace 規約だけを改訂し、初期値は別内容のままにする**: 新しい consumer が改訂済みの
  規範と強制機構の説明を受け取れない。
- **初期値を workspace 規約の完全な複製にする**: consumer 所有であること、プロジェクト固有の
  値をテンプレートが定めないことを示せない。
- **環境前提に共通の既定値を記入して出荷する**: consumer が決めるべき前提をテンプレートが
  決めることになる。

## Consequences

- 良: 新しい consumer は、workspace の改訂済み規範と同じ要求および強制機構の説明を初期値として受け取る。
- 良: 環境前提について、テンプレートは宣言の枠と指針を供給し、値の決定は consumer に残る。
- 負: workspace 規約の規範的な改訂には、対応する初期値の更新も必要になる。
- 負: 対応の完全性は有限の文書対のレビューに委ね、出荷するファイルの被覆は
  `bin/sotp template check-convention-shipping` の結果に依存する。

## Reassess When

- consumer 向けの表現だけでは workspace 規約と初期値の規範的な対応を保てない規約が現れたとき。
- consumer 所有の初期値に含める規約の範囲を変更する判断が必要になったとき。

## Related

- `knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md` D1・D3・D5・D6・D7 — 本決定が改訂関係を結ぶ各連鎖の先頭。強制機構の対応注記、規約改訂、環境前提の枠を consumer 所有の初期値にもこの決定の範囲で届ける。
- `knowledge/adr/2026-08-25-2239-required-ports-exempt-from-necessity-test.md` D1 — 元の ADR の D2 の連鎖の現在の先頭。structure-required port の必要性テストの例外を定める。
- `knowledge/adr/2026-08-25-1021-validated-usecase-input-boundaries.md` D1 — 元の ADR の D4 の連鎖の現在の先頭。検証済み Command / Query の規則を定める。
- `knowledge/adr/2026-07-24-0326-consumer-convention-ownership-and-harness-decoupling.md` D1・D3 — consumer 所有の overlay 初期値と、その出力境界を定める先行決定。
- `.harness/policies/consumer-ownership.md` — consumer 所有の内容はテンプレートが初期値と案内を供給し、consumer 自身が決めるという関係を定める policy。
