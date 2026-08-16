# Observations — 2026-08-14-review-yield-measurement

## 既知の逸脱: `infrastructure::TelemetryEvent` の legacy フィールドが R9 に非適合

### 事実

`infrastructure-types.json` の `TelemetryEvent`（`action: modify`）は、概念を担うフィールドを生プリミティブのまま宣言している。

`schema_version: u32` / `track_id: String` / `duration_ms: u64` / `timestamp: String` / `gate_name: String` / `verdict: String` / `retry_count: u32` / `verdict_parse_failed: bool` / `hook_name: String`

`knowledge/conventions/type-designer-kind-selection.md` R9 は、検証可能な制約・有限値集合・ドメイン的意味を持つ概念を生プリミティブで宣言することを禁じ、serde 境界も例外ではないと明記している。`verdict` / `gate_name` / `hook_name` は有限集合、`timestamp` は書式制約、`track_id` は識別子、`duration_ms` は単位付き数量であり、いずれも「真に不透明な値」とは言えない。したがって type-designer の 12c（project-declared rule confirmation）はこの track で通っていない。

### 本 track が持ち込んだものではない

これらのフィールドは 2026-06-11 の宣言時点から存在する。R9 の serde 境界条項はその 3 週間前（2026-05-21）から有効だったが、当時の 12c もその後のレビューも指摘していない。本 track が `ReviewRound` variant に追加したのは型付き DTO ひとつであり、それ自体は R9 に適合している。既存フィールドを残しているのは、本 track のレビューが「変更しない variant の wire 形状を変えてはならない」と要求したためである。

`TelemetryEvent` の完全な post-change 形状を再宣言する義務が生じたのは、`TelemetryWriter::write` が当該 enum を受け取る以上、記録軸の追加が variant の変更を avoidably ではなく必然的に伴うからである。

### 判断

本 track は計測の追加を主題とし、legacy telemetry envelope の型付けは是正しない。是正は後続 track で扱う。この逸脱は PR の Accepted Deviations として user 承認の下に記録し、merge は止めない（user 裁定、2026-08-15）。

### 後続 track への引き継ぎ

- 対象は上記 9 フィールドと、それらを持つ全 variant。
- `TrackId`（`libs/domain/src/ids.rs`）と `Timestamp`（`libs/domain/src/timestamp.rs`）は既存であり、新規に型を起こす必要があるのは verdict / gate_name / hook_name 等の有限集合と、duration の単位付き数量。
- serde は newtype を透過的に直列化できるため、wire 形状を保ったまま型のみ締められる見込み。
- 同じ違反が 2 回のレビューを素通りしている。後続 track では 12c の対象を「本 track が変更した slot」ではなく「宣言に現れる全 slot」として明示的に走査すること。

## 観測: 「spec 要素を持たない内部協力者を公開面に出す」が 3 回繰り返された

本 track では、次の 3 つの型が公開面に置かれ、いずれも引くべき spec 要素を持てずに義務検証で行き詰まった。

1. `ReviewYieldRecordingReviewerError`（構築時の track 不一致エラー）— 4 通りの binding すべてが `CentralUnverified` / `Substitution` で否定され、spec に要素を足す案も review が「契約の拡張」として却下。最終的に不一致を構築不能にして型ごと削除。
2. `ReviewerStartCapture` / `ReviewerStartRecorder`（計時ヘルパー）— composition 層から infrastructure へ移した際に公開型として宣言し、AC-03 を引いたが「AC-03 は除外入口と既存挙動の保全を定めるだけで、開始時刻捕捉を要求していない」と却下。記録 adapter の内部詳細に畳み込んで解決。

いずれも解決は「引用を探す」ではなく「その型を公開面から無くす」だった。カタログの適用範囲は公開面であり、内部協力者を公開面に出すと、機能全体の要素を借りて引用するほかなくなる。この借用を義務検証（fulfillment judge）が一貫して拒否したため、設計の側が是正された。

検査の実効性という観点では、この 3 件はいずれも signal・lint・CI をすべて通り抜けており、捕まえたのは fulfillment judge と scope reviewer だけである。

## 観測: Phase 2 は「宣言の欠落」を検出できない

本 track は実装段階で type-design に複数回再入した。根本原因は Phase 2 が記録という振る舞いの担い手（port と adapter）を宣言しないまま収束扱いになったことにある。

chain ②（カタログ → spec）は宣言された entry が spec を引いているかを評価するため、「spec が要求する仕事に対応する entry が存在しない」という欠落は原理的に検出できない。欠落が可視になるのは chain ③、すなわち実装との突き合わせ時である。types scope のレビューも 2 回 zero_findings を返しており、人間側でも捕捉されなかった。

この track の主題（どの検査が何を検出しているか）に対する実例として記録する。
