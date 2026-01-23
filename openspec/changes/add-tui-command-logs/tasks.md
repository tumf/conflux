## 1. 実装
- [x] 1.1 `ExecutionEvent`の`ApplyStarted`/`ArchiveStarted`/`ResolveStarted`に展開済みコマンド文字列を保持する`command`フィールドを追加する（検証: `src/events.rs`とイベント生成箇所でコンパイル可能な型に更新されている）
- [x] 1.2 apply/archive/resolveの実行前にプレースホルダー展開済みコマンドをイベントに格納する（検証: `src/agent/runner.rs`と`src/parallel/mod.rs`で展開済み文字列が渡されている）
- [x] 1.3 TUI Logs Viewにコマンドログを表示する（検証: `src/tui/state/events/stages.rs`で`ApplyStarted`/`ArchiveStarted`/`ResolveStarted`のハンドラがコマンドを出力する）
- [x] 1.4 Web/状態管理のイベント処理を更新して新フィールドを扱う（検証: `src/web/state.rs`と`src/orchestration/state.rs`のイベントマッチが更新される）
- [x] 1.5 イベント型変更に伴うテストを更新し、`cargo test`が成功することを確認する（検証: `cargo test`）

## Acceptance #1 Failure Follow-up
- [x] 2.1. `src/tui/orchestrator.rs`: `run_orchestrator()` で `SerialRunService::process_change()` 呼び出し前にapply用の展開済みコマンドを生成し、`ApplyStarted` イベントで送信する（検証: `ApplyStarted` イベントに展開済みコマンド文字列が含まれる）
- [x] 2.2. `src/tui/orchestrator.rs`: `archive_single_change()` で `agent.run_archive_streaming_with_runner()` 呼び出し前にarchive用の展開済みコマンドを生成し、`ArchiveStarted` イベントで送信する（検証: `ArchiveStarted` イベントに展開済みコマンド文字列が含まれる）
- [x] 2.3. `src/tui/runner.rs`: `TuiCommand::ResolveMerge` 処理では `resolve_deferred_merge()` が内部で `ResolveStarted` を送信するため、プレースホルダー送信を削除する（検証: `resolve_merges_with_retry()` が展開済みコマンドを送信）
- [x] 2.4. `src/parallel/conflict.rs`: `resolve_conflicts_with_retry()` と `resolve_merges_with_retry()` で `ResolveStarted` に送信する `initial_command` に `{conflict_files}` プレースホルダーを `expand_conflict_files` で展開する（検証: `initial_command` に `{conflict_files}` プレースホルダーが残らない）
- [x] 2.5. 変更をコミットし、git working treeがcleanになることを確認する（検証: `git status --short` の出力が空）
