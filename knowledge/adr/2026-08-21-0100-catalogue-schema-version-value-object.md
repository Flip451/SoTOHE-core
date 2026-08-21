---
adr_id: "2026-08-21-0100-catalogue-schema-version-value-object"
decisions:
  - id: D1
    review_finding_ref: "chain2:domain:CatalogueSchemaVersion:semantic-verification-failure"
    status: proposed
---
# catalogue の schema version を値オブジェクトとして扱う

## Context

catalogue 文書の `schema_version` は JSON では数値として表され、受理できる version の意味論も数値を基準としている。一方で、catalogue の schema version は任意の数値一般ではなく、文書形式の版を表すドメイン概念である。

型カタログの semantic verification では、この概念を path 解決の決定へ接地しようとしたが、両者の目的は異なるため接地できなかった。`schema_version` を生の数値のまま扱い続けることは、版を表すという意味を型から失わせる。

## Decision

### D1: schema version を数値表現を保つ値オブジェクトにする

catalogue 文書の `schema_version` は、schema version を表す専用の値オブジェクトで保持し、任意の数値一般とは区別する。この値オブジェクトは既存の schema version 数値を保持・公開できるものとし、codec と検証はその値を用いて既存の受理 version 意味論を維持する。

外部 JSON における `schema_version` の表現は従来どおり数値とする。値オブジェクトの導入は wire format の変更、既存に受理される version の追加・削除、または数値としての読み出しの廃止を意味しない。

この決定は `schema_version` に非ゼロ不変条件を導入しない。また、値がゼロであることを理由とするエラーの要否・形・挙動も決定しない。受理範囲に新たな制約を加える根拠はこの決定には含めない。

## Rejected Alternatives

- **`schema_version` を生の数値のまま保持する**: 文書形式の版というドメイン上の意味が型に現れず、任意の数値との取り違えを防げない。
- **JSON で文字列または object に変更する**: schema version のドメイン化に wire format の変更は不要であり、既存の数値表現との互換性を失う。
- **非ゼロ制約を同時に導入する**: 現在の受理 version 意味論を変更する根拠がなく、値オブジェクト化からは導けない別の制約である。

## Consequences

- 良: catalogue 文書内で schema version が担う意味を型として明示でき、raw numeric value との混同を避けられる。
- 良: 既存の JSON 数値表現と受理 version 意味論を維持したまま、domain・codec・利用側の境界で schema version を明示的に扱える。
- 中立: 値オブジェクトへの変換と数値の公開を担う境界を実装で定める必要があるが、それは既存の受理 version 意味論を変更しない。
- 中立: ゼロ値の扱いは本 ADR で決めないため、将来その必要が生じた場合は独立した根拠と決定を要する。

## Reassess When

- catalogue schema の version 表現が数値以外へ変更される場合。
- 受理する schema version の意味論を変更する必要が生じた場合。

## Related

- `knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1` — 型・トレイトの完全修飾パスによる識別を決める既存決定。本 D1 は schema version のドメイン表現を対象とする独立した決定であり、これを supersede も refine もしない。
