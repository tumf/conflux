# Change: ワークスペース再開時のarchive判定を強化する

## Why
archive 済みの change が再起動後に acceptance から再実行されてしまう。worktree の HEAD ツリーでは変更が archive に移動済みであるため、archive 完了後から再開できる判定が必要である。

## What Changes
- ワークスペース再開時の archive 判定を worktree の HEAD ツリー状態と archive エントリの有無で判断できるようにする
- `Archive: <change_id>` コミットが HEAD 以外でも archived と判定できるようにする
- 判定ロジックのテストケースを追加・更新する

## Impact
- Affected specs: `parallel-execution`
- Affected code: `src/execution/state.rs`, `src/execution/archive.rs`, `src/parallel/mod.rs`
