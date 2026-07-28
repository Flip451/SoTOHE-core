---
adr_id: "2026-07-28-1024-catalogue-scope-is-implementation-delta"
decisions:
  - id: D1
    review_finding_ref: "user_adjudication:current-task:2026-07-28:D6-scope-correction"
    status: proposed
  - id: D2
    review_finding_ref: "user_adjudication:current-task:2026-07-28:D6-scope-correction"
    status: proposed
  - id: D3
    review_finding_ref: "user_adjudication:current-task:2026-07-28:D6-scope-correction"
    status: proposed
---
# catalogue の適用範囲を実装の追加・変更に一致させる

## Context

type catalogue は、仕様から型契約を経て実装へ至る整合性を、その変更で設計する実装成分について検証するための機構である。既存の実装成分を repository 全体の inventory として列挙し、そのすべてへ新しい契約を遡及的に与える機構ではない。

cargo feature の有効化によって、それまで抽出面に現れなかった既存 public 要素が観測可能になることがある。しかし、抽出面への可視性と、実装変更における新規性は別の性質である。feature の宣言は観測レンズを変えるが、そのレンズから見える既存コードを宣言者が設計または実装したことにはならない。

可視化された既存要素に entry 固有の仕様上の根拠がなく、全体の完了条件だけを根拠として catalogue entry を作らざるを得ない場合、それは catalogue が結ぶべき設計契約が存在しないことを示す。観測可能になったという理由だけで catalogue への宣言を要求すると、可視性を実装の新規性と同一視することになる。

一度も観測されていなかったコードほど乖離している可能性が高いという懸念は、監査上は正当である。ただし、それは既存コードの drift を検出する機構が扱う関心であり、catalogue の適合契約へ遡及的な責任として持ち込むべきものではない。

## Decision

### D1: catalogue の適合対象を実装の追加・変更に限定する

catalogue は、変更が追加または変更する実装成分について、仕様から型契約を経て実装へ至る整合性を検証する。変更が設計せず、追加も変更もしない既存の実装成分は、観測可能であることだけを理由に catalogue の適合対象へ含めない。

本決定は `2026-07-27-0039-tddd-track-scoped-feature-declaration.md` D6 の scope を訂正し、D6 を supersede する。D6 は新たな可視性を実装の新規性と同一視して既存 public 要素の catalogue 整備責任を課しており、その責任を残したまま限定条件を加える refinement では訂正できないためである。

### D2: feature の有効化は観測レンズだけを変更する

cargo feature の有効化は抽出が観測する surface を変更する。これにより初めて抽出面へ現れた既存 public 要素も、feature を有効化したという事実だけでは新規または変更された実装成分にならない。

したがって、feature を初めて宣言したことだけを根拠として、可視化された既存 public 要素の catalogue 宣言を義務づけない。catalogue への義務は、その要素を実際に追加または変更することから生じる。

### D3: 設計対象外の既存要素には通常の baseline 処理を適用する

feature の有効化によって抽出面へ入り、かつ変更が追加も変更もしない既存要素には、他の既存要素と同じ通常の baseline 処理を適用する。catalogue entry の追加を要求するための特別な新規可視集合は設けない。

これは grandfathering や適合義務からの exemption ではない。対象の既存要素には、この変更が結ぶべき仕様上の契約がなく、もともと catalogue の適合範囲に入っていないためである。

## Rejected Alternatives

### A. D6 の責任を維持して強制機構だけを追加する

可視性を実装の新規性とみなす前提を固定し、誤った適合範囲をより強く強制することになる。強制可能性を高めても scope の category error は解消しないため採用しない。

### B. 新たに可視化された既存要素をすべて catalogue に宣言する

変更が設計していない要素には entry 固有の仕様契約がなく、catalogue の trace を全体的な完了条件で代用することになる。仕様から型契約への対応を表さない entry を増やし、catalogue の conformance semantics を弱めるため採用しない。

### C. 新たに可視化された既存要素を grandfathering リストへ登録する

適合対象である要素を例外扱いする構造だが、これらの既存要素は最初から変更の設計対象ではない。存在しない義務に exemption を設けることになり、scope の誤りを温存するため採用しない。

## Consequences

### Positive

- catalogue entry は、その変更が設計する実装成分と仕様上の契約に対応する。
- feature 宣言は抽出条件の指定に留まり、既存コード全体の catalogue 化を暗黙に要求しない。
- 通常の baseline 処理と catalogue の適合責任の境界が、実装の追加・変更という同じ基準で揃う。

### Negative

- feature の有効化によって初めて観測される既存コードの drift は、catalogue への宣言義務では検出されない。
- D6 の責任を前提としていた下流成果物は、SoT chain に従って再評価する必要がある。

### Neutral

- 既存コードの drift を検出する必要性そのものは否定しない。その責任は catalogue の適合契約とは別に設計される。

## Reassess When

- catalogue の役割を、変更が設計する実装成分の契約から repository 全体の public surface inventory へ拡張する判断がなされたとき。
- 未観測だった既存コードの drift を検出する独立機構を設計し、catalogue または baseline との責務境界を再定義する必要が生じたとき。

## Related

- `knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md` — 本 ADR は D6 の scope を訂正し、可視性を理由とする既存 public 要素の catalogue 整備責任を supersede する。D1–D5 と D7 は変更しない。
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — 変更しない既存要素を通常の baseline で扱い、catalogue の対象を変更された実装成分へ限定する基礎判断。
