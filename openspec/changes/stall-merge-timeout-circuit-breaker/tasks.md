## 1. 要件・設計
- [ ] 1.1 既存仕様（circuit-breaker/parallel-execution/cli/web-monitoring/configuration）を確認し、merge 停滞監視の差分を整理する
- [ ] 1.2 監視対象（serial/parallel 両方、Merge change: <change_id>）と停止条件を設計にまとめる
- [ ] 1.3 監視タスクの起動/停止タイミングとキャンセル連携を設計にまとめる
- [ ] 1.4 設定項目（閾値・監視間隔）とデフォルト値を決める

## 2. 実装
- [ ] 2.1 merge 進捗監視のストール検知ロジックを追加する
- [ ] 2.2 監視タスクを orchestrator/run loop に統合する（serial/parallel 両方）
- [ ] 2.3 stall 検知時に CancellationToken を発火し、即時停止する
- [ ] 2.4 CLI/TUI/Web の停止メッセージに stall 原因を反映する
- [ ] 2.5 設定値を読み込み、デフォルト/上書きの挙動を実装する

## 3. 検証
- [ ] 3.1 監視タイムアウト時に停止イベントが発生するテストを追加する
- [ ] 3.2 設定値を変更した場合の挙動を検証する
- [ ] 3.3 既存の stall/circuit-breaker と干渉しないことを確認する
