# Observations — ci-shorten-review-gate-fix-2026-06-01

## T001: CI timing baseline audit (2026-06-01)

監査対象: `.github/workflows/ci.yml`, `compose.yml`, `Dockerfile`, `Makefile.toml`。
所要時間は GitHub Actions の実行ログ(`gh run list/view`)で測定済み。本監査は変更を加えず、T002 の入力として候補レバーを絞り込むことが目的(原因は ADR どおり未確定として扱う)。

### 計測ベースライン(GitHub Actions)

- 重量級ネイティブ依存(lancedb/ort/fastembed/arrow)追加前: 約 3〜4 分。
- 追加後の steady state(warm cache): 約 14〜16 分。cold cache スパイク: 28〜33 分。
- steady 16 分実行の step 内訳(`gh run view --json jobs`):
  - `Run CI suite`(= `docker exec ci-runner cargo make ci-container`)= **約 783 秒(13 分)** ← 悪化の本体。
  - `Build tools image with cache` = 159 秒(gha buildkit cache hit、速い)。
  - sccache restore = 約 10 秒(restore 自体は速い)。

→ 悪化の本体は image build でも cache restore でもなく、**コンテナ内 `cargo make ci-container`(重量級依存グラフの compile/link/test)**。

### キャッシュレバーの所在

1. **実行時 sccache**: `compose.yml` の `SCCACHE_DIR=/workspace/.cache/sccache`、`ci.yml` の actions/cache(key=`${runner.os}-sccache-${hashFiles('Dockerfile','**/Cargo.lock')}`、`restore-keys: ${runner.os}-sccache-`)。Cargo.lock 変更で key bust、新規重量級 crate は entry なしで cold compile。
2. **ビルド時 sccache**: `Dockerfile` builder 系の `/opt/sccache`(gha buildkit cache、`build-push-action` の cache-from/to type=gha)。実行時 sccache とは別ディレクトリ。
3. **image レイヤの cook 済み依存**: `Dockerfile` `dev-base-ci` が `cargo chef cook --all-targets --all-features` で依存を **image の実レイヤ `/workspace/target`** に焼き込む(cache mount ではない)。gha buildkit cache 化。

### 候補となる主因(未確定・要検証 / T002 で実測)

- `compose.yml` は host `.` を `/workspace` に bind-mount し `CARGO_TARGET_DIR=/workspace/target` を指す。GHA は host `target/` をキャッシュしないため毎ラン空であり、**空の host `target/` が image に焼かれた cook 済み依存を shadow** している疑いが強い。結果、実行時 `cargo make ci-container` がフルにビルドを駆動し、build script 再実行(prost-build/protoc codegen、cc compile)とリンクが走る。sccache はコンパイル出力をキャッシュするが **build script 実行とリンクはキャッシュしない**ため、空 `target/` だとこのコストがそのまま残る(783 秒と整合)。

### T002 候補レバー(キャッシュ戦略のみ・ソース/依存不変)

- **C(最有力候補)**: CI 専用の compose override で `/workspace/target` を host bind-mount で覆わない(匿名/名前付き volume にする)。image の cook 済み成果物が実行時に使われ、workspace crate のみ再 compile(baseline 相当)に戻る見込み。既存の image cook 機構を再利用するだけで追加キャッシュストレージ不要。要対処: volume init 時の owner(image build 時 vs 実行時 `HOST_UID`)権限。
- B: host `target/` を actions/cache でキャッシュ。target/ が巨大(GHA cache 10GB/repo・LRU)で sccache cache と予算を取り合う。image が cook 済みなので冗長気味。
- sccache `restore-keys` 拡張: 既に最大幅 fallback あり。target/ shadow を解決せず効果限定的。
- 重量級依存を焼いた image を registry push して `FROM`: 最も堅牢だが registry/push 運用が必要で純キャッシュの範囲を超える。

### 非原因(再調査不要)

- `.dockerignore`(`target/`/`.cache/`/`tmp/` 等を除外)は Dockerfile が `COPY` する対象を一切除外していない(`vendor/` は除外されず正しく COPY)。ビルドを壊さない。
- 「キャッシュが全面的に死んでいる」わけではない(steady 15 分で cold spike の 33 分ではない)。重量級依存だけが両キャッシュ層を貫通している。

### 測定方法の注意(T002 / T004 の acceptance に関わる)

CI 所要時間(AC-04)の実測は **GitHub Actions の実行(push/PR)でのみ**可能。ローカルの `cargo make ci` は GHA の bind-mount/cache トポロジを再現しないため duration の代理にならない。したがって T002 の「duration measurably reduced」確認は本トラック PR(adr2pr Step 10 / `/track:pr-review` の push 後)で行うのが妥当。純キャッシュ変更で目標に届かない場合は scope-boundary finding として報告する。

## T002: CI cache strategy fix (2026-06-01)

### 変更内容

**Option C(推奨候補)** を実装。

1. **`compose.ci.yml`** (新規) — CI 専用の compose override。名前付き volume `ci-target` を `/workspace/target` にマウントすることで、`compose.yml` の host bind-mount(`.:/workspace`)が `/workspace/target` をサブパスとして覆い隠す現象を解消する。`compose.yml` 自体は変更していないので、ローカル開発は従来どおり host `target/` を使用する。

2. **`.github/workflows/ci.yml`** (変更) — `docker compose run` を `docker compose -f compose.yml -f compose.ci.yml run` に変更して CI override を適用。さらに、`ci-runner` コンテナ起動前に `Seed ci-target volume from image` ステップを追加した。このステップは `docker volume create ci-target` 後、host workspace を bind-mount しない短命コンテナで `ci-target` だけを `/workspace/target` にマウントする。これにより named volume の初期化元を image レイヤの `/workspace/target` に固定し、`dev-base-ci` ステージが焼き込んだ cook 済み依存を volume に seed してから本体 CI コンテナを起動する。

### 権限問題の対処

`dev-base-ci` ステージは root で `/workspace/target` を生成するため、named volume の初期化コンテンツも root 所有になる。`compose.yml` の `user: "${HOST_UID}:${HOST_GID}"` で動く runtime container はこのディレクトリに書き込めず `cargo` が失敗する。  
対処: `Seed ci-target volume from image` の短命コンテナを root で実行し、`ci-target` volume を seed した直後に `chown -R "${HOST_UID}:${HOST_GID}" /workspace/target` で runtime user に所有権を渡す。この方法を選んだ理由:
- 既存の `Fix cache permissions` ステップ(sudo chown)と同じ思想で一貫性がある。
- 本体の `ci-runner` を root で起動する案(user: "0:0")と比べ、`compose.ci.yml` での実行ユーザー変更が不要でシンプル。
- `chown` の対象が `/workspace/target` のみに限定され、他のマウントポイントに副作用がない。

### ローカル CI 検証

`cargo make ci` をローカルで実行し合格を確認(CI は `compose.yml` のみ使用のため挙動変化なし)。`libs/`・`apps/`・`Cargo.toml`・`Cargo.lock` への変更はゼロ。

### duration 検証の繰り延べ(AC-04)

実際の CI 所要時間削減(AC-04)は GitHub Actions の実行(push/PR)でのみ確認可能。本変更は純粋なキャッシュ設定変更であり、duration 検証はトラック PR の GHA run に繰り延べる。

### volume-seeding の扱い

当初懸念: `ci-target` named volume を本体 `ci-runner` 起動時に初期化すると、`/workspace` bind-mount の下にあるため、seed 元が image レイヤの `/workspace/target` ではなく host `target/` になる恐れがあった。その場合 volume は空のままで cache 再利用の狙いが defeat される(CI は壊れず、最悪「速くならない」だけ)。

対処済み: workflow に pre-start の `Seed ci-target volume from image` ステップを入れ、host workspace を bind-mount しない短命コンテナで `ci-target` だけを `/workspace/target` にマウントして volume を初期化する。これにより seed 元は image レイヤの `/workspace/target` になり、overlapping bind-mount 下で初期化される曖昧さを避ける。

**ユーザー決定(2026-06-01): Option C のまま PR の GitHub Actions run で実測・反復する。** PR 検証手順: PR の CI で `Run CI suite` step の所要時間を観測する。
- 依然 ~13 分のままなら、volume seed 以外の要因(例: cook 済み artifact の再利用不能、workspace crate 側の compile/link 支配、別 cache key の不一致)を再調査し、Option B(host `target/` actions/cache 化)や cook 先/CARGO_TARGET_DIR の見直しへ反復する。
- baseline(~4 分)に近づけば Option C が機能している。

## T002 反復: PR #147 の GHA 実測結果と真因(2026-06-01)

### Option C は効かなかった(実測)
PR #147 の GHA CI(commit 9cef4a08)で `Run CI suite` = 776s、ベースライン 783s とほぼ不変。Seed step は 9s で走ったが短縮効果ゼロ。

### sccache 統計の duration 別比較(decisive)
| Run | duration | Compile requests | Rust misses | Rust hit rate | C/C++ hit rate |
|---|---|---|---|---|---|
| ~4分 baseline(pre-lancedb) | ~4分 | 275 | 186 | 0.00% | — |
| ~28分 cold spike | ~28分 | 1327 | 1012 | 0.00% | 100% |
| ~15分 steady | ~15分 | 1327 | 1012 | 0.00% | 100% |
| PR #147(Option C) | 776s | 1327 | 1012 | 0.00% | 100% |

**sccache の Rust ヒット率は全 run で一貫して 0%**(~4分 baseline ですら)。sccache は CI 間で Rust を一度もキャッシュできていない慢性問題。CI 時間は毎回スクラッチ compile する Rust crate 数(186→1012)で決まっていた。Option C(volume）は数値を一切動かさず。

### 真因(Codex + Claude の独立並列調査が一致)
**`CARGO_INCREMENTAL` が CI で 0 に設定されていない。** Cargo の dev/test profile は incremental=true がデフォルトで、rustc に `-C incremental=<path>` が付く。**sccache は incremental compile をキャッシュできない**(sccache 公式 Rust ドキュメントが明記)。C/C++ は cargo の incremental 経路を使わないので 100% ヒット → 「Rust 0% / C/C++ 100%」の正体。

### 修正(cache-config のみ・機能不変)
1. Option C(`compose.ci.yml` の named volume + `Seed ci-target volume from image` step)を **revert**(0% を動かさず、sccache が真因のため不要)。
2. `Run CI suite` を `docker exec -e CARGO_INCREMENTAL=0 ci-runner cargo make ci-container` に変更(runtime build を incremental 無効化 → rustc 結果が sccache キャッシュ可能になる)。
3. sccache の actions/cache key を `-v2` セグメント追加で bump(現行 key の incremental 汚染キャッシュは actions/cache 仕様で再保存されないため、フレッシュなキャッシュを保存させる)。

### 検証(PR で 2 run 必要)
- run1(`-v2` cold): フルコンパイルだが今回は cacheable → 新キャッシュ保存。duration はまだ長い見込み。
- run2(同 key restore): `sccache --show-stats` の **Rust hit rate が 0% → 高ヒット率**に上がり、`Run CI suite` が大幅短縮されれば解消。

### 検証結果(PR #147、commit d4f767e9、2026-06-01)— 解消

| 指標 | 修正前(steady) | run1(`-v2` cold) | **run2(`-v2` warm rerun)** |
|---|---|---|---|
| Run CI suite | 776s(~13分) | ~cold | **141s(~2.4分)** |
| sccache Rust hit rate | 0.00% | 0%(cold) | **100.00%** |
| Cache misses | 1012 | — | **0** |
| Cache hits (Rust) | 0 | — | **1020** |
| CI 全体 | ~15-16分 | — | **~5.6分** |

`Run CI suite` 776s → **141s(約 5.5 倍高速化)**、CI 全体 ~15分 → **~5.6分**。AC-04(目標 ≤6分)達成。フィーチャーゲートなし・cache-config(`CARGO_INCREMENTAL=0` + sccache key `-v2` bump)のみで解消。run1 は `-v2` cold でフレッシュキャッシュを保存、run2 で 100% ヒット。
