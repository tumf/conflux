## Context
server の Git 同期は `git/pull` / `git/push` の 2 エンドポイントに分割されており、クライアントは分岐と状態集約が必要になる。これを `git/sync` に統合し、resolve を必須化する。

## Goals / Non-Goals
- Goals:
  - クライアントが `git/sync` 1 本で同期できる
  - resolve_command が必ず実行される
  - push/pull は互換性のために残すが内部で `sync` に委譲する
- Non-Goals:
  - resolve_command の実行方式（テンプレート/引数）を変更する
  - Git のリモート設定や認証方式を変更する

## Decisions
- Decision: `sync` は常に resolve_command を実行する
  - 理由: 事前分岐を排除し、API 呼び出しの一貫性を高めるため
- Decision: `sync` レスポンスは pull/push の各結果を内包する
  - 理由: クライアントが成否を 1 回のレスポンスで判断できるため

## Risks / Trade-offs
- resolve_command が未設定の環境では `sync` が必ず失敗する
  - Mitigation: エラーメッセージを明確化し、設定方法を案内する

## Migration Plan
- クライアントは `git/sync` を優先して使用する
- 既存 `git/pull` / `git/push` は互換ルートとして残す

## Open Questions
- なし
