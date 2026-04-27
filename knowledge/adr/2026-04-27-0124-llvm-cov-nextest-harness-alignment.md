# `cargo make llvm-cov` を nextest 経路に統一する

## Context

`cargo make llvm-cov` が `apps/cli/src/commands/review/tests.rs` の 13 テストで失敗していた一方、`cargo make test` (nextest 経由) は 2062 件全 pass という乖離が観測された。失敗内訳は 1 件が実 assertion 失敗、残り 12 件は `PoisonError` の cascade。

根本原因は test harness の差にある:

- `cargo llvm-cov --html` は内部で **`cargo test` の標準 libtest harness** (単一プロセス + parallel threads) を使う
- `cargo make test` は **`cargo nextest run`** (process per test) を使う

`apps/cli/src/commands/review/tests.rs` は次に依存している:

- `env_lock()` (単一 `Mutex<()>`) で `env::set_var` を直列化
- `EnvVarGuard` で `SOTP_FAKE_CODEX_EXIT_CODE` 等を一時設定
- fake codex shell script を subprocess 起動

`env::set_var` は Rust 1.80+ で unsafe 化された通り、他 thread が env を read している状況では本質的に race-prone。nextest の per-process 隔離で暗黙に守られていた前提が、plain `cargo test` の in-process thread harness で露出する。先頭テストが assertion で panic → `env_lock` を握ったまま落ちる → Mutex poisoned → 以降 `.lock().unwrap()` が cascade 失敗、という連鎖が観測された。

## Decision

### D1: `Makefile.toml` の `llvm-cov-local` task で `cargo llvm-cov nextest` 経由に切り替える

`Makefile.toml` (L445-449) の `llvm-cov-local` task の args を以下に変更する:

<!-- illustrative, non-canonical -->
- 旧: `["llvm-cov", "--html", "--all-features"]`
- 新: `["llvm-cov", "nextest", "--html", "--all-features", "--locked"]`

`cargo llvm-cov` は `nextest` サブコマンドを持ち、内部で `cargo nextest run` を呼んで計測する。これにより llvm-cov も per-process 隔離になり、`cargo make test` と test harness が揃う。検証済み:

<!-- illustrative, non-canonical -->
```
docker compose run --rm tools cargo llvm-cov nextest --html --all-features --locked
→ 2062 件全 pass + HTML レポート生成成功
```

### D2: env mutation 方式そのもの (DI 化 / `serial_test`) の対応は別 track として deferred する

本決定の scope は llvm-cov harness 差の直接原因のみ。`apps/cli/src/commands/review/tests.rs` の env mutation + `env_lock()` 方式は Rust 1.80+ の `env::set_var` unsafe 化を踏まえると中長期的に脆いが、本 ADR では対象外とする。中長期改善案 (`CodexLocalArgs` への exec path 注入による DI 化、`serial_test` クレート導入など) は必要になったタイミングで別 ADR / 別 track として切る。

## Rejected Alternatives

### A. env mutation 方式そのものを直す (DI 化 / `serial_test` クレート)

`apps/cli/src/commands/review/tests.rs` の env mutation を排除する方向に進めば、libtest harness でも race しなくなり llvm-cov の失敗も消える。却下理由: llvm-cov harness 差の直接原因ではなく、scope を分けるべき。1 行修正で `cargo make test` と `cargo make llvm-cov` を同じ nextest 経路に揃えるほうが最小修正かつ可逆性が高い。test 設計の再検討は別 track の判断材料が揃ってからに送る (D2 で deferred 明示)。

### B. `RUST_TEST_THREADS=1` で libtest を直列化する

`cargo llvm-cov --html` の内部 `cargo test` 起動時に `RUST_TEST_THREADS=1` を渡せば in-process でも直列化される。却下理由: `env::set_var` の thread race は「他 thread が env を read していると発生する」ため、テスト実行 thread を 1 にしても環境変数を read する別 thread (libtest 自身が span する monitoring thread や std 内部 thread) が並走している場合に防げる保証がない。また `cargo make test` (nextest) との harness 差が残るため、再発リスクが残る。

### C. `cargo make llvm-cov` を撤去する (coverage を諦める)

llvm-cov を CI から外せば失敗自体は消える。却下理由: ADR `2026-03-24-0900-coverage-not-a-signal.md` で coverage は信号機ではないが補助指標として残す方針が確認されている。撤去は方針逆行であり、1 行修正で済む選択肢が他にある以上、過剰対応。

## Consequences

### Positive

- `cargo make llvm-cov` が通る (2062 件全 pass + HTML レポート生成)
- test harness が `cargo make test` と統一され、harness 差由来の不整合が将来も再発しにくい
- 修正は `Makefile.toml` 1 行のみで、可逆性が高い (元に戻すのも 1 行)

### Negative

- `cargo llvm-cov nextest` サブコマンドへの依存が深まる (将来サブコマンドが廃止・仕様変更されると影響する)
- env mutation race そのものは未解決のまま残る (libtest 経路を再採用すると即座に再発する)

### Neutral

- coverage の数値そのものへの影響は基本的にない (両 harness とも同じテストを実行する)

## Reassess When

- Rust 標準が `env::set_var` を thread-safe に置き換える、または `std::env` API が刷新される
- `cargo llvm-cov nextest` サブコマンドの仕様変更 / 廃止 / parity 喪失
- `apps/cli/src/commands/review/tests.rs` の env mutation 方式が DI 化 / `serial_test` 化されて libtest harness でも race しなくなった (この時点で本 ADR は意義を失い、`cargo llvm-cov --html` への戻しを検討してよい)
- coverage を CI 上で信号機として再評価する判断 (現方針: `2026-03-24-0900-coverage-not-a-signal.md`) が変わる

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md` — coverage は信号機ではなく CI 補助指標
- `Makefile.toml` — `llvm-cov-local` task (修正対象)
- `apps/cli/src/commands/review/tests.rs` — env mutation race の発生源 (D2 で deferred)
