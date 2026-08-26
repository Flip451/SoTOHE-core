# Security Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。ハーネスが所有するのは、規約を capability へ届ける解決機構 (`required_for` frontmatter とその resolver) と、本文が案内する gate task (`cargo make deny`) の定義だけである。**何を機密として扱い、どう守るか** という方針そのもの — 機密ディレクトリの選定、symlink 拒絶の適用範囲、レビュー観点 — はこの文書にあり、プロジェクトのものである。

この文書には `required_for` frontmatter がない。つまり出荷時点では、どの capability もこの規約を必読として解決しない。特定の capability に必読として届けたい場合は、ファイル先頭へ frontmatter を足して capability ID を列挙する。

以下に挙げる `private/` と `config/secrets/` は、テンプレートが既定として出荷する機密ディレクトリの初期値である。名前も個数もプロジェクトが決めてよい。変更する場合は §Enforcement の手順に従って `.gitignore` と `.claude/settings.json` の deny 規則を同時に更新する。

## Sensitive Directories

このプロジェクトは 2 つの機密ディレクトリを定義する。これらのディレクトリのファイルはバージョン管理へコミットしてはならず、AI エージェントに読ませてはならない。

> **強制先**: review 観点 — harness-policy scope

> **Scope of enforcement**: `.claude/settings.json` の `Read` / `Grep` deny 規則は Claude Code 自身の tool 呼び出しにだけ適用される。外部 provider の subprocess が `workspace-write` 相当の sandbox でファイルシステムへ直接到達する場合には適用されない (詳細は `.claude/rules/guardrails.md` §Sandbox and Hook Coverage Warning)。外部 subprocess を書き込み可能な sandbox で走らせるときは、`private/` と `config/secrets/` を読まないよう明示的に指示する。

> **強制先**: review 観点 — harness-policy scope
>
> **Optional container-level enforcement**: Docker 環境を選択したプロジェクトは、出荷される `compose.yml` / `compose.dev.yml` の設定で OS レベルの強制を追加できる。
> - `.git` は read-only (`:ro`) でマウントされ、コンテナからの `git add` / `git commit` は EROFS で失敗する。ただし `git push` は主に `.git` を読むだけなので成功しうる。push の抑止が必要ならネットワーク制御か hook による遮断を使う。
> - `private/` と `config/secrets/` は空の tmpfs でマスクされ、ホスト側の中身にかかわらずコンテナ内では空に見える。

> **強制先**: 強制なし (明記) — Docker 環境の選択は consumer 所有
>
> このコンテナ隔離は、ホスト上で直接実行されるコマンドや書き込み可能な sandbox の外部 subprocess を覆わない。それらの経路は上記の permission 規則と guarded workflow 規則に従う必要がある。

> **強制先**: review 観点 — harness-policy scope

### `private/`

Purpose: ローカル証明書、TLS 資格情報、SSH 鍵など、開発者のマシンから出てはならないホスト固有の秘密。

- **Git**: コミット禁止。`.gitignore` に `private/` を追加する。
  > **強制先**: review 観点 — harness-policy scope
- **AI read**: 禁止。`Read(./private/**)` と `Grep(./private/**)` を `.claude/settings.json` の deny に置く。
  > **強制先**: review 観点 — harness-policy scope
- **Typical contents**: `dev-cert.crt`、`dev-key.pem`、資格情報を埋め込んだホスト固有設定。

### `config/secrets/`

Purpose: ローカル開発向けのアプリケーションレベルの秘密 (OAuth クライアント ID、API キー、データベースパスワードなどの資格情報ファイル)。

- **Git**: コミット禁止。`.gitignore` に `config/secrets/` を追加する。
  > **強制先**: review 観点 — harness-policy scope
- **AI read**: 禁止。`Read(./config/secrets/**)` と `Grep(./config/secrets/**)` を `.claude/settings.json` の deny に置く。
  > **強制先**: review 観点 — harness-policy scope
- **Typical contents**: `local.toml`、`oauth/client.json`、環境ごとの資格情報ファイル。

## Symlink Rejection in Infrastructure Adapters

Infrastructure 層のファイル I/O アダプターは、composition root から別途信頼して受け取る root を起点に、対象までの **すべての既存パス要素**（中間ディレクトリと leaf を含む）の symlink を事前に拒絶し、対象への相対パスを root より下に閉じる。leaf と直上の親だけを検査する実装は不十分である。

### ルール

| 対象 | チェック |
|---|---|
| trusted root | composition root から別途渡され、信頼境界として扱う。対象パスはこの root より下に閉じ、`..` などで root の外へ出る対象は fail-closed エラー |
| root より下の各既存要素（中間ディレクトリと leaf を含む） | root から leaf に向かって順に `symlink_metadata()` で検査し、symlink なら fail-closed エラー |
| 新規作成する leaf | 作成前に既存の全 ancestor を上記のとおり検査する |

> **強制先**: review 観点 — infrastructure / cli_composition scope

### 理由

- symlink 経由のファイル差し替えにより、永続化した状態や metadata が外部パスへ redirect される可能性がある
- `std::fs::read_to_string` や一般的な atomic write の実装は symlink を透過的に follow する
- 中間ディレクトリの symlink も透過的に follow されるため、leaf と直近の親だけの検査では trusted root の外へ escape しうる
- symlink を含まない `../` 経路でも root の外へ出られるため、symlink 検査だけでは containment にならない

### 新規アダプター追加時

1. composition root から trusted root を別途受け取り、対象への相対パスを root より下に閉じる（root の外を指す対象は fail-closed エラー）
2. ファイル I/O の前に、root より下の既存の各 path component を root 側から順に `symlink_metadata()` で検査する
3. symlink の場合は fail-closed でエラーを返す（silent skip 禁止）
4. leaf と直近の親だけでなく、中間ディレクトリに置いた nested symlink と root 外への `../` 経路の拒絶もテストする（プラットフォーム対応に注意）

> **強制先**: review 観点 — infrastructure scope

## Security Boundary Failure Handling

秘匿・入力検証・権限判定のセキュリティ境界では、エラー時の無音の機能縮退を許さない。
構築または初期化に失敗した場合は、警告のみの通知、無効値への縮退、保護なしでの処理継続ではなく、
処理を停止してエラーを返すか fail-stop とする。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope

静的な秘匿パターンなど、構築がプログラミングエラーを示す場合も、同じ構築保証を適用する。
外部入力を使う動的な値は、検証に失敗した時点でエラーとして伝播させる。

> **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope

## Enforcement

このプロジェクトに新しい機密ディレクトリを追加するとき:

1. そのディレクトリを `.gitignore` に追加する。
2. `Read(./new-dir/**)` と `Grep(./new-dir/**)` の deny 規則を `.claude/settings.json` に追加する。
3. このファイルにディレクトリの目的を記録する。

> **強制先**: review 観点 — harness-policy scope

> **CI 強制はプロジェクトの裁量**: `.claude/settings.json` が期待どおりの `Read` / `Grep` deny エントリを持つことを CI で検証するかどうかは、プロジェクトが決める。テンプレートは推奨 deny エントリを既定として出荷し、その意図をここに記録するが、CI で hard-fail させない。この provide-not-enforce の原則は `.harness/policies/consumer-ownership.md` にある。

> **強制先**: 強制なし (明記) — sensitive-directory CI は consumer 所有

## Secrets Management

```rust
// Bad: hardcoded secrets
let api_key = "sk-1234abcd";

// Good: from environment — propagate error instead of panicking
// Implement From<std::env::VarError> for ConfigError to preserve the error kind.
fn init_config() -> Result<Config, ConfigError> {
    let api_key = std::env::var("API_KEY")?; // ? uses From<VarError> for ConfigError
    Ok(Config { api_key })
}
```

`.env` はコミットしない。`.env.example` のみコミットする。

> **強制先**: review 観点 — harness-policy scope

## Input Validation

ドメイン型のコンストラクタで検証する。newtype で不正値を構築不能にする設計原則は `prefer-type-safe-abstractions.md` にある。

> **強制先**: review 観点 — domain scope

```rust
pub struct Email(String);
impl Email {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        if !is_valid_email(s) { return Err(ValidationError::InvalidEmail); }
        Ok(Self(s.to_string()))
    }
}
```

## SQL Injection Prevention

SQL を扱う場合、クエリ文字列に外部入力を埋め込まず、必ずパラメータバインドを使う。以下は SQL クライアント crate として `sqlx` を採用した場合の例である。crate の選定はプロジェクトが ADR で決める事項であり、テンプレートは既定の SQL クライアントを出荷しない。

> **強制先**: review 観点 — infrastructure scope

```rust
// Bad
let query = format!("SELECT * FROM users WHERE id = {id}");

// Good
let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
    .bind(user_id)
    .fetch_optional(&pool)
    .await?;
```

## Error Messages

内部詳細をユーザーに漏らさない。

> **強制先**: review 観点 — infrastructure scope

```rust
// Bad: leaks internal info
Err(AppError::Database(format!("Connection to {}:{} failed", host, port)))

// Good: abstract to user, log details internally
tracing::error!("DB connection failed: host={} err={}", host, err);
Err(AppError::Internal("Service unavailable".to_string()))
```

## Dependencies

```bash
cargo make deny      # 脆弱性・ライセンス・禁止クレートチェック
```

> **強制先**: 機械 lint — cargo make deny

## Code Review Checklist

- [ ] シークレットのハードコードなし
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope
- [ ] 外部入力はドメイン型で検証済み
  > **強制先**: review 観点 — domain scope
- [ ] SQL クエリはパラメータバインド使用
  > **強制先**: review 観点 — infrastructure scope
- [ ] エラーメッセージは内部情報を漏らさない
  > **強制先**: review 観点 — infrastructure scope
- [ ] ログに機密情報が含まれていない
  > **強制先**: review 観点 — infrastructure scope
- [ ] セキュリティ境界（秘匿・検証・権限判定）で無音の機能縮退がなく、構築・初期化の失敗が停止として扱われている
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope
- [ ] `unsafe` コードは最小限かつコメント付き
  > **強制先**: review 観点 — domain / usecase / infrastructure / cli / cli_driver / cli_composition / harness-policy scope
- [ ] `cargo make deny` が通っている
  > **強制先**: 機械 lint — cargo make deny

## Related Documents

- `coding-principles.md` — エラーハンドリング、`unsafe` の扱い、パニック禁止ルール
- `prefer-type-safe-abstractions.md` — 不正値を構築不能にする newtype / enum の設計原則
- `testing.md` — symlink 拒絶などセキュリティ要件のテスト方針
