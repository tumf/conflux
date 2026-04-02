## Implementation Tasks

- [x] 1. rejection flow の canonical spec を、`REJECTED.md` only commit へ更新する (verification: orchestration-state / parallel-execution spec delta に reject flow が `REJECTED.md` only であることが明記されている)
- [x] 2. `src/orchestration/rejection.rs` から `openspec resolve` 依存を除去し、base commit に `REJECTED.md` 以外が含まれないことを保証する (verification: reject flow が `git add openspec/changes/<change_id>/REJECTED.md` のみを行い、resolve 呼び出しが存在しない)
- [x] 3. serial / parallel の rejection result handling を、`resolve` ではなく reject marker commit 完了で終端扱いするよう整合させる (verification: `src/serial_run_service.rs` と `src/parallel/dispatch.rs` / `queue_state.rs` で reject flow 完了条件が `REJECTED.md` marker commit ベースになっている)
- [x] 4. rejected change の一覧/状態判定を `REJECTED.md` marker と runtime terminal state に基づいて維持し、reject flow が `resolve` を呼ばなくても UI/queue で `rejected` と扱えるようにする (verification: rejected change が frontend と state machine で `rejected` 表示になり、再キューされない)
- [x] 5. reject flow の回帰テストを追加し、base commit diff が `REJECTED.md` のみであることと worktree cleanup が実行されることを検証する (verification: targeted Rust tests が追加される)

## Future Work

- rejected change を archive ツリーへ移す専用 policy が必要かを別 proposal で検討する
- `REJECTED.md` marker を使う active change filtering と historical audit UX の見直し
