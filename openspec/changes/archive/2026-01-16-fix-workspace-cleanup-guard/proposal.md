# Proposal: Fix Workspace Cleanup Guard

## Why

並列実行モードでエラーが発生した際、失敗したワークスペースは保持されるべきですが、現在の実装では以下の問題により正しく動作していません：

1. `cleanup_guard.preserve()`が呼ばれていないため、早期リターン時に失敗したワークスペースが削除される
2. ワークツリー削除前にブランチ削除を試みるため、Gitエラーが発生する

この修正により、失敗したワークスペースが確実に保持され、デバッグとリジューム機能が正常に動作するようになります。

## 概要

`WorkspaceCleanupGuard`に2つの実装バグが存在します：

1. **失敗したワークスペースの保護が機能していない**: `mod.rs`でエラー時に`cleanup_guard.preserve()`が呼ばれておらず、失敗したワークスペースが誤ってクリーンアップされる可能性がある
2. **クリーンアップの順序が不正**: `cleanup.rs`の`Drop`実装でワークツリーを削除せずにブランチ削除を試みるため、Gitが「ブランチが使用中」エラーを返す

## 問題の詳細

### 問題1: `cleanup_guard.preserve()`が未呼び出し

`src/parallel/mod.rs:1009-1032`で失敗したワークスペースに対してログ出力とイベント送信は行われていますが、`cleanup_guard.preserve()`が呼ばれていません。

```rust
// 現在のコード
for result in &failed {
    if result.error.is_some() {
        error!(
            "Failed for {}, workspace preserved: {}",
            result.change_id, result.workspace_name
        );
        // ここで cleanup_guard.preserve() が呼ばれるべき
    }
}
```

この結果、早期リターンやパニック時に`Drop`が実行されると、失敗したワークスペースも削除対象になってしまいます。

### 問題2: ワークツリー削除前のブランチ削除

`src/parallel/cleanup.rs:96-127`の`Drop`実装では、`git branch -D`のみを実行していますが、ワークツリーがまだ存在する状態ではGitがブランチ削除を拒否します：

```
error: cannot delete branch 'rename-to-conflux' used by worktree at '/path/to/worktree'
```

正しい順序は：
1. `git worktree remove <path> --force`（ワークツリー削除）
2. `git branch -D <branch_name>`（ブランチ削除）

この順序は`src/vcs/git/mod.rs:949-1003`の`forget_workspace_sync`では正しく実装されています。

## 影響範囲

- 失敗したワークスペースが意図せず削除され、デバッグやリジューム機能が動作しない
- Gitエラーログが出力される（機能的には大きな影響なし、ただしログが汚れる）

## 提案する解決策

1. `mod.rs:1011-1021`のループ内で`cleanup_guard.preserve(&result.workspace_name)`を呼び出す
2. `cleanup.rs`の`WorkspaceCleanupGuard`構造体を変更し、ワークスペース名だけでなくパスも保持
3. `cleanup.rs`の`Drop`実装で、ブランチ削除前にワークツリー削除を実行

## What Changes

- `workspace-cleanup`: `WorkspaceCleanupGuard`のデータ構造とクリーンアップ順序を修正

## 関連仕様

- `workspace-cleanup`: ワークスペースクリーンアップの基本動作
- `parallel-execution`: エラー時のワークスペース保持要件（アーカイブされた仕様より）
