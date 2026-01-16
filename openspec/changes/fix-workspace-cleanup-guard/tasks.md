# Tasks

## 1. `WorkspaceCleanupGuard`のデータ構造を変更

`src/parallel/cleanup.rs`の`WorkspaceCleanupGuard`構造体を変更：
- `workspace_names: Vec<String>`を`workspaces: HashMap<String, PathBuf>`に変更
- ワークスペース名とパスの両方を保持

**検証**: コンパイルが通ることを確認

## 2. `WorkspaceCleanupGuard::track()`メソッドのシグネチャ変更

`track()`メソッドをパスも受け取るように変更：
```rust
pub fn track(&mut self, workspace_name: String, workspace_path: PathBuf)
```

**検証**: コンパイルが通ることを確認

## 3. `WorkspaceCleanupGuard::Drop`実装の修正

`Drop`実装で正しい順序でクリーンアップを実行：
1. `git worktree remove <path> --force`を先に実行
2. その後`git branch -D <branch_name>`を実行

`src/vcs/git/mod.rs:949-1003`の`forget_workspace_sync`を参考に実装。

**検証**: 既存のユニットテストが通ることを確認

## 4. `mod.rs`の`cleanup_guard.track()`呼び出しを更新

`src/parallel/mod.rs`の全ての`cleanup_guard.track()`呼び出しでパスも渡すように変更：
```rust
cleanup_guard.track(ws.name.clone(), ws.path.clone());
```

**検証**: コンパイルが通ることを確認

## 5. 失敗したワークスペースに`preserve()`を呼び出す

`src/parallel/mod.rs:1011-1021`のループ内で`cleanup_guard.preserve(&result.workspace_name)`を追加。

**検証**: 
- 失敗したワークスペースが`Drop`時にクリーンアップされないことを確認
- ログに「workspace preserved」が出力されることを確認

## 6. ユニットテストの更新

`src/parallel/cleanup.rs`のテストケースを更新：
- `track()`呼び出しにパスを追加
- Drop時のワークツリー削除動作を検証するテストを追加（必要に応じて）

**検証**: `cargo test`が全て通ることを確認

## 7. E2Eテストの実行

並列実行モードでエラーが発生するシナリオをテスト：
- 失敗したワークスペースが保持されること
- クリーンアップ時にGitエラーが発生しないこと

**検証**: 
- `RUST_LOG=debug cargo run -- run`でログを確認
- 失敗したワークスペースのディレクトリとブランチが存在することを確認

## 8. コードフォーマットとリント

修正完了後：
```bash
cargo fmt
cargo clippy -- -D warnings
```

**検証**: 警告・エラーがないことを確認
