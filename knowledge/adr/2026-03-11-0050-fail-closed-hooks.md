# Fail-closed フック エラーハンドリング

## Status

Accepted

## Context

Claude Code の PreToolUse フック（guard）がエラーを起こした場合、ツール実行を許可するか拒否するか。

## Decision

Fail-closed: guard フックはエラー時（CLI not found, unexpected exception 等）にツール実行をブロックする。検証なしに進行しない。

## Rejected Alternatives

- Fail-open (silently skip guard on error): guard の存在意義が失われる。エラー時にバイパスされるセキュリティ機構は無意味

## Consequences

- Good: セキュリティガードが確実に機能する
- Bad: CLI 未ビルド時（fresh clone 等）にフックがブロックし、bootstrap が困難（INF-12）
- Bad: フック内のバグが全ツール実行を停止させる

## Reassess When

- bootstrap 時の UX 問題（INF-12）が深刻化した場合
- フックの安定性が十分に確立され、fail-open が安全と判断できる場合
