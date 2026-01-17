# 設計書

## 問題の根本原因

現在の実装では、Mキーの表示条件（UI側）とマージ実行条件（ロジック側）に以下のギャップがあります：

### UI側（render.rs）
```rust
if !wt.is_main && !wt.is_detached && !wt.has_merge_conflict() && !wt.branch.is_empty() {
    key_hints.push(("M", "merge"));
}
```

### ロジック側（state/mod.rs）
```rust
if self.view_mode != ViewMode::Worktrees { return None; }  // 警告なし
if self.worktrees.is_empty() || cursor >= len { return None; }  // 警告なし
if worktree.is_main { warning + return None; }
if worktree.is_detached { warning + return None; }
if worktree.has_merge_conflict() { warning + return None; }
if branch_name.is_empty() { warning + return None; }
```

**問題点**：
1. UI側は`view_mode`とカーソル範囲をチェックしていない（暗黙的に正しいと仮定）
2. ロジック側の最初の2つの条件は警告メッセージを設定しない → ユーザーに何が起きているか分からない
3. **最大の問題**: baseブランチとの差分をチェックしていない → マージ不要なworktreeでもMキーが表示される
4. **クリティカルバグ**: マージ実行先が間違っている（後述）

### マージ実行先の問題（runner.rs）

現在の実装:
```rust
// runner.rs:1110 - 間違い
merge_branch(&worktree_path, &merge_branch)
```

**問題**: worktree側でマージを実行しているが、目的は「worktreeのブランチをbaseにマージする」こと。
- working directory cleanチェックがworktree側で行われる → worktreeがdirtyだとエラー
- マージコミットがworktree側に作成される → 意図と逆

**正しい実装**:
```rust
// base（main worktree）側でマージを実行
merge_branch(&merge_repo_root, &merge_branch)
```

## 設計方針

### 1. マージ実行先の修正（最優先）

マージは**base（main worktree）側**で実行するように修正します。これにより:
- working directory cleanチェックがbase側で正しく行われる
- マージコミットがbase側に作成される
- worktree側のuncommitted changesはマージに影響しない

### 2. 表示条件とロジック条件の完全一致

UI側で表示する条件と、ロジック側で実行できる条件を完全に一致させます。

### 3. 失敗時の明確なフィードバック

すべての条件チェックで失敗した場合、ユーザーに分かりやすい警告メッセージを表示します。

### 4. パフォーマンス重視の並列実行

差分チェックは既存のconflict checkと同様に、`tokio::task::JoinSet`で並列実行します。

## データフロー

```
1. Worktrees Viewに切り替え (Tab key)
   ↓
2. load_worktrees_with_conflict_check() 実行
   ↓
3. git worktree list --porcelain で基本情報取得
   ↓
4. 各worktreeに対して並列実行:
   - conflict check: git merge --no-commit --no-ff
   - ahead check: git rev-list --count <base>..<branch>
   ↓
5. WorktreeInfo構築（has_commits_aheadフィールド含む）
   ↓
6. WorktreesRefreshed イベント送信
   ↓
7. UI描画時に条件チェック
   - has_commits_ahead含むすべての条件を満たす → Mキー表示
   ↓
8. Mキー押下
   ↓
9. request_merge_worktree_branch()で再度条件チェック
   - すべての条件を満たす → TuiCommand::MergeWorktreeBranch送信
   - 1つでも満たさない → 警告メッセージ設定 + None返却
   ↓
10. TuiCommand::MergeWorktreeBranch処理
    - merge_branch(&repo_root, &branch_name) を実行 ← base側で実行
    - 成功: BranchMergeCompleted イベント + worktreeリスト更新
    - 失敗: BranchMergeFailed イベント + エラー表示
```

## 実装詳細

### マージ実行先の修正

`src/tui/runner.rs` の `TuiCommand::MergeWorktreeBranch` ハンドラ:

```rust
// 修正前（間違い）
match crate::vcs::git::commands::merge_branch(&worktree_path, &merge_branch).await

// 修正後（正しい）
match crate::vcs::git::commands::merge_branch(&merge_repo_root, &merge_branch).await
```

`merge_repo_root` は既に `repo_root.clone()` で取得済み（1099行目）なので、変更は1行のみ。

### WorktreeInfo拡張

```rust
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub head: String,
    pub branch: String,
    pub is_detached: bool,
    pub is_main: bool,
    pub merge_conflict: Option<MergeConflictInfo>,
    pub has_commits_ahead: bool,  // NEW
}
```

### 差分チェック関数

`src/vcs/git/commands.rs` に追加：

```rust
pub async fn count_commits_ahead<P: AsRef<Path>>(
    cwd: P,
    base_branch: &str,
    worktree_branch: &str,
) -> VcsResult<usize> {
    let range = format!("{}..{}", base_branch, worktree_branch);
    let output = run_git(&["rev-list", "--count", &range], cwd).await?;
    let count = output.trim().parse::<usize>()
        .map_err(|e| VcsError::git_command(format!("Invalid count: {}", e)))?;
    Ok(count)
}
```

### 並列実行パターン

既存のconflict check実装と同じパターン：

```rust
let mut tasks = tokio::task::JoinSet::new();

for (idx, worktree) in worktrees.iter().enumerate() {
    if worktree.is_main || worktree.is_detached || worktree.branch.is_empty() {
        continue;
    }

    let wt_path = worktree.path.clone();
    let branch_name = worktree.branch.clone();
    let base = base_branch.clone();

    tasks.spawn(async move {
        // conflict check
        let conflict = check_merge_conflicts(&wt_path, &base).await;
        // ahead check
        let ahead_count = count_commits_ahead(&wt_path, &base, &branch_name).await;
        (idx, conflict, ahead_count)
    });
}

while let Some(result) = tasks.join_next().await {
    // 結果をWorktreeInfoに反映
}
```

## エラーハンドリング

差分チェックでエラーが発生した場合：
- `has_commits_ahead = false` とする（安全側に倒す）
- ログに警告を出力
- worktreeリスト全体の取得は失敗させない

理由：差分チェックの失敗は致命的ではなく、単にMキーが表示されないだけで済む。

## テスト戦略

### 単体テスト

- `count_commits_ahead()` の動作確認（差分あり/なし/エラーケース）
- `WorktreeInfo` の新フィールドを含むコンストラクタテスト

### 統合テスト

手動テストで以下のシナリオを確認：

1. baseと同じコミット → Mキー非表示
2. baseより先のコミットあり → Mキー表示
3. main worktree → Mキー非表示（既存動作維持）
4. detached HEAD → Mキー非表示（既存動作維持）
5. conflict検出時 → Mキー非表示（既存動作維持）
6. 条件を満たさない状態でMキー押下 → 適切な警告メッセージ表示
7. **worktree側にuncommitted changesがある状態でマージ → 成功**
8. **base側にuncommitted changesがある状態でマージ → 失敗（期待通り）**

## 後方互換性

- 既存のworktree操作には影響なし
- 新フィールド`has_commits_ahead`はデフォルトで`false`（安全側）
- 既存のテストはフィールド追加に合わせて更新が必要

## パフォーマンス影響

- worktreeロード時に並列でgitコマンド実行（conflict checkと同じパターン）
- worktree数がN個の場合、O(N)の並列実行（実時間はほぼ定数）
- 典型的なケース（4 worktrees）: < 1秒（conflict checkと合わせて）

## 代替案と却下理由

### 代替案1: UI側のみで差分チェック

**却下理由**: renderのたびにgitコマンドを実行するのは非効率。イベントループをブロックするリスクがある。

### 代替案2: 差分チェックなしでMキー常時表示

**却下理由**: ユーザーがマージ不要なworktreeでMキーを押してエラーになる UX が悪い。

### 代替案3: Mキー押下時に差分チェック

**却下理由**: レスポンスが遅くなる。事前にチェックして表示制御するほうがUX的に優れている。
