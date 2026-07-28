# Security Convention

## この文書の所有権

この規約は **利用プロジェクトが所有する**。初期値としてテンプレートから供給されるが、以後の改稿・改名・削除はプロジェクトの裁量である。ハーネスが所有するのは、規約を capability へ届ける解決機構 (`required_for` frontmatter とその resolver) と、本文が案内する gate task (`cargo make deny`) の定義だけである。**何を機密として扱い、どう守るか** という方針そのもの — 機密ディレクトリの選定、symlink 拒絶の適用範囲、レビュー観点 — はこの文書にあり、プロジェクトのものである。

この文書には `required_for` frontmatter がない。つまり出荷時点では、どの capability もこの規約を必読として解決しない。特定の capability に必読として届けたい場合は、ファイル先頭へ frontmatter を足して capability ID を列挙する。

以下に挙げる `private/` と `config/secrets/` は、テンプレートが既定として出荷する機密ディレクトリの初期値である。名前も個数もプロジェクトが決めてよい。変更する場合は §Enforcement の手順に従って `.gitignore` と `.claude/settings.json` の deny 規則を同時に更新する。

## Sensitive Directories

このプロジェクトは 2 つの機密ディレクトリを定義する。これらのディレクトリのファイルはバージョン管理へコミットしてはならず、AI エージェントに読ませてはならない。

> **Scope of enforcement**: `.claude/settings.json` の `Read` / `Grep` deny 規則は Claude Code 自身の tool 呼び出しにだけ適用される。外部 provider の subprocess が `workspace-write` 相当の sandbox でファイルシステムへ直接到達する場合には適用されない (詳細は `.claude/rules/guardrails.md` §Sandbox and Hook Coverage Warning)。外部 subprocess を書き込み可能な sandbox で走らせるときは、`private/` と `config/secrets/` を読まないよう明示的に指示する。
>
> **Optional container-level enforcement**: Docker 環境を選択したプロジェクトは、出荷される `compose.yml` / `compose.dev.yml` の設定で OS レベルの強制を追加できる。
> - `.git` は read-only (`:ro`) でマウントされ、コンテナからの `git add` / `git commit` は EROFS で失敗する。ただし `git push` は主に `.git` を読むだけなので成功しうる。push の抑止が必要ならネットワーク制御か hook による遮断を使う。
> - `private/` と `config/secrets/` は空の tmpfs でマスクされ、ホスト側の中身にかかわらずコンテナ内では空に見える。
>
> このコンテナ隔離は、ホスト上で直接実行されるコマンドや書き込み可能な sandbox の外部 subprocess を覆わない。それらの経路は上記の permission 規則と guarded workflow 規則に従う必要がある。

### `private/`

Purpose: ローカル証明書、TLS 資格情報、SSH 鍵など、開発者のマシンから出てはならないホスト固有の秘密。

- **Git**: コミット禁止。`.gitignore` に `private/` を追加する。
- **AI read**: 禁止。`Read(./private/**)` と `Grep(./private/**)` を `.claude/settings.json` の deny に置く。
- **Typical contents**: `dev-cert.crt`、`dev-key.pem`、資格情報を埋め込んだホスト固有設定。

### `config/secrets/`

Purpose: ローカル開発向けのアプリケーションレベルの秘密 (OAuth クライアント ID、API キー、データベースパスワードなどの資格情報ファイル)。

- **Git**: コミット禁止。`.gitignore` に `config/secrets/` を追加する。
- **AI read**: 禁止。`Read(./config/secrets/**)` と `Grep(./config/secrets/**)` を `.claude/settings.json` の deny に置く。
- **Typical contents**: `local.toml`、`oauth/client.json`、環境ごとの資格情報ファイル。

## Symlink Rejection in Infrastructure Adapters

infrastructure 層のファイル I/O アダプターは、composition root から別途信頼して受け取る root を起点に、対象までの **すべての既存パス要素** の symlink を I/O 前に拒絶する。leaf と直近の親だけを検査してはならない。

### ルール

| 対象 | チェック |
|---|---|
| trusted root | composition root から別途渡され、信頼境界として扱う |
| root より下の各既存要素 (中間ディレクトリと leaf を含む) | root から順に `symlink_metadata()` で検査し、symlink なら fail-closed エラー |
| 新規作成する leaf | 作成前に既存の全 ancestor を上記のとおり検査する |

### 理由

- symlink 経由のファイル差し替えにより、永続化した状態や metadata が外部パスへ redirect される可能性がある
- `std::fs::read_to_string` や一般的な atomic write の実装は symlink を透過的に follow する
- 中間ディレクトリの symlink も一般的なファイル I/O では透過的に follow されるため、leaf と直近の親だけの検査では trusted root の外へ escape しうる
- tamper-proof 対策として、ファイルアクセス前に trusted root より下の symlink をすべて検出して拒絶する

### 新規アダプター追加時

1. composition root から trusted root を別途受け取り、対象への相対パスを root より下に閉じる
2. ファイル I/O の前に、root より下の既存の各 path component を順に `symlink_metadata()` で検査する
3. symlink の場合は fail-closed でエラーを返す (silent skip 禁止)
4. leaf と直近の親だけでなく、中間ディレクトリに置いた nested symlink も含めて拒絶をテストする (プラットフォーム対応に注意)

## Enforcement

このプロジェクトに新しい機密ディレクトリを追加するとき:

1. そのディレクトリを `.gitignore` に追加する。
2. `Read(./new-dir/**)` と `Grep(./new-dir/**)` の deny 規則を `.claude/settings.json` に追加する。
3. このファイルにディレクトリの目的を記録する。

> **CI 強制はプロジェクトの裁量**: `.claude/settings.json` が期待どおりの `Read` / `Grep` deny エントリを持つことを CI で検証するかどうかは、プロジェクトが決める。テンプレートは推奨 deny エントリを既定として出荷し、その意図をここに記録するが、CI で hard-fail させない。この provide-not-enforce の原則は `.harness/policies/consumer-ownership.md` にある。

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

## Input Validation

ドメイン型のコンストラクタで検証する。newtype で不正値を構築不能にする設計原則は `prefer-type-safe-abstractions.md` にある。

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

## Code Review Checklist

- [ ] シークレットのハードコードなし
- [ ] 外部入力はドメイン型で検証済み
- [ ] SQL クエリはパラメータバインド使用
- [ ] エラーメッセージは内部情報を漏らさない
- [ ] ログに機密情報が含まれていない
- [ ] `unsafe` コードは最小限かつコメント付き
- [ ] ファイル I/O アダプターが trusted root より下の leaf・親・中間ディレクトリの symlink を fail-closed で拒絶している
- [ ] `cargo make deny` が通っている

## Related Documents

- `coding-principles.md` — エラーハンドリング、`unsafe` の扱い、パニック禁止ルール
- `prefer-type-safe-abstractions.md` — 不正値を構築不能にする newtype / enum の設計原則
- `testing.md` — symlink 拒絶などセキュリティ要件のテスト方針
