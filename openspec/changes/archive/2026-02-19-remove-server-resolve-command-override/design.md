## Context
server モードの git auto_resolve が `server.resolve_command` を要求する実装になっており、既存のトップレベル `resolve_command` と設定が二重化されている。ユーザーは起動時に追加フラグを渡さず、設定ファイルの `resolve_command` をそのまま使いたい。

## Goals / Non-Goals
- Goals:
  - サーバの auto_resolve がトップレベルの `resolve_command` を使用する
  - `server.resolve_command` と `--resolve-command` を廃止し、設定の一本化を行う
- Non-Goals:
  - resolve_command のテンプレート展開やシェル解釈方式の変更
  - auto_resolve の挙動自体（merge/rebase 戦略など）の追加変更

## Decisions
- Decision: サーバは設定マージ済みのトップレベル `resolve_command` を参照する
  - 理由: 既存のオーケストレーター設定と同一キーで一貫性があるため
- Decision: `server.resolve_command` が存在する場合は設定エラーとする
  - 理由: サイレントな無視や別挙動による混乱を避けるため
- Decision: `cflx server --resolve-command` を廃止し、不明フラグとしてエラーにする
  - 理由: 起動時フラグによる分岐をなくし、設定ファイルに統一するため

## Risks / Trade-offs
- 既存で `server.resolve_command` を使っている環境が即座にエラーになる
  - Mitigation: 変更点を明確にし、`resolve_command` へ移行するガイドを追加する

## Migration Plan
- `server.resolve_command` を削除し、同じ値をトップレベル `resolve_command` に移す
- `cflx server --resolve-command` を使用している場合は削除する

## Open Questions
- なし
