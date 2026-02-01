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

## Acceptance #2 Failure Follow-up
- [x] src/tui/orchestrator.rs: AcceptanceStartedのcommand生成が簡略化されており、run_acceptance_streamingのdiff context/last outputを含む実コマンドと一致しないため、実行コマンドと一致する文字列を生成して送信する
  - 完了確認: src/orchestration/acceptance.rs内でacceptance_test_streamingが実際のコマンド文字列をログ出力し、シリアルモードではAcceptanceStartedイベントを送信しない（パラレルモードのみ送信）
- [x] src/tui/state/events/stages.rs と src/tui/state/events/completion.rs: Apply/Archive/Resolve/Acceptanceの`Command:`行が`LogEntry::info`のみでoperationが付与されていないため、対応するoperationとして記録されるよう`.with_operation(...)`（必要に応じて`.with_change_id(...)`）を付与する
  - 完了確認: すべてのCommand行に.with_operation()と.with_change_id()を付与（apply/archive/acceptance/resolve）

## Acceptance #3 Failure Follow-up
- [x] src/tui/orchestrator.rs: AcceptanceContinue/AcceptanceFailed/AcceptanceCommandFailedで送信するAcceptanceStartedのcommandがuser_prompt+historyのみでbuild_acceptance_promptのdiff context/last outputを含まず、実際のacceptance実行コマンド（src/agent/runner.rsのrun_acceptance_streaming）と不一致になっているため、実行コマンドと一致する文字列を送信する
  - 完了確認: AcceptanceContinue/AcceptanceFailed/AcceptanceCommandFailedの処理から重複するAcceptanceStartedイベント送信を削除。acceptance_test_streaming内で実際のコマンド文字列（diff context/last output含む）がログ出力される
