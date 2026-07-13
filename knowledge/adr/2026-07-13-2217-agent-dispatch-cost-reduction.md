---
adr_id: 2026-07-13-2217-agent-dispatch-cost-reduction
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 fast/final 二段構成を維持し、capability と fast/final 毎に effort を指定可能にして fast を low にする提案 + 同日追補: final は各 provider の最大段階を規定し effort 未指定は fail-closed、pr-reviewer を例外とし ref-verifier-chain1/chain2 を対象に含める"
    candidate_selection: "from:[per-capability-tier-effort,merge-fast-final-tiers,global-effort-lowering,fail-closed-on-missing-effort,fail-open-on-missing-effort] chose:[per-capability-tier-effort,fail-closed-on-missing-effort]"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 claude -p -r / codex exec resume による差分 review の提案 + 同日追補: 差分は reviewer が自己確認し、再入時の全件再判定の権限・義務は capability SSoT に常設記載"
    candidate_selection: "from:[same-scope-same-tier-resume,fresh-every-round,cross-tier-resume] chose:same-scope-same-tier-resume"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 D1-D3 を同一 ADR に含める裁定 + 同日追補: skip 判定状態は per-layer type-signals artifact への実装側入力 hash 記録で保持する裁定"
    candidate_selection: "from:[diff-based-prelude-skip,recompute-every-round,artifact-embedded-input-hash,separate-transient-hash-cache] chose:[diff-based-prelude-skip,artifact-embedded-input-hash]"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01DNXZbHA36W7ziMHyccmyvt:2026-07-13 review 関連以外の capability も resume できるようにすべきという提案 + 同日追補: track 外 dispatch は workspace transient 領域に capability × 対象 artifact キーで cache する二層化裁定"
    candidate_selection: "from:[orchestrator-opt-in-capability-resume,fresh-every-dispatch,always-auto-resume,two-tier-track-and-workspace-cache,no-resume-outside-track,workspace-only-cache] chose:[orchestrator-opt-in-capability-resume,two-tier-track-and-workspace-cache]"
    status: proposed
---
# 外部 agent 呼び出しのコスト削減

## Context

1 track の全工程（ADR 起草 → 設計 Phase → 実装 batch review → PR review 通過）の所要時間を実測したところ、reviewer round 約 46 回・writer/実装 dispatch 約 15 回・full CI 約 7 回で合計約 5.5 時間となり、支配項は「round 数 × 1 round の単価」だった。1 round は 2〜16 分（中央値約 5 分）で、単価の内訳には次の固定費が含まれる。

1. reviewer の推論深度: fast round（前置フィルタ用の軽量 model）も final round と同じ高 effort で走っていた。effort の出所が provider CLI の project 全体設定であり、capability や round 種別の単位で制御する手段が存在しない。
2. 文脈の再取得: 同一 scope の再 round（fixer 修正後の再判定）でも、reviewer は毎回新規 session で scope diff・severity policy・関連文書を読み直す。docs 1 語の指摘の解消でも「writer 再入 + fast 再走 + final 再走」の 3 round を新規文脈構築込みで払う。
3. gate 前段の再計算: reviewer 起動 wrapper は毎回 rustdoc 抽出を伴う signal 再計算を実行するが、設計フェーズ中は Rust コードが不変であり、この再計算は結果が変わらない。
4. writer 系 capability の再入も同じ構造を持つ: 同一 track で type-designer は 9 回 dispatch され、2 回目以降の大半は小修正の再入（entry 追加・anchor 修正・docs 短縮）だったが、毎回新規 session のため capability 運用文書・spec・全層 catalogue・baseline の文脈再構築から始まった。timeout で中断した implementer の継続も、中間状態を説明する briefing を人手で再構成する必要があった。

fast/final の二段構成そのものは維持する（前置フィルタと full-model 最終判定の役割分離は保つ）。本 ADR は二段構成を保ったまま、外部 agent 呼び出し（reviewer round と capability dispatch）の単価を下げる 4 決定を定める。

## Decision

### D1: effort は capability と round 種別（fast/final）の単位で明示する

capability → provider routing の設定に effort 指定を追加する。reviewer のように fast/final で model を使い分ける capability は round 種別ごとの effort（fast 用と final 用）を持ち、その他の capability は単一の effort を持つ。dispatch 経路（reviewer 起動・fixer 起動・汎用 capability 起動）は解決した effort を provider CLI に伝える — codex は reasoning effort の設定 override（注入により実行 header の effort 表示が変わることを実機確認済み）、claude は effort 指定 flag（低〜最大の段階値。headless 実行との併用が受理されることを実機確認済み。効果の直接観測手段はないため、実装時は受理の regression 確認までを検証範囲とする）。

effort 指定の無い dispatch は fail-closed とする — 対象 capability（reviewer は round 種別ごと）の effort が profile から解決できない場合、実行を拒否する。暗黙の provider 既定へ落ちる fail-open は採らない（推論深度という挙動決定要因を設定 file から一意に読めるようにする）。したがって committed default profile は対象の全 capability に effort を明示し、reviewer は fast を低 effort（low）、final を各 provider の最大段階（codex は xhigh、claude は max）とする。

適用範囲は provider CLI subprocess を起動して実行する全 capability とし、semantic reference 検証の `ref-verifier-chain1` / `ref-verifier-chain2` も対象に含める。例外は `pr-reviewer` — 実行が repository 外の hosted service 側で行われ、dispatch 側から model / effort 等の実行パラメータを注入できないため、effort 明示の対象外とし fail-closed 検査も適用しない。

### D2: 同一 scope × 同一 round 種別の再 round は reviewer session を再開して差分 review する

fixer の修正後に同じ scope・同じ round 種別で再判定する場合、前回 reviewer session を provider の session 再開機構（codex exec resume / claude -p の resume）で継続する。再開 prompt は再判定の依頼のみを伝え、変更内容の列挙はしない — 差分は reviewer 自身が scope の file list と diff から確認する（runner 側で差分を要約して渡すことは、判定材料の選別という判断の混入経路になるため行わない）。再入 round でも scope 全件を再判定する権限・義務を持つことは、reviewer 系 capability の運用契約（capability SSoT 文書）に常設の規定として記載し、prompt ごとの注入に依存させない。初回 round と fast→final の escalation は従来どおり新規 session とする（判定の独立性を保ち、また model が変わる escalation は再開不能なため）。

session id は reviewer 実行基盤が track × scope × round 種別のキーで保存する。保存先は track 単位の機械 local transient として track 成果物 directory 配下に gitignore 管理で置き、committed な SoT file には入れない（新設の top-level transient path は template export の境界 manifest 分類に抵触するため作らない）。cache entry は provider と model に加え、**安定した execution contract だけ**から計算した fingerprint を束縛する。入力は、現在の reviewer capability SSoT、scope-specific severity policy、scope identity と file-selection 規則を定める review-scope 設定、及び生成済み review briefing のうちこれらの固定 template / 設定から機械的に切り出せる execution-contract 部分であり、いずれも path と内容を hash 化する。briefing の Design Intent・再判定依頼等の可変 task payload、解決済み scope file list の内容、及びその diff は fingerprint に含めない。これらは fixer 修正のたびに変わる**現在の判定材料**であり、再開の可否ではなく再開後に reviewer 自身が読み直して全件再判定する対象である。読み出し時に現在の profile 解決結果又は安定 contract fingerprint と不一致なら失効として新規 session に落とす。安定 contract のいずれかを読出し又は hash 化できない場合も再開を許さず新規 session にする。再開の失敗・session 期限切れも同様に新規 session へ fallback する（resume という最適化は諦めるが、round の実行自体は止めない）。

**再開時は model・sandbox・effort の全実行 flag を、引き継ぎ挙動に依存せず毎回明示的に再指定する。** 実機検証で provider 間の挙動差を確認している — codex の再開は元 session の設定を引き継がず、無指定では project 既定（別 model・書き込み可 sandbox・高 effort）に落ちる。claude の再開は model を引き継ぐことを確認したが、effort 等の引き継ぎ有無は出力から観測できない。引き継ぎに依存した素の再開は、provider や version によって sandbox 逸脱を含む欠陥となり得るため、明示再指定を両 provider 共通の必須規則とする。

### D3: reviewer 起動 wrapper の signal 再計算は対象コードの差分がないとき skip する

reviewer 起動前の gate 前段では、**rustdoc 抽出だけを**前回計算時点から実装側入力と rustdoc-extraction contract が不変な layer で skip する。signal 評価そのものを省略できるのは、その評価に使う全入力 — 実装側 input、catalogue declaration、baseline、及び evaluator contract — が不変である場合に限る。catalogue 又は baseline の変更時は、検証済みの live rustdoc snapshot を用いて implementation↔catalogue signal を再評価する。evaluator contract の変更又は判定不能時も signal 評価を必須にするが、**rustdoc-extraction contract が一致して検証済み snapshot の条件を満たす場合に限り**、それだけで rustdoc 抽出までは要求しない。rustdoc-extraction contract の変更又は判定不能時は snapshot を無効として rustdoc 抽出から再計算する。spec の変更時は catalogue↔spec 側の評価を再実行するが、これも rustdoc 抽出の再実行理由にはしない。いずれの reuse 判定も内容 hash の機械的比較で行い、signal 評価 skip の判定不能時は signal 評価を、snapshot 再利用の判定不能時は rustdoc 抽出からの再計算を必須にする（skip という省略に対して fail-closed）。

skip 判定の状態は別の cache file を新設せず、計算産物である per-layer type-signals artifact 自体に実装側入力 hash、baseline hash、live rustdoc snapshot hash、evaluator-contract hash、及び rustdoc-extraction-contract hash として記録する。既存 artifact は catalogue 側入力の hash（declaration_hash）を既に保持しており、これらを対称に加えることで、鮮度判定は「artifact に記録された入力 hash と現在の入力 hash の一致」という artifact 自己記述の検証になる（鮮度情報とそれが記述する計算結果が同一 file に閉じ、別 file 間の対応管理が発生しない）。evaluator contract は、signal 出力の意味に影響する evaluator 実装の解決済み code / build identity、rule・設定 file の内容、schema version、及び有効 feature / 設定値の完全な正規化済み closure とする。rustdoc-extraction contract は、Cargo target と rustdoc root の解決、rustdoc invocation の引数・出力形式、及び snapshot の読出し・検証に影響する実装 code / build identity と設定値の完全な正規化済み closure とする。ある実装又は設定が両方へ影響する場合は両方の contract に含める。実装は両 contract を機械的に解決して hash 化する。現在の evaluator-contract hash が artifact 記録値と不一致、又は旧 artifact を含めいずれかの値を読出し・hash 化できない場合は signal 評価を必須にする。現在の rustdoc-extraction-contract hash が不一致、又は読出し・hash 化できない場合は snapshot を無効として rustdoc 抽出から再計算する。snapshot 本体の正本は、当該 rustdoc invocation が解決した Cargo target directory の `doc/<resolved-rustdoc-root>.json`（通常は workspace の `target/doc/`）とする。再利用経路は同一の target / rustdoc-root 解決規則でこの既存 JSON の path を求め、通常の symlink guard と JSON parser を通して**直接読出す**。この経路は Cargo / rustdoc を起動しないため、新しい transient cache は作らない。catalogue / baseline 変更時にこれを再利用できるのは、現在の**実装側 input hash**と rustdoc-extraction-contract hash が artifact 記録値に一致し、当該 JSON が存在して読出し・parse に成功し、その内容 hash が記録済み snapshot hash と一致する場合だけである。baseline hash の不一致又は evaluator-contract hash の不一致は signal 評価を必須にするが、snapshot 再利用を拒否する条件にはしない。実装側 input hash、rustdoc-extraction-contract hash、又は snapshot 検証のいずれかが欠ける又は不一致なら、snapshot を信用せず rustdoc 抽出から再計算する。

実装側 input hash は手選別の file list ではなく、対象 rustdoc invocation の**完全な解決済み build-input closure**を正規化して hash 化する。これは対象 crate の source / manifest だけでなく、workspace 内又は path dependency の source・manifest、build script と proc-macro の source、依存 package の解決済み source identity / checksum、lockfile、workspace と crate の Cargo manifest、Cargo config、toolchain 識別子、target triple、有効 feature 集合、及び rustdoc / rustc 又は build script に到達する設定値（`RUSTFLAGS`、`RUSTDOCFLAGS`、Cargo config と許可された環境値を含む）を含む。実装が closure を取得・正規化できない場合、又は build script / 環境 / 設定の入力で影響有無を機械的に確定できないものを検出した場合は hash を確定させず、snapshot / signal の reuse を許さず rustdoc 抽出から再計算に倒す。判定と skip の粒度は layer（crate）単位とし、変更のあった layer のみ再計算する。

### D4: capability dispatch も `capability exec` の resume オプションで session を再開できるようにする

`sotp capability exec` に session 再開の option を追加する。orchestrator は、同一 track × 同一 capability の**継続作業**（同じ成果物への追補・修正の再入、中断からの続行）と判断した dispatch でこの option を使い、初回 dispatch と関心事の切り替わる dispatch は従来どおり新規 session とする（opt-in — 再開するかの判断は呼び出し側が持つ）。

session id は capability 実行基盤が **track × capability × 対象成果物 identity** のキーで保存する。対象成果物 identity は対象 artifact の repo-relative path（複数を対象とする dispatch は path の正規化済み順序付き集合）とし、読み出し時にも同じ identity の一致を必須とする。対象 path が未確定の dispatch は track cache の対象外とする（新規 session で実行し、session id も記録しない）。保存先は D2 と共通の規則に従う: **track 単位の機械 local transient として track 成果物 directory 配下に gitignore 管理で置き、committed な SoT file（review 記録・track identity 等）には入れない**。track の削除・archive と lifecycle を共にし、並行 track 間または同一 track 内の別成果物間で混線しない。cache entry は session id に加えて **provider と model、及び安定した execution contract の path と内容から計算した fingerprint**を束縛する。安定 contract は現在の capability SSoT、dispatcher が常時注入する discipline、及び capability profile / SSoT が静的 contract input として宣言した policy / contract file から成り、dispatch briefing の本文そのものは含めない。briefing の task 指示・追補 / 修正依頼、対象成果物の現在内容、及び diff は継続作業ごとに変わり得る可変 payload であるため、同じ対象成果物への opt-in 再入を失効させる理由にはしない。再開時は現在渡された briefing を読み、対象成果物と必要な上流入力の変更を capability 自身が確認してから作業する。読み出し時に現在の profile 解決結果又は fingerprint と不一致なら失効（新規 session）として扱い、安定 contract のいずれかを読出し又は hash 化できない場合も resume しない — provider 間に session 互換はなく、model 跨ぎ又は古い作業契約での再開は品質劣化を伴うためである。

track 外の dispatch でも resume を使えるようにする（pre-track ADR 起草の adr-editor のように、branch-bound の track 解決が失敗する base branch 上の文脈）。この場合の session id は、workspace の機械 local transient 領域（briefing file 置き場と同じ gitignore 済みの既存 runtime path 配下 — 新設の top-level path は作らない）に **capability × 対象 artifact（repo-relative path）** のキーで保存する。capability 単独のキーは別対象への切り替え時に無関係な session を掴むため、対象 artifact のキー成分で機械的に失効させる。対象 path が未確定の dispatch は cache の対象外とする（新規 session で実行し、session id も記録しない）。path が確定した dispatch から session id を記録し、以後の再入で resume を使えるようにする。track cache と workspace cache のどちらを使うかは dispatch 時の track 解決結果で機械的に分岐し（解決成功 → track cache、解決失敗 → workspace cache）、新たな判定機構は設けない。provider / model と execution-contract fingerprint の束縛、読出し / hash 化不能時を含む失効、期限切れ時の新規 session への fallback は track cache と同一規則とする。

再開時も dispatcher は通常の dispatch と同一の解決（profile からの model / effort、provider-native 定義からの sandbox）を行い、**解決した全実行 flag を明示的に再注入する**（D2 と同一の規則。provider の引き継ぎ挙動に依存しない）。再開の失敗・session 期限切れは新規 session に fallback し、dispatch 自体は止めない。

writer 特有の注意として、再入の間に他 writer が上流 artifact を変更している場合がある（例: spec への要素追加後の型カタログ再入）。再開 prompt に runner が変更点を要約して渡すことはせず、**再開時は上流 artifact の変更有無を自ら確認し、変更があれば再読してから作業する義務**を capability の運用契約（共有 discipline ないし各 capability SSoT）に常設の規定として記載する。

適用範囲は subprocess dispatch 経路（provider CLI の起動を伴うもの）とし、in-host 委譲（host 自身の subagent 機構に委ねる分岐）は対象外とする。

## Rejected Alternatives

### A. fast/final の二段構成又は effort tier を統合する

round 数を半減して二段構成を統合すると、前置フィルタ（安価な model での早期発見）と full-model 最終判定という役割分離が失われる。二段構成を保ったまま `merge-fast-final-tiers` で fast / final の effort tier を共通化しても、fast だけを低 effort にして単価を下げる D1 の目的を達成できない。二段構成と tier 別 effort をともに維持するため却下。

### B. provider CLI の project 全体設定で effort を一律に下げる

fast round は安くなるが、final round・writer 系 capability・implementer の品質まで一律に下がる。効かせたい箇所（fast の前置フィルタ）だけに効かせられないため却下。

### C. fast→final の escalation でも session を再開する

escalation は model が変わるため技術的に再開できず、また final の独立した全件判定という役割を fast の心証が汚染する。同一 scope × 同一 round 種別の再 round に限定するため却下。

### D. session 再開時に元 session の実行設定の引き継ぎを期待する

実機検証で provider 間の挙動差を確認した。codex は再開時に model・sandbox・effort を引き継がず project 既定に落ちる（軽量 model・読み取り専用で記録した session が別 model・書き込み可 sandbox で再開され失敗した）。claude は model を引き継ぐが、effort 等の引き継ぎ有無は観測できない。挙動が provider 依存かつ一部観測不能である以上、引き継ぎ前提の素の再開は sandbox 逸脱を含む欠陥となり得るため、全 flag の明示再指定を必須とする。

### E. D2 の reviewer を毎 round 新規 session のままにし、provider 側の prompt cache に頼る

`fresh-every-round` は server 側の cache により入力 token 費用を下げられるが、reviewer がローカルで行う file 読み・diff 取得・探索の実時間は削減されない。round 単価の実測支配項は後者であり、同一 scope × 同一 round 種別の再判定では D2 の session 再開で削減できるため却下。

### F. gate 前段の signal 再計算を毎 round 維持する

現状維持案。設計フェーズでは対象 Rust コードが不変で再計算の結果が変わらないにもかかわらず、rustdoc 抽出を伴う再計算に毎 round 数分を払い続ける。差分判定の導入コスト（hash 比較と保守的 fallback）は一過性であり、round ごとの累積浪費の方が大きいため却下。

### G. effort 未指定の dispatch を provider 既定に fail-open する

既存 profile を無改変で導入できる利点はあるが、推論深度が provider / version 依存の暗黙既定で決まり、設定 file を読んでも実挙動が分からない状態が続く。今回の障害調査でも「fast round がいつの間にか最高 effort で走っていた」原因が global 設定への暗黙依存だった。挙動決定要因は profile に明示させる fail-closed を採るため却下。

### H. capability dispatch を毎回新規 session のまま維持する

現状維持案。再入 1 回ごとに capability 運用文書・上流 artifact・baseline の文脈再構築（数分〜十数分）を払い続け、中断からの続行も中間状態説明の briefing 再構成を要する。実測で writer 再入は 1 track に 10 回超発生しており、累積浪費が大きいため却下。

### I. capability dispatch を常時自動で resume する

opt-in でなく既定で再開する案。関心事が切り替わった dispatch（別 phase・別成果物）まで前回文脈を引き継ぎ、無関係な心証や stale な読み取りが新しい作業を汚染する。再開が有益かは呼び出し側の文脈でしか判定できないため、orchestrator の明示 opt-in とし却下。

### J. skip 判定の hash を別の gitignored transient cache に保持する

session cache（D2/D4）と同様の track 配下 transient file として実装側入力 hash を持つ案。committed artifact の schema に触れない利点はあるが、鮮度情報がそれの記述する計算結果と別 file に分離し、対応管理（どの signals file にどの cache entry が対応するか）が新たに発生する。計算産物に入力 hash を記録する自己記述の pattern は catalogue 側（declaration_hash）に既に存在し、実装側入力の hash を対称に加える方が構造が単純なため却下。なお完了 track の成果物は歴史的記録として CI 対象外という既決方針があるため、schema 変更に伴う旧 artifact の migration は不要。

### K. track 外の dispatch は resume 非対応とする

session cache を track 配下の一層に保てる単純さはあるが、pre-track ADR 起草（hearing → 起草 → 修正の再入 loop が最も多い工程の一つ）がまさに track の存在しない base branch 上で走るため、D4 の利益の相当部分を失う。既存の branch-bound track 解決の成否をそのまま分岐に流用でき、二層化の追加コストが小さいため却下。

### L. session cache を workspace 単位に一本化する

置き場が 1 つになる単純さはあるが、cache が track の削除・archive と lifecycle を共にするという track cache の規則を壊し、削除済み track の entry が workspace に残存して別途の失効管理を要するようになる。track 文脈の有無は dispatch 時の track 解決で機械的に判定できるため、置き場を一本化する利点が乏しく却下。

## Consequences

### Positive

- fast round の推論コストが下がり（実測 fast 約 20 round × 2〜4 分の短縮見込み）、二段構成の「安い前置フィルタ」という設計意図が実態と一致する。
- 再 round の文脈再構築が差分投入に置き換わり、修正確認 loop（1 finding あたり 3 round）の単価が下がる。
- writer 系 capability の再入（実測 1 track で 10 回超）と中断からの続行が、文脈再構築なしの差分指示で行える。
- 設計フェーズ中の無意味な rustdoc 再計算（毎 round 約 1〜3 分の見込み）が消える。
- D1-D3 の合算で、実測 5.5 時間の track 全工程を 2.5〜3 時間圏へ短縮する試算（D4 の writer 再入・中断続行の短縮はさらに上乗せ）。
- 全 capability の推論深度が profile に明示され、設定 file から実挙動を一意に読める（global 設定への暗黙依存の解消）。

### Negative

- fail-closed のため、導入時に committed profile の全 capability へ effort 追記が必須になる（未記入の capability は dispatch 不能）。template 利用者が capability を追加する際も effort 記入が強制される。
- reviewer 実行基盤と capability 実行基盤が session id の状態管理（保存・突合・失効処理）を持つことになり複雑化する。置き場も track 内 / track 外の二層になる。
- 再開 review には前回心証への anchoring リスクがあり、capability SSoT の常設規定（再入時も全件を再判定する権限・義務）で緩和するが排除はできない。
- 再開 dispatch には stale 文脈リスク（再入の間の上流 artifact 変更を古い読み取りで上書き判断する）があり、上流変更の自己確認・再読義務の常設規定で緩和するが排除はできない。
- fast の低 effort 化により fast の検出力が下がり、final で初めて見つかる finding が増える可能性がある（二段構成の後段が受け止める設計だが、escalation 後の手戻りは増え得る）。
- 差分有無の判定機構（D3）自体の欠陥は stale signal での review を招くため、判定は保守的（不明なら再計算）に保つ必要がある。

### Neutral

- review 結果の記録機構（scope hash による承認・round 記録）は変更しない。session 再開は文脈の再利用であり、判定・記録の単位は従来どおり round である。
- per-layer type-signals artifact の schema は実装側入力 hash の field 追加で繰り上がる。完了 track の成果物は歴史的記録（CI 対象外）のため旧 artifact の migration は発生しない。
- fast/final の二段構成・scope 分割・per-scope 並列実行は変更しない。

## Reassess When

- fast の低 effort 化後、final での新規 finding 率が有意に上がり、前置フィルタとしての fast の価値が崩れたとき。
- 再開 review での見落とし（anchoring 起因の未検出）が実測されたとき。
- provider CLI の session 再開仕様（設定引き継ぎ・失効・flag surface）が変わったとき。
- 差分判定の誤 skip による stale signal 事故が起きたとき。
- provider の effort 段階（値の語彙・最大値）が変わり、profile の明示値との対応を見直す必要が出たとき。

## Related

- `knowledge/adr/` — ADR 索引
- `.harness/config/agent-profiles.json` — capability → provider routing の SSoT（effort field の追加先）
- `.harness/workflows/track/review.md` — review workflow SSoT
- `knowledge/conventions/enforce-by-mechanism.md` — fail-open / fail-closed の使い分けの方針
