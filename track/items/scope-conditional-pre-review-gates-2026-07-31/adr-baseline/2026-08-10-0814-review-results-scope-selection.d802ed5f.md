---
adr_id: 2026-08-10-0814-review-results-scope-selection
decisions:
  - id: D1
    review_finding_ref: "rollback-diagnoser:scope-conditional-pre-review-gates-2026-07-31:D4-scope-selection"
    status: proposed
---
# `sotp review results` の scope selector と列挙範囲を一致させる

## Context

`sotp review results` の scope universe は `scope_config.all_scope_names()` で定義される。一方、
単一 scope を指定する query では、universe 全体の列挙と選択 scope だけの表示を同時に要求できない。
selector の種類ごとに列挙範囲を定めなければ、単一 scope の指定が出力へ反映されず、selector の
競合や未知の scope 名も一意に扱えない。

## Decision

### D1: selector ごとに列挙範囲を定め、競合と invalid scope を拒否する

`sotp review results` の scope selection は `All | Named(validated ScopeName)` とする。

- `All` は明示的な `--all` または selector の省略で選ばれ、
  `scope_config.all_scope_names()` が返す scope universe を省略なく完全に列挙する。
- `Named(validated ScopeName)` は `--scope <name>` で選ばれ、検証済みの選択 scope だけを表示する。
- `--scope <name>` と `--all` の同時指定は、どちらかへ暗黙に優先順位を付けず拒否する。
- `--scope <name>` の値は query の前に `ScopeName` の形式を検証し、さらに検証済みの値が
  `scope_config.all_scope_names()` の返す scope universe に属することを検証する。両方の検証を
  通過した場合だけ `Named` を構築し、形式が invalid な名前と universe に存在しない未知名を
  fail-closed で拒否する。

この決定は `2026-04-28-1905-review-results-command.md` D4 を selector-aware な列挙規則へ
**refine** し、D4 の unconditional な完全列挙句だけを supersede する。`All` における scope universe の
完全性、implicit `Other` の包含、各 scope の state 導出、および選択された列挙範囲内で scope を
省略しない規則は維持する。

理由は、列挙範囲を selection type の variant に対応させ、`Named` の構築を形式検証と universe
membership 検証の両方に従属させることで、全体表示と単一 scope query の意味を混在させず、
競合入力と未知入力を query 実行前に fail-closed で排除できるためである。

## Rejected Alternatives

### A. selector にかかわらず常に全 scope を列挙する

`--scope <name>` が出力範囲を限定せず、単一 scope query の意味を満たさないため採用しない。

### B. `--scope <name>` と `--all` の同時指定に暗黙の優先順位を設ける

利用者の競合した意図を隠し、呼び出し側ごとに優先規則の推測を生むため採用しない。

### C. invalid scope 名を空の結果として扱う

未知の scope と、既知 scope に表示対象がない状態を区別できず、入力誤りを成功として扱うため採用しない。

## Consequences

### Positive

- selector の指定と表示される scope 集合が一致する。
- selector 省略時も `All` として full scope universe の完全列挙を維持できる。
- 競合入力と invalid scope を表示結果から推測せず、入力境界で検出できる。

### Negative

- 単一 scope の情報だけが必要な呼び出し側は `--scope <name>` を明示する必要がある。
- 複数の named scopes を一度に選択する surface は提供しない。

## Reassess When

- 複数 named scopes の選択や selector expression が必要になり、`All | Named` の二 variant では
  表現できなくなったとき。
- scope universe または `ScopeName` の検証境界が変更されたとき。

## Related

- `knowledge/adr/2026-04-28-1905-review-results-command.md` D4 — D1 が unconditional な完全列挙句を
  selector-aware な規則へ refine する対象。`All` の full universe 列挙と state 導出規則は維持する。
