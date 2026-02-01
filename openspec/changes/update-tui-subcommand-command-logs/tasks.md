## 1. Implementation
- [x] 1.1 ExecutionEventにacceptance開始コマンドの伝達経路を追加する（serial/parallelの開始イベントでcommandを含め、TUIログでCommand行が出る）
  - 完了確認: src/events.rsのAcceptanceStartedがcommandを保持し、src/tui/state/events/completion.rsでCommand行が出力される
- [x] 1.2 シリアル実行のサブコマンド出力にoperationを付与する（apply以外はacceptance/archive/resolve）
  - 完了確認: src/tui/orchestrator.rsのChannelOutputHandlerがoperation trackerを経由してサブコマンド種別に応じてoperationを設定する
- [x] 1.3 パラレル実行でacceptanceの開始コマンドをイベント経由でTUIに表示する
  - 完了確認: src/parallel/executor.rsでAcceptanceStartedのcommandが送信され、TUIログにCommand行が出る
- [x] 1.4 ログ表示の回帰テストを追加/更新する
  - 完了確認: src/events.rsにAcceptanceStarted/ArchiveStarted/ResolveStartedイベントのテストを追加

## Acceptance #1 Failure Follow-up
- [x] Gitの作業ツリーがクリーンではありません。未コミットの変更を解消する（Modified: openspec/changes/update-tui-subcommand-command-logs/tasks.md, src/events.rs, src/orchestration/mod.rs, src/orchestration/output.rs, src/orchestrator.rs, src/parallel/executor.rs, src/serial_run_service.rs, src/tui/orchestrator.rs, src/tui/state/events/completion.rs, src/tui/state/events/mod.rs, src/web/state.rs）
  - 完了確認: すべての変更をコミットし、ビルドとテストが成功した（commit 0b33cfa1）
