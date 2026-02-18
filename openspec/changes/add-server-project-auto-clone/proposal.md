# Change: プロジェクト追加時に自動クローンする

## Why
`POST /api/v1/projects` はレジストリ登録のみで、ローカルの clone/checkout が行われません。結果として changes を読み取れず、追加直後の操作性が悪い状態です。

## What Changes
- `POST /api/v1/projects` 成功時に、リモートブランチの検証とローカル clone/worktree の準備を行う
- clone/worktree 作成が失敗した場合は登録を完了せず、エラーとして返す
- 既存の git/pull ロジックを再利用できるように整理する

## Impact
- Affected specs: server-mode
- Affected code: src/server/api.rs, src/server/registry.rs
