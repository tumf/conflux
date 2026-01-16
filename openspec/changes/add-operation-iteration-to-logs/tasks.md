# Tasks: TUIログヘッダーにオペレーションタイプとイテレーション番号を追加

## 実装タスク

- [x] 1. **LogEntry構造体の拡張** (`src/events.rs`)
  - `operation: Option<String>` フィールドを追加
  - `iteration: Option<u32>` フィールドを追加
  - `with_operation()` ビルダーメソッドを実装
  - `with_iteration()` ビルダーメソッドを実装
  - 単体テストを追加（ビルダーメソッドの動作確認）

- [x] 2. **ログヘッダーレンダリングの更新** (`src/tui/render.rs`)
  - `render_logs()` 関数内のヘッダー表示ロジックを更新
  - `[change_id:operation:iteration]` 形式に対応
  - オペレーション/イテレーションがない場合の後方互換性を維持
  - 表示幅の計算を調整（より長いヘッダーに対応）

- [x] 3. **並列実行モードのログ生成を更新** (`src/parallel/executor.rs`)
  - `execute_apply_with_retry()` 内のログに `with_operation("apply")` と `with_iteration(iteration)` を追加
  - その他のログエントリー生成箇所も確認して必要に応じて更新

- [x] 4. **並列実行モードのarchive/resolveログを更新** (`src/parallel/mod.rs`)
  - archive操作のログに `with_operation("archive")` を追加
  - resolve操作のログに `with_operation("resolve")` を追加

- [x] 5. **シリアルモードのログ生成を更新** (`src/tui/orchestrator.rs`)
  - apply/archive操作のログに適切なoperationとiterationを設定
  - イテレーション番号が不明な場合は省略

- [x] 6. **テストの追加/更新**
  - `src/tui/render.rs` のレンダリングテストを追加/更新
  - `src/events.rs` のビルダーメソッドテストを追加
  - 既存のテストが正常に動作することを確認

## 検証基準

- [x] LogEntry構造体に新しいフィールドとビルダーメソッドが追加されている
- [x] ログヘッダーが `[change_id:operation:iteration]` 形式で表示される
- [x] オペレーション/イテレーションがない場合は従来の形式で表示される（後方互換性）
- [x] 並列実行時にapplyログにイテレーション番号が表示される
- [x] archive/resolveログに適切なオペレーションタイプが表示される
- [x] 既存のテストがすべてパスする
- [x] 新しいテストがすべてパスする
- [x] `cargo fmt` と `cargo clippy` が警告なく完了する

## Future work

- 動作確認
  - TUIモードで実行してログヘッダーの表示を確認
  - 並列実行モードで複数の変更を処理し、ヘッダーの区別を確認
  - イテレーション番号が正しく表示されることを確認
