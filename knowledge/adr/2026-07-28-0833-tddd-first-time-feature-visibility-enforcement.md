---
adr_id: "2026-07-28-0833-tddd-first-time-feature-visibility-enforcement"
decisions:
  - id: D1
    review_finding_ref: "github_pr_review:#227:D6-enforcement-mechanism-gap"
    candidate_selection: "from:[merged_declaration_history,current_tree_only,manual_assertion] chose:merged_declaration_history"
    status: proposed
  - id: D2
    review_finding_ref: "github_pr_review:#227:D6-enforcement-mechanism-gap"
    candidate_selection: "from:[same-revision_counterfactual,prior-track_baseline,source_inference] chose:same-revision_counterfactual"
    status: proposed
  - id: D3
    review_finding_ref: "github_pr_review:#227:D6-enforcement-mechanism-gap"
    candidate_selection: "from:[exclude_from_ordinary_baseline_seeding,separate_fail-closed_diff] chose:separate_fail-closed_diff"
    status: proposed
---
# 初回 feature 宣言で新たに可視化される public 要素を fail-closed で catalogue に拘束する

## Context

`2026-07-27-0039-tddd-track-scoped-feature-declaration.md` D6 は、ある feature を最初に宣言する track に、その feature によって新たに可視化される既存 public 要素を catalogue へ整備する責任を課した。

通常の TDDD baseline は、既存 public 要素を各 track の catalogue へ繰り返し宣言させないために、baseline に存在する要素を暗黙の `Reference` として扱う。この沈黙は既存要素のノイズを避けるために必要である。一方、feature を有効にして取得した baseline には、その feature によって初めて抽出面へ現れた要素も含まれる。そのため、対象要素が catalogue に無くても通常の baseline 処理だけでは未宣言として検出されず、D6 の責任を gate で強制できない。

D6 を強制可能にするには、宣言が初回かを判定する正本、feature を宣言する前の抽出面、および両抽出面の差分に対する評価規則が必要である。これは通常の baseline が担う既存要素の除外を廃止する判断ではなく、初回宣言による可視性の増分だけを別に拘束する判断である。

## Decision

### D1: 初回宣言の正本を merge target に取り込まれた宣言成果物の累積履歴とする

feature の初回宣言判定には、merge target に取り込まれた track 単位の feature 宣言成果物の累積履歴を用いる。作業中の branch、現在の `Cargo.toml`、人手による申告は、先行宣言の有無を決める正本にしない。

feature の同一性は対象 crate と cargo feature 名の組で判定する。layer 名だけでは、layer と crate の対応変更によって別の feature を同一視し得るため、同一性の根拠にしない。

評価対象の宣言に含まれ、かつ先行して merge target に取り込まれた宣言成果物のいずれにも含まれない feature を初回宣言とする。履歴の欠落、重複、crate 同一性の不明確さなどによって先行宣言の有無を一意に決められない場合は、初回でないと推定せず fail-closed とする。

### D2: 同一 source revision から feature 宣言前後の抽出面を対で取得し保持する

初回宣言された feature の集合を `N`、評価対象の宣言が有効にする feature の全集合を `F` とする。新たに可視化された public 要素を判定するため、同一の source revision から次の二つの抽出面を取得する。

- 宣言後の抽出面: `F` を有効にした面
- 宣言前の抽出面: `F` から `N` を除き、初回宣言された feature が有効でない面

両面は source revision、toolchain、および feature 集合以外の抽出条件を同一にする。初回宣言 feature が別 feature から推移的に有効になる場合を含め、宣言前の面で当該 feature の無効化を保証できなければ fail-closed とする。

宣言前の面は、宣言後の baseline と比較できる対の証拠として、当該 baseline を用いる評価が完了するまで保持する。過去の別 revision の baseline は source 変更と feature 可視性の差を分離できないため、この面の代用にしない。

### D3: 新規可視集合を独立した fail-closed 差分として catalogue と照合する

宣言後の抽出面を `B_enabled`、宣言前の抽出面を `B_pre` とし、`B_enabled \ B_pre` を初回宣言による新規可視集合 `V` とする。

`V` の各 public 要素は catalogue に明示的な契約を持たなければならない。`B_enabled` に含まれることを理由とした暗黙の `Reference` は、この要件を満たしたものとみなさない。`V` に catalogue 未宣言の要素がある場合、または対となる抽出面を比較できず `V` を確定できない場合は fail-closed とする。

通常の baseline から暗黙の `Reference` を作る規則は変更しない。`V` の照合を独立させることで、先行する既存 public 要素に対する必要な沈黙を維持しながら、D6 が対象とする可視性の増分だけを強制する。

本決定は `2026-07-27-0039-tddd-track-scoped-feature-declaration.md` D6 に、欠けていた強制機構を追加する refinement であり、D6 を supersede しない。

## Rejected Alternatives

### A. 現在の source tree または人手の申告から初回宣言を判定する

現在の source tree は feature の存在を示せても、どの track が既に宣言したかを示さない。人手の申告は branch 間の並行作業や過去の宣言を安定して照合できず、誤った「初回ではない」という判定が gate を迂回するため採用しない。

### B. 過去の別 revision の baseline を宣言前の面として再利用する

二つの面の差に source 変更と feature 可視性の変更が混在する。差分のどの要素が初回宣言によって現れたかを一意に決められないため採用しない。

### C. 新規可視集合を通常の baseline から暗黙の `Reference` へ seed しない

初回宣言に起因する要素を未宣言として検出できるが、通常の baseline seeding が担う既存要素のノイズ抑制と、D6 固有の責任を同じ集合演算へ混在させる。新規可視集合の算出失敗が通常の baseline 全体の意味を変え、既存要素まで未宣言として扱う危険があるため採用しない。

### D. source 上の `cfg` 属性から新規可視集合を推定する

Cargo feature の依存、推移的な有効化、および条件式を通過した実際の public 抽出面を source 上の属性だけでは確定できない。catalogue と照合すべき観測結果ではなく近似になるため採用しない。

## Consequences

### Positive

- D6 の catalogue 整備責任が、任意の遵守ではなく fail-closed な検査対象になる。
- 通常の baseline による既存 public 要素のノイズ抑制を維持できる。
- 初回宣言判定と新規可視集合の双方が、レビュー可能な repository 内の証拠に基づく。

### Negative

- 初回 feature 宣言を含む評価では、同一 revision から二つの抽出面を取得するコストが増える。
- 宣言履歴と対になる抽出面を、判定に必要な期間保持する責務が増える。
- feature の推移的な有効化によって宣言前の面を構成できない場合、評価は停止する。

### Neutral

- 初回宣言を含まない評価の通常の baseline seeding は変わらない。
- catalogue に要求する public 要素の契約内容は既存の TDDD 規則に従い、本 ADR では新しい catalogue schema を定めない。

## Reassess When

- Cargo が非加算的な feature 意味論を導入し、`F` と `F \ N` の対比較が可視性の増分を表さなくなったとき。
- merge target の宣言成果物履歴を、同じ来歴を保つ専用の機械可読な累積記録へ置き換える必要が生じたとき。
- 同一 revision の二重抽出コストが、初回宣言の検査を継続できない水準になったとき。

## Related

- `knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md` — D6 の catalogue 整備責任を、初回判定・宣言前後の抽出面・独立差分によって強制可能にする refinement。supersede はしない。
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — 通常の baseline が既存 public 要素を未宣言ノイズから除外する判断。
