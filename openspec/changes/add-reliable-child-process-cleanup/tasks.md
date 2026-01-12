# Tasks: 子プロセスの確実なクリーンアップ

## 1. 依存クレートの追加

- [ ] 1.1 `Cargo.toml` に `nix = "0.27"` を追加（Unix系プロセスグループ管理用）
- [ ] 1.2 `Cargo.toml` に `windows = { version = "0.52", features = ["Win32_System_JobObjects", "Win32_Foundation"] }` を追加（Windows ジョブオブジェクト用）
- [ ] 1.3 ビルドが通ることを確認（`cargo check --all-targets`）

## 2. プロセス管理モジュールの実装

- [ ] 2.1 `src/process_manager.rs` を新規作成し、クロスプラットフォームのプロセス管理抽象化を実装
  - Unix: `ProcessGroupManager` 構造体（`setpgid` + `killpg`）
  - Windows: `JobObjectManager` 構造体（ジョブオブジェクト管理）
  - 共通: `ProcessHandle` トレイトで統一インターフェース提供
- [ ] 2.2 Unix 用の `kill_process_group()` 関数を実装（`nix::sys::signal::killpg` 使用）
- [ ] 2.3 Windows 用の `assign_to_job()` 関数を実装（ジョブオブジェクト作成と割り当て）
- [ ] 2.4 `src/lib.rs` または `src/main.rs` で `mod process_manager;` を宣言

## 3. agent.rs の子プロセス生成ロジック変更

- [ ] 3.1 `execute_shell_command_streaming()` 内の Unix 用 `pre_exec` を変更
  - `setsid()` の代わりに `setpgid(0, 0)` を使用
  - `/dev/tty` 操作ロジックは維持（TUI との分離のため）
- [ ] 3.2 `execute_shell_command_streaming()` 内の Windows 用ロジックに、spawn 直後の `assign_to_job()` 呼び出しを追加
- [ ] 3.3 `ProcessHandle` を `Child` とともに返すよう戻り値の型を変更
  - 新しい構造体 `ManagedChild` を定義: `{ child: Child, handle: Box<dyn ProcessHandle> }`
- [ ] 3.4 非ストリーミング版（`execute_shell_command`、`execute_shell_command_with_output`）も同様に変更

## 4. kill 呼び出しの変更

- [ ] 4.1 `src/orchestration/apply.rs` の `apply_change_streaming()` 内の `child.kill()` を `process_manager::terminate(&managed_child)` に変更
- [ ] 4.2 `src/orchestration/archive.rs` の `archive_change_streaming()` 内の `child.kill()` を同様に変更
- [ ] 4.3 `src/tui/orchestrator.rs` の `archive_single_change()` と `run_orchestrator()` 内の `child.kill()` を変更

## 5. run モードへのシグナルハンドリング追加

- [ ] 5.1 `src/main.rs` の `Commands::Run` ブランチ内で、`CancellationToken` を作成
- [ ] 5.2 `tokio::signal::ctrl_c()` を監視する非同期タスクを spawn
- [ ] 5.3 Unix 環境では `tokio::signal::unix::signal(SignalKind::terminate())` も監視
- [ ] 5.4 シグナル受信時に `cancel_token.cancel()` を呼び出し
- [ ] 5.5 `Orchestrator::run()` メソッドに `cancel_token` を引数として渡すよう変更
- [ ] 5.6 `Orchestrator::run()` 内のループで定期的に `cancel_token.is_cancelled()` をチェック
- [ ] 5.7 キャンセル検出時に現在処理中の子プロセスを terminate してから終了

## 6. TUI モードの終了待機時間調整

- [ ] 6.1 `src/tui/runner.rs` の終了処理部分（`cancel_token.cancel()` 後）のタイムアウトを 2秒 → 5秒に変更
- [ ] 6.2 `tokio::time::timeout()` で orchestrator_handle を await し、タイムアウト時に警告ログを出力
- [ ] 6.3 タイムアウト後も確実に `child.wait()` が呼ばれることを確認（プロセスゾンビ防止）

## 7. テストの追加

- [ ] 7.1 `tests/process_cleanup_test.rs` を作成し、以下のテストを実装:
  - Unix: 子プロセスとその子が確実に終了することを確認
  - Windows: ジョブオブジェクトによる自動終了を確認
  - run モード: SIGTERM 受信時にクリーンアップされることを確認
  - TUI モード: cancel_token による終了が正常に動作することを確認
- [ ] 7.2 各プラットフォームでテストが通ることを確認（`cargo test`）

## 8. ドキュメント更新

- [ ] 8.1 `AGENTS.md` に新しいプロセス管理の説明を追加
- [ ] 8.2 プロセスクリーンアップに関するトラブルシューティング情報を追加

## 9. 統合テストと検証

- [ ] 9.1 TUI モードで複数の変更を処理中に Esc → Esc（強制停止）を実行し、子プロセスが残らないことを確認
- [ ] 9.2 run モードで処理中に Ctrl+C を実行し、子プロセスが残らないことを確認
- [ ] 9.3 macOS、Linux、Windows の各環境で動作確認
- [ ] 9.4 長時間実行されるエージェントコマンドでテスト（例: `sleep 60`）
