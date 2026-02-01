## 1. Implementation
- [ ] 1.1 ExecutionEventにacceptance開始コマンドの伝達経路を追加する（serial/parallelの開始イベントでcommandを含め、TUIログでCommand行が出る）
  - 完了確認: src/events.rsのAcceptanceStartedがcommandを保持し、src/tui/state/events/stages.rsでCommand行が出力される
- [ ] 1.2 シリアル実行のサブコマンド出力にoperationを付与する（apply以外はacceptance/archive/resolve）
  - 完了確認: src/tui/orchestrator.rsのChannelOutputHandlerがサブコマンド種別に応じてoperationを設定する
- [ ] 1.3 パラレル実行でacceptanceの開始コマンドをイベント経由でTUIに表示する
  - 完了確認: src/parallel/executor.rsでAcceptanceStartedのcommandが送信され、TUIログにCommand行が出る
- [ ] 1.4 ログ表示の回帰テストを追加/更新する
  - 完了確認: src/tui/render.rsまたは関連テストにサブコマンドのヘッダー/Command表示の検証がある
