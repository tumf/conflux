## 1. 実装
- [ ] 1.1 acceptance failure の follow-up 追記フォーマットを更新する
  確認: `src/orchestration/acceptance.rs` が `## Acceptance #<n> Failure Follow-up` と各 finding の `- [ ]` 行のみを生成し、`Address acceptance findings` の固定行やネスト箇条書きを含まない
- [ ] 1.2 acceptance failure の試行番号を follow-up に反映する
  確認: `src/serial_run_service.rs` と `src/parallel/mod.rs` が acceptance 失敗時に試行番号を渡し、同一 change の再失敗で `Acceptance #2` 以降が付与される
- [ ] 1.3 follow-up フォーマットのユニットテストを追加する
  確認: `src/orchestration/acceptance.rs` のテストで見出しとタスク行の生成を検証する

## 2. 検証
- [ ] 2.1 `cargo test update_tasks_on_acceptance_failure` を実行する
  確認: follow-up フォーマットのテストが成功する
