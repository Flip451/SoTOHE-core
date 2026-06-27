---
adr_id: 2026-06-27-0440-tddd-rustdoc-document-hidden-items
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01LZzkoFBfPHtNXvhWmjBox3:2026-06-27"
    candidate_selection: "from:[A,B,C,D,E] chose:rustdoc-document-hidden-items"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_01LZzkoFBfPHtNXvhWmjBox3:2026-06-27"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_01LZzkoFBfPHtNXvhWmjBox3:2026-06-27"
    status: proposed
---
# TDDD chain ③ の `cargo rustdoc` 呼び出しに `--document-hidden-items` を追加する

## Context

TDDD chain ③ (`bin/sotp signal calc-impl-catalog`) は各 layer crate の rustdoc JSON を `cargo rustdoc --output-format json` で取得し、baseline / actual の impl catalogue を生成する。`--output-format json` 自体が rustdoc unstable feature であり、本 chain は既に nightly toolchain と `-Z unstable-options` を要求する。

rustdoc の既定挙動は `#[doc(hidden)]` 属性のついた要素を paths から除外する。そのため `pub` かつ `#[doc(hidden)]` な要素は chain ③ の rustdoc paths から消え、baseline で declare 済みの Id がパスとしては解決できず、`DanglingId` Yellow/Red を発火させて track-active-gate を block する事象が観測された。

rustdoc には `-Z unstable-options --document-hidden-items` フラグがあり、`#[doc(hidden)]` 要素を paths に含めて出力できる。chain ③ は既に nightly + `-Z unstable-options` に依存しているため、本フラグ追加は追加 toolchain コストを発生させない。問題の root cause (rustdoc paths から消える) を rustdoc invocation 側で直接解消する余地がある。

## Decision

### D1: `calc-impl-catalog` の rustdoc invocation に `-Z unstable-options --document-hidden-items` を付与する

TDDD chain ③ の baseline 取得 / actual 取得双方が呼ぶ `cargo rustdoc --output-format json` の引数に `-Z unstable-options --document-hidden-items` を追加する。これにより `#[doc(hidden)]` のついた pub 要素も rustdoc paths に含まれ、catalogue 突合で `DanglingId` を発火しなくなる。

### D2: 適用範囲は TDDD chain ③ の rustdoc 呼び出しのみに限定する

本フラグの適用は `bin/sotp signal calc-impl-catalog` が直接実行する `cargo rustdoc` 呼び出し 1 箇所のみとする。プロジェクト全体の rustdoc 設定や `cargo doc` 経由の user-facing doc 生成には適用しない。理由: 本変更は TDDD 突合のための内部 catalogue 生成手段であり、公開 doc の表示方針 (hidden の本来用途) を巻き添えにしない。

### D3: source レベルでの `#[doc(hidden)]` 禁止 gate は導入しない

既存提案 (`forbid-doc-hidden` / `prohibit-doc-hidden-attribute`) の syn AST scanner 案は採用しない。`#[doc(hidden)]` は rustdoc 上の表示制御マーカとして valid な Rust API であり、本来の用途 (semver hazard hiding / internal API marker / unstable API の非公開) を妨げる正当な理由は無い。chain ③ の DanglingId 問題は rustdoc invocation 側の解決 (D1) で十分。

## Rejected Alternatives

### A. syn AST レベルで `#[doc(hidden)]` を ban する gate を新設

source を syn でパースし `#[doc(hidden)]` 相当の attribute を検出して fail させる gate を新設する案。却下理由:

- `cfg_attr` / `r#hidden` (raw identifier) / inner attribute (`#![doc(hidden)]`) / inline mod / `#[path]` / impl block propagation / 関連 const / fn 内 local item などの edge case 連鎖を網羅するため、大規模な scanner と module resolution ロジックが必要になる
- `#[doc(hidden)]` 自体は Rust の正規 API であり、user-facing rustdoc から hide する正当な用途を殺してしまう
- D1 で chain ③ 側の root cause を直接解消できるため、source 側 ban は不要

### B. `#[doc(hidden)]` を coding convention でだけ抑止する

機械チェック無しで convention guide のみで運用する案。却下理由:

- 機械チェックが無いので後発で `#[doc(hidden)]` が混入したときに DanglingId が再発する
- D1 で DanglingId 自体を構造的に防げるので、convention での抑止は不要

### C. TDDD chain ③ で `#[doc(hidden)]` 要素を catalogue 比較から除外

catalogue 突合時に `#[doc(hidden)]` flag のある Id を skip する案。却下理由:

- 既定 rustdoc では `#[doc(hidden)]` 要素は paths に現れないので、比較側で属性を見ることができない (前提が崩壊する)
- baseline ↔ actual の差分が `#[doc(hidden)]` 追加で生じた場合、その差分自体を見失う

### D. rustdoc 経由を諦め syn AST から catalogue を構築

chain ③ の rustdoc JSON 依存を廃止し、syn AST から path 解決 / generic 解決 / trait method dispatch を自前実装する案。却下理由:

- rustdoc が提供する解決済みの path / generic resolution / trait method dispatch を再実装する必要があり、保守コストが膨大
- rustc が解決済みの情報を再パースで作り直す責務を抱え込むのは inversion of responsibility

### E. 文字列 grep で `#[doc(hidden)]` を検出して fail

ripgrep ベースで source を grep する gate 案。却下理由:

- `cfg_attr` / マクロ展開 / 文字列リテラル誤検知 / raw identifier 等で false positive / false negative が多発する
- prior track が syn AST に逃げた背景と同じ問題に直撃する

## Consequences

### Positive

- DanglingId 起因の track-active-gate ブロックが構造的に解消される
- `#[doc(hidden)]` を本来の API hide 用途 (semver hazard hiding / internal marker / unstable API 非公開) で使えるようになる
- 修正は chain ③ の rustdoc 呼び出し 1 箇所で完結し、syn / module resolution の edge case 連鎖を踏まない
- source レベルの ban gate を別途新設する選択肢を取らずに済むため、scanner 系の保守負担が発生しない

### Negative

- TDDD catalogue の declaration 対象が拡張される (`#[doc(hidden)]` 要素も declare 必要)。既存 catalogue に `#[doc(hidden)]` 要素がある layer crate では再 baseline + 再 catalogue 整備が必要になる可能性がある
- `--document-hidden-items` の挙動が将来 rustdoc 側で変わった場合、再評価が必要

### Neutral

- rustdoc unstable flag (`-Z unstable-options`) 依存が増えるが、chain ③ は既に同 flag を使用しているため増分依存はゼロ
- nightly toolchain 依存は変化しない (`--output-format json` 自体が nightly 要)

## Reassess When

- rustdoc の `--document-hidden-items` フラグ仕様が変更されたとき
- `--output-format json` が stable 化されたとき (同時に `--document-hidden-items` の stabilization 状況を確認)
- `#[doc(hidden)]` の rustc/rustdoc semantic が変わったとき
- TDDD chain ③ が rustdoc JSON 依存をやめる別決定がなされたとき
- chain ③ rustdoc が `Cargo.toml` の `[package.metadata.docs.rs]` 設定など別経路で hidden 含む経路を提供したとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/` — 工学規約一式
