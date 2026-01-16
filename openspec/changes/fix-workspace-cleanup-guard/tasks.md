# Tasks: fix-workspace-cleanup-guard

## 実装タスク

- [ ] `src/parallel/cleanup.rs`の`WorkspaceCleanupGuard`構造体を変更（`workspace_names: Vec<String>`を`workspaces: HashMap<String, PathBuf>`に変更）
- [ ] `WorkspaceCleanupGuard::track()`メソッドのシグネチャを変更してパスも受け取るように
- [ ] `WorkspaceCleanupGuard::Drop`実装を修正（ワークツリー削除→ブランチ削除の順序に）
- [ ] `src/parallel/mod.rs`の全ての`cleanup_guard.track()`呼び出しでパスも渡すように変更
- [ ] `src/parallel/mod.rs:1011-1021`のループ内で`cleanup_guard.preserve(&result.workspace_name)`を追加
- [ ] `src/parallel/cleanup.rs`のテストケースを更新（`track()`呼び出しにパスを追加）
- [ ] `cargo fmt`でフォーマット
- [ ] `cargo clippy`でリント確認
- [ ] `cargo test`で既存テストの動作確認
