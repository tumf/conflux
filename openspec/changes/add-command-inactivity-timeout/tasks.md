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

## Acceptance #1 Failure Follow-up

- [x] `openspec/changes/add-command-inactivity-timeout/specs/configuration/spec.md` の要件（デフォルト 900 秒 / 猶予 5 秒）に合わせて、`src/config/defaults.rs` の `DEFAULT_COMMAND_INACTIVITY_TIMEOUT_SECS` と `DEFAULT_COMMAND_INACTIVITY_KILL_GRACE_SECS`（および関連コメント）を修正する。
- [x] `openspec/changes/add-command-inactivity-timeout/specs/command-queue/spec.md` の要件に合わせて、無出力タイムアウト時の失敗経路で返すエラー文言に必ず `"inactivity timeout"` を含める（例: `src/command_queue.rs` の `stream_and_wait` / `execute_with_retry_streaming` の連携）。
- [x] `openspec/changes/add-command-inactivity-timeout/specs/observability/spec.md` の要件に合わせて、無出力タイムアウト warning ログに操作種別（apply/archive/resolve/analyze/acceptance）と `change_id` を含めるよう実装する（`src/command_queue.rs` の warning 出力か、呼び出し元で同等情報を付与）。

## Acceptance #2 Failure Follow-up

- [x] `src/config/mod.rs` の無出力タイムアウト関連コメントを仕様値（900 秒 / 5 秒）に更新する（`command_inactivity_timeout_secs` フィールド説明と `get_command_inactivity_*` getter 説明）。
- [x] 無出力タイムアウト warning ログに操作種別と `change_id` が実行フローで必ず入るよう、`AiCommandRunner::execute_streaming_with_retry` から `CommandQueue::execute_with_retry_streaming` へ `operation_type`/`change_id` を渡す API を追加し、apply/archive/resolve/analyze/acceptance 呼び出し側から設定する。
- [x] `src/command_queue.rs` の `Grace period expired, terminating inactive process` warning にも操作種別と `change_id` を含める。
