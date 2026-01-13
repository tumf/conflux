# Design: 定期的なワークスペースコミット

## Context

並列実行モードでは、各changeに対してworkspaceを作成し、タスクが100%完了するまでapplyコマンドを繰り返し実行する。最大イテレーション数（デフォルト50回）に達した場合、エラーとして処理される。

現在の実装では、applyループ完了後にのみコミットを作成している。これにより、最大イテレーション到達時に途中の作業結果が失われる。

## Goals

- 各イテレーション終了時に進捗をコミットして永続化
- エラー発生時でも途中の作業結果を保持
- `add-workspace-resume` 機能との連携を可能に

## Non-Goals

- コミット履歴の圧縮（squash）機能
- コミットメッセージのカスタマイズ

## Decisions

### 1. コミットタイミング

**決定**: 各applyイテレーション終了後、タスク進捗チェック後にコミットを作成

```rust
// After apply command and progress check
if new_progress.completed > progress.completed {
    // Progress was made, commit the changes
    create_progress_commit(workspace_path, change_id, new_progress, vcs_backend).await?;
}
```

**理由**:
- 進捗があった場合のみコミットすることで、無駄なコミットを削減
- タスク進捗情報をコミットメッセージに含めることで、状態を追跡可能

### 2. コミットメッセージ形式

**決定**: `WIP: {change_id} ({completed}/{total} tasks)` 形式

```
WIP: add-web-monitoring (50/70 tasks)
```

**理由**:
- WIPプレフィックスで作業中であることを明示
- タスク進捗を数値で示すことで、状態が一目で分かる

### 3. jjとGitの処理の違い

**決定**:
- jj: `jj describe` でコミットメッセージを更新（working copyの変更は自動的にスナップショットされる）
- Git: `git add -A && git commit --amend` で変更をコミット

```rust
match vcs_backend {
    VcsBackend::Jj => {
        // jj automatically snapshots working copy changes
        // Just update the commit message
        Command::new("jj")
            .args(["describe", "--ignore-working-copy", "-m", &commit_message])
            .current_dir(workspace_path)
            .output()
            .await?;
    }
    VcsBackend::Git => {
        // Stage and amend the commit
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(workspace_path)
            .output()
            .await?;
        Command::new("git")
            .args(["commit", "--amend", "-m", &commit_message])
            .current_dir(workspace_path)
            .output()
            .await?;
    }
}
```

**理由**:
- jjは自動スナップショット機能があるため、describeのみで十分
- Gitは明示的にadd/commitが必要

### 4. 処理フロー

```
┌─────────────────────────────────────────────────────────────┐
│                    Apply Loop Iteration                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │ Check task progress  │
                   │ (before apply)       │
                   └──────────────────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │ Execute apply cmd    │
                   └──────────────────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │ Check task progress  │
                   │ (after apply)        │
                   └──────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Progress made?               │
               └──────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
       ┌─────────────┐                 ┌─────────────┐
       │    Yes      │                 │    No       │
       └─────────────┘                 └─────────────┘
              │                               │
              ▼                               │
   ┌──────────────────────┐                   │
   │ Create progress      │                   │
   │ commit with message: │                   │
   │ "WIP: {id} (X/Y)"    │                   │
   └──────────────────────┘                   │
              │                               │
              └───────────────┬───────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Tasks complete?              │
               └──────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
       ┌─────────────┐                 ┌─────────────┐
       │    Yes      │                 │    No       │
       │ (exit loop) │                 │ (continue)  │
       └─────────────┘                 └─────────────┘
```

## Risks / Trade-offs

### Risk 1: コミット数の増加

- **リスク**: イテレーションごとにコミットすると、履歴が膨大になる可能性
- **対策**: jjでは自動スクワッシュを活用、Gitではamendを使用

### Risk 2: パフォーマンスへの影響

- **リスク**: 毎イテレーションでのコミット処理がオーバーヘッドになる可能性
- **対策**: コミット処理は軽量（メッセージ更新のみ）なので影響は最小限
