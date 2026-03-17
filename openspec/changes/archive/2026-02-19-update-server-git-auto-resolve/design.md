## Context
server API の git/pull・git/push は non-fast-forward を検知すると 422 を返して終了する。API テストで resolve_command の実行を確認するには、明示的に auto_resolve を有効化した場合のみ解決処理を走らせる必要がある。

## Goals / Non-Goals
- Goals:
  - `auto_resolve=true` のときに resolve_command が実行されることを API テストで観測できる
  - 既存挙動（明示エラー）をデフォルトで維持する
- Non-Goals:
  - `auto_resolve` 未指定時に暗黙の履歴変更を行う
  - 既存の並列実行・TUI フローを変更する

## Decisions
- Decision: `git/pull` と `git/push` に `auto_resolve`（boolean）と `resolve_strategy`（merge|rebase）を追加する
  - 理由: API テスト用途の明示的なスイッチが必要で、既存の安全挙動を壊さないため
- Decision: `resolve_strategy` の既定は `merge` とする
  - 理由: rebase は履歴を書き換えるためデフォルトに不向き

## Risks / Trade-offs
- 履歴操作をサーバ側で行うため、auto_resolve 有効時の副作用が大きい
  - Mitigation: auto_resolve を明示的に指定した場合のみ実行する
- resolve_command の実行結果が失敗した場合、ワークツリーが中途半端な状態になりうる
  - Mitigation: 失敗時は処理を中断して明示的エラーを返し、状態をログに残す

## Migration Plan
- 既存 API は `auto_resolve` を指定しない限り変更なし
- テストでのみ `auto_resolve` を使用し、現行クライアントへの影響はない

## Open Questions
- なし
