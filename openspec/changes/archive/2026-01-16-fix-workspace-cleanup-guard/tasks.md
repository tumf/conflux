# Tasks: fix-workspace-cleanup-guard

## 実装タスク

- [x] `src/parallel/cleanup.rs`の`WorkspaceCleanupGuard`構造体を変更（`workspace_names: Vec<String>`を`workspaces: HashMap<String, PathBuf>`に変更）
- [x] `WorkspaceCleanupGuard::track()`メソッドのシグネチャを変更してパスも受け取るように
- [x] `WorkspaceCleanupGuard::Drop`実装を修正（ワークツリー削除→ブランチ削除の順序に）
- [x] `src/parallel/mod.rs`の全ての`cleanup_guard.track()`呼び出しでパスも渡すように変更
- [x] `src/parallel/mod.rs:1011-1021`のループ内で`cleanup_guard.preserve(&result.workspace_name)`を追加
- [x] `src/parallel/cleanup.rs`のテストケースを更新（`track()`呼び出しにパスを追加）
- [x] `cargo fmt`でフォーマット
- [x] `cargo clippy`でリント確認
- [x] `cargo test`で既存テストの動作確認
