## 1. 設定とデフォルト値

- [x] 1.1 `command_inactivity_timeout_secs` と `command_inactivity_kill_grace_secs` のデフォルト定数を追加する（確認: `src/config/defaults.rs` に新しい定数が定義されている）
- [x] 1.2 設定フィールドを `OrchestratorConfig` に追加する（確認: `src/config/mod.rs` に Option フィールドと getter が存在する）
- [x] 1.3 `CommandQueueConfig` へ設定値を受け渡す（確認: `src/agent/runner.rs` の CommandQueueConfig 構築に新フィールドが含まれる）

## 2. 無出力タイムアウト監視

- [x] 2.1 streaming 実行で出力時刻を更新する監視ロジックを追加する（確認: `src/command_queue.rs` で stdout/stderr 受信時にタイムスタンプが更新される）
- [x] 2.2 タイムアウト検知時にプロセス終了と猶予 kill を行う（確認: `src/command_queue.rs` に inactivity timeout 判定と kill/grace 処理がある）

## 3. ログとエラー整備

- [x] 3.1 タイムアウト検知時の warning ログを追加する（確認: `src/command_queue.rs` に warning ログがあり、操作種別やタイムアウト秒を含む）
- [x] 3.2 inactivity timeout のエラー文言を統一する（確認: `OrchestratorError::AgentCommand` の文言に "inactivity timeout" が含まれる）

## 4. テスト追加

- [x] 4.1 無出力タイムアウトが発生するテストを追加する（確認: `src/command_queue.rs` のテストで sleep コマンドがタイムアウトする）
- [x] 4.2 定期出力でタイムアウトしないテストを追加する（確認: `src/command_queue.rs` のテストで一定間隔出力が成功する）
- [x] 4.3 タイムアウト無効化（0）のテストを追加する（確認: `src/command_queue.rs` のテストで無効化時にタイムアウトしない）
