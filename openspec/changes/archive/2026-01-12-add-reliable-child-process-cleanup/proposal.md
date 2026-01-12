# Change: 子プロセスの確実なクリーンアップ

## Why

現在、orchestrator が起動する子プロセス（agent コマンド）は以下の問題を抱えています：

1. **TUI キャンセル時**: `child.kill()` を呼ぶが、孫プロセスが残る可能性がある
2. **`run` モードの終了時**: シグナルハンドリングがなく、SIGINT/SIGTERM で終了した場合に子プロセスが残る
3. **プロセスグループ管理の不在**: Unix では `setsid()` でセッション分離しているが、グループ全体を kill する仕組みがない
4. **Windows サポートの不備**: Windows ではジョブオブジェクトによる管理が標準だが、実装されていない

アプリケーション停止時にエージェントプロセスが残ると、リソースリークや意図しない副作用が発生します。

## What Changes

- Unix 系（macOS/Linux）では **プロセスグループ** を使用し、終了時にグループ全体を kill
- Windows では **ジョブオブジェクト** を使用し、親プロセス終了時に子プロセスも自動終了
- `run` モードに **シグナルハンドラ（SIGINT/SIGTERM）** を追加し、受信時に子プロセスをクリーンアップしてから終了
- TUI モードの終了待機時間を延長し、確実にクリーンアップが完了するまで待機
- 子プロセスの追跡機構を追加し、終了時に残存プロセスがないことを確認

## Impact

- Affected specs: `cli`
- Affected code:
  - `src/agent.rs` - プロセス生成ロジックの変更（プロセスグループ/ジョブオブジェクト）
  - `src/orchestration/apply.rs`, `src/orchestration/archive.rs` - kill 呼び出しの変更
  - `src/tui/orchestrator.rs` - kill 呼び出しの変更
  - `src/tui/runner.rs` - 終了時のクリーンアップ待機時間調整
  - `src/main.rs` - run モードへのシグナルハンドラ追加
- Breaking: なし（内部実装の変更のみ）
