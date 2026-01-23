## 1. 実装
- [ ] 1.1 `ExecutionEvent`の`ApplyStarted`/`ArchiveStarted`/`ResolveStarted`に展開済みコマンド文字列を保持する`command`フィールドを追加する（検証: `src/events.rs`とイベント生成箇所でコンパイル可能な型に更新されている）
- [ ] 1.2 apply/archive/resolveの実行前にプレースホルダー展開済みコマンドをイベントに格納する（検証: `src/agent/runner.rs`と`src/parallel/mod.rs`で展開済み文字列が渡されている）
- [ ] 1.3 TUI Logs Viewにコマンドログを表示する（検証: `src/tui/state/events/stages.rs`で`ApplyStarted`/`ArchiveStarted`/`ResolveStarted`のハンドラがコマンドを出力する）
- [ ] 1.4 Web/状態管理のイベント処理を更新して新フィールドを扱う（検証: `src/web/state.rs`と`src/orchestration/state.rs`のイベントマッチが更新される）
- [ ] 1.5 イベント型変更に伴うテストを更新し、`cargo test`が成功することを確認する（検証: `cargo test`）
