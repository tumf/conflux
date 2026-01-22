## 1. 実装
- [x] 1.1 acceptance failure の follow-up 追記フォーマットを更新する
  確認: `src/orchestration/acceptance.rs` が `## Acceptance #<n> Failure Follow-up` と各 finding の `- [ ]` 行のみを生成し、`Address acceptance findings` の固定行やネスト箇条書きを含まない
- [x] 1.2 acceptance failure の試行番号を follow-up に反映する
  確認: `src/serial_run_service.rs` と `src/parallel/mod.rs` が acceptance 失敗時に試行番号を渡し、同一 change の再失敗で `Acceptance #2` 以降が付与される
- [x] 1.3 follow-up フォーマットのユニットテストを追加する
  確認: `src/orchestration/acceptance.rs` のテストで見出しとタスク行の生成を検証する

## 2. 検証
- [x] 2.1 `cargo test update_tasks_on_acceptance_failure` を実行する
  確認: follow-up フォーマットのテストが成功する


## Acceptance #1 Failure Follow-up
- [x] `src/serial_run_service.rs:428` で `update_tasks_on_acceptance_failure` に渡す試行番号として `agent.next_acceptance_attempt_number` を再計算していますが、`src/orchestration/acceptance.rs:156` で同関数を使って失敗試行がすでに記録済みのため、`## Acceptance #<n>` が実際の失敗試行より 1 つ先の番号になります。
- [x] `src/parallel/mod.rs:2054` で `acceptance_iteration = agent.next_acceptance_attempt_number(&change_id)` を `execute_acceptance_in_workspace` 実行後に取得しており、`src/parallel/executor.rs:1439` の記録済み試行数より 1 つ先の値になります。これが `src/parallel/mod.rs:2127`/`src/parallel/mod.rs:2168` の `update_tasks_on_acceptance_failure` に渡され、フォローアップ番号がズレます。
- [x] `src/parallel/mod.rs:1378` のリジューム経路で `AcceptanceResult::Fail` の分岐に `update_tasks_on_acceptance_failure` がなく、リジューム中の acceptance 失敗で `tasks.md` が更新されません。
