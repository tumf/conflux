# Design: Fix Workspace Cleanup Guard

## 背景

`WorkspaceCleanupGuard`はRAIIパターンを使用して、エラー発生時にワークスペースが孤立しないようにする役割を持ちます。しかし、現在の実装には2つの問題があります：

1. 失敗したワークスペースを保護する`preserve()`メソッドが呼ばれていない
2. `Drop`実装がGitの正しいクリーンアップ順序に従っていない

## 根本原因分析

### 歴史的経緯

これらのバグは `preserve-workspace-on-error` 変更（コミット `73ded01`, 2026-01-12）の実装時に混入しました。

#### 問題1の原因: 実装漏れ

**設計意図**:
- `preserve()` メソッドを追加し、失敗したワークスペースを保護する
- エラー時に `cleanup_guard.preserve()` を呼び出す

**実装されたもの**:
1. ✅ `preserve()` メソッドの追加（ただし `#[allow(dead_code)]` 付き）
2. ✅ `Drop` 実装での `preserved_workspaces` チェック
3. ✅ 正常系クリーンアップループでの `failed_workspace_names` フィルタリング
4. ✅ エラーログに「workspace preserved」メッセージ追加

**欠落していたもの**:
- ❌ **`cleanup_guard.preserve()` の実際の呼び出し**

コミットを見ると、`mod.rs:1009-1032` でコメントとログは追加されましたが、肝心の `preserve()` 呼び出しが書かれていません：

```rust
// Also preserve workspaces for failed changes (do not cleanup)  // ← コメントは追加
for result in &failed {
    error!("Failed for {}, workspace preserved: {}", ...);      // ← ログも追加
    // ここで cleanup_guard.preserve() を呼ぶべきだった！← 呼び出しが欠落
}
```

**なぜ気づかなかったか**:
1. `#[allow(dead_code)]` により、コンパイラが「使われていない」警告を出さなかった
2. 正常系のクリーンアップでは `failed_workspace_names` でフィルタリングされるため、通常は問題にならない
3. 早期リターン時のみ発生するため、テストで検出されにくい

#### 問題2の原因: 設計不足とベストエフォート実装

**元々の実装** (`cleanup.rs` の Drop):
```rust
VcsBackend::Git | VcsBackend::Auto => {
    // For Git, we need the worktree path, but we only have the name
    // This is a best-effort cleanup; the worktree will be orphaned
    // but can be cleaned up later with `git worktree prune`
    //                    ↑↑↑ この時点で問題を認識していた
    let _ = Command::new("git")
        .args(["branch", "-D", workspace_name])  // ブランチ削除のみ
        ...
}
```

**設計上の制約**:
- `WorkspaceCleanupGuard` はワークスペース名のみを保持（パスを持たない）
- Drop 時にワークツリーのパスが不明
- 「best-effort cleanup」として、ブランチ削除のみを試み、ワークツリーは `git worktree prune` に任せる方針

**一方、`forget_workspace_sync` では**:
- ワークスペース情報（名前とパス）を持っている
- 正しい順序でクリーンアップできていた（ワークツリー → ブランチ）

**なぜこの設計になったか**:
- 当初は jj (Jujutsu) バックエンドもサポートしていた
- Git の場合、ワークツリーパスの取得が面倒なため「best-effort」として放置された
- 後に jj サポートが削除されたが（`08444e9 feat!: drop jj backend`）、この問題は残った

**なぜ気づかなかったか**:
1. ブランチ削除失敗のエラーは `debug!` レベルでログされるため、通常の実行では見えない
2. ワークツリーは残るが、機能的には大きな問題にならない
3. 「best-effort」として許容されていた

## 設計上の考慮事項

### 問題1の解決: `preserve()`の呼び出し

#### 現状の動作

`src/parallel/mod.rs:1009-1032`では：
- エラーログとイベントは発行される
- しかし`cleanup_guard.preserve()`は呼ばれない
- 正常系では`cleanup_guard.commit()`が呼ばれてDrop時のクリーンアップを抑制
- 早期リターン時（行996、1046）には`commit()`が呼ばれずDropが実行される

#### なぜ問題か

早期リターンやパニック時に`Drop`が実行されると、失敗したワークスペースも削除対象になります。これは「失敗したワークスペースは保持する」という仕様に反します。

#### 解決策

失敗したワークスペースを処理するループ内で`cleanup_guard.preserve()`を呼び出します：

```rust
for result in &failed {
    if result.error.is_some() {
        error!("Failed for {}, workspace preserved: {}", ...);
        cleanup_guard.preserve(&result.workspace_name); // 追加
    }
}
```

これにより、Drop時に`preserved_workspaces`がチェックされ、失敗したワークスペースはスキップされます。

### 問題2の解決: クリーンアップの順序

#### 現状の動作

`src/parallel/cleanup.rs:96-127`の`Drop`実装では：
- `git branch -D <workspace_name>`のみを実行
- ワークツリーの削除を試みない

#### なぜ問題か

Gitはワークツリーで使用中のブランチを削除できません：
```
error: cannot delete branch 'rename-to-conflux' used by worktree at '/path'
```

正しい順序は：
1. `git worktree remove <path> --force`
2. `git branch -D <branch_name>`

この順序は`src/vcs/git/mod.rs:949-1003`の`forget_workspace_sync`で既に実装されています。

#### 解決策の選択肢

**オプションA**: `WorkspaceCleanupGuard`に`base_dir`を追加し、パスを`base_dir/workspace_name`として導出
- 問題: `base_dir`は動的に生成されるtempディレクトリで、guarドからアクセスできない

**オプションB**: `track()`でパスも受け取り、`HashMap<String, PathBuf>`として保持
- **採用**: シンプルで確実

**オプションC**: Drop時に`git worktree list --porcelain`を実行してパスを取得
- 問題: Dropでの同期I/Oが増え、エラーハンドリングが複雑化

#### 採用した設計

`WorkspaceCleanupGuard`の構造を変更：

```rust
pub(crate) struct WorkspaceCleanupGuard {
    // 変更前: workspace_names: Vec<String>,
    // 変更後:
    workspaces: HashMap<String, PathBuf>,
    preserved_workspaces: std::collections::HashSet<String>,
    vcs_backend: VcsBackend,
    repo_root: PathBuf,
    committed: bool,
}
```

`track()`メソッドのシグネチャ変更：

```rust
// 変更前
pub fn track(&mut self, workspace_name: String)

// 変更後
pub fn track(&mut self, workspace_name: String, workspace_path: PathBuf)
```

`Drop`実装の修正（`forget_workspace_sync`と同様）：

```rust
for (workspace_name, workspace_path) in &workspaces_to_clean {
    // 1. ワークツリー削除
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", workspace_path.to_str().unwrap(), "--force"])
        .current_dir(&self.repo_root)
        .output();
    
    // 2. ブランチ削除
    let _ = std::process::Command::new("git")
        .args(["branch", "-D", workspace_name])
        .current_dir(&self.repo_root)
        .output();
}
```

## 影響を受けるコンポーネント

### 直接影響

- `src/parallel/cleanup.rs`: 構造体、メソッド、Drop実装の変更
- `src/parallel/mod.rs`: `track()`呼び出しと`preserve()`呼び出しの追加

### 間接影響

- なし（他のモジュールは`WorkspaceCleanupGuard`の内部実装に依存していない）

## 後方互換性

この変更は内部実装の修正であり、外部APIには影響しません。

## テスト戦略

1. **ユニットテスト**: `cleanup.rs`の既存テストを更新し、新しいシグネチャに対応
2. **E2Eテスト**: 並列実行でエラーが発生するシナリオをテストし、ワークスペースが正しく保持されることを確認
3. **ログ確認**: Gitエラーログが出力されないことを確認

## 代替案と却下理由

### 代替案1: `preserve()`を呼ばず、Dropロジックを変更

失敗したワークスペースをDrop時に識別する方法を追加する。

**却下理由**: `preserve()`メソッドが既に存在し、この目的のために設計されているため、新しいロジックを追加する必要がない。

### 代替案2: ワークツリー削除をスキップし、ブランチのみ削除

ワークツリーは残しておき、`git worktree prune`で後からクリーンアップする。

**却下理由**: 不完全なクリーンアップになり、ディスク容量を無駄に消費する。また、`forget_workspace_sync`との一貫性がない。

## リスクと緩和策

### リスク1: 既存のテストが壊れる

`track()`のシグネチャ変更により、既存のテストコードが壊れる可能性。

**緩和策**: 変更は小さく、コンパイラが全ての呼び出し箇所を検出するため、修正は機械的。

### リスク2: Dropでの同期I/Oがブロッキングする

ワークツリー削除がブロックする可能性。

**緩和策**: Drop時のクリーンアップはベストエフォート。エラーログは出すが、パニックしない。また、`forget_workspace_sync`と同じパターンを使用しているため、既に本番環境で実績がある。
