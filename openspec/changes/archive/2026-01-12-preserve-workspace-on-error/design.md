# Design: エラー時のワークスペース保持

## Context

並列実行モードでは、各changeに対してworkspaceを作成してapplyループを実行する。処理完了後（成功・失敗問わず）、workspaceはクリーンアップされる。

現在の実装では、エラー発生時も含めて全てのworkspaceが削除されるため、途中の作業結果が失われる。これは `add-workspace-resume` 機能との連携も妨げている。

## Goals

- エラー発生時にworkspaceを保持して作業結果を保全
- ログにworkspace名を出力して、手動調査・復旧を可能に
- `add-workspace-resume` と連携して自動復旧を実現

## Non-Goals

- 成功時のworkspace保持（これは別の機能として検討）
- 古いworkspaceの自動削除（タイムアウトベース）

## Decisions

### 1. エラー判定基準

**決定**: 以下の場合にエラーと判定し、workspaceを保持する

- 最大イテレーション到達 (`Max iterations reached`)
- applyコマンドの失敗 (非ゼロ終了コード)
- archiveコマンドの失敗
- その他の `OrchestratorError` 発生

```rust
// In execute_apply_and_archive_parallel
match execute_apply_in_workspace(...).await {
    Ok(_) => {
        // Success: proceed to archive, then cleanup
    }
    Err(e) => {
        // Error: preserve workspace, log workspace name
        error!(
            "Failed for {}, workspace preserved: {}",
            change_id, workspace_name
        );
        // Do NOT call cleanup_workspace
    }
}
```

### 2. ログ出力形式

**決定**: ERRORレベルで以下の形式を出力

```
[ERROR] Failed for {change_id}, workspace preserved: {workspace_name}
```

追加で、INFOレベルで復旧方法のヒントを出力：

```
[INFO] To resume: run with the same change_id, workspace will be automatically detected
```

**理由**:
- ERRORレベルで目立つようにする
- workspace名を出力することで、手動調査が可能
- 復旧方法のヒントを提供

### 3. クリーンアップロジックの変更

**決定**: 成功したworkspaceのみクリーンアップ、失敗したworkspaceは保持

```rust
// After parallel execution
for result in results {
    if result.error.is_some() {
        // Preserve workspace - do not cleanup
        error!(
            "Workspace preserved for failed change {}: {}",
            result.change_id, result.workspace_name
        );
    } else {
        // Success - cleanup workspace
        workspace_manager.cleanup_workspace(&result.workspace_name).await?;
    }
}
```

### 4. `add-workspace-resume` との連携

**決定**: 保持されたworkspaceは、次回実行時に `find_existing_workspace` で自動検出される

```
┌─────────────────────────────────────────────────────────────┐
│                    Execution Flow                           │
└─────────────────────────────────────────────────────────────┘

Initial Run:
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Create          │────▶│ Execute         │────▶│ Max iterations  │
│ Workspace       │     │ Apply Loop      │     │ reached (ERROR) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                               ┌─────────────────┐
                                               │ Preserve        │
                                               │ Workspace       │
                                               │ (no cleanup)    │
                                               └─────────────────┘

Retry Run:
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ find_existing   │────▶│ Reuse existing  │────▶│ Continue        │
│ _workspace()    │     │ workspace       │     │ Apply Loop      │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │
        │ (workspace detected from previous failed run)
        ▼
```

### 5. 処理フロー詳細

```
┌─────────────────────────────────────────────────────────────┐
│               Parallel Execution Complete                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ For each workspace result    │
               └──────────────────────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Has error?                   │
               └──────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
       ┌─────────────┐                 ┌─────────────┐
       │    Yes      │                 │    No       │
       └─────────────┘                 └─────────────┘
              │                               │
              ▼                               ▼
   ┌──────────────────────┐       ┌──────────────────────┐
   │ Log error with       │       │ Cleanup workspace    │
   │ workspace name       │       │                      │
   │                      │       │                      │
   │ "Failed for X,       │       │ workspace_manager    │
   │  workspace preserved:│       │   .cleanup_workspace │
   │  ws-X-abc123"        │       │   (workspace_name)   │
   └──────────────────────┘       └──────────────────────┘
              │                               │
              ▼                               │
   ┌──────────────────────┐                   │
   │ DO NOT cleanup       │                   │
   │ (preserve workspace) │                   │
   └──────────────────────┘                   │
              │                               │
              └───────────────┬───────────────┘
                              │
                              ▼
               ┌──────────────────────────────┐
               │ Continue with next result    │
               └──────────────────────────────┘
```

## Risks / Trade-offs

### Risk 1: ディスク容量の圧迫

- **リスク**: 失敗したworkspaceが蓄積し、ディスク容量を圧迫する可能性
- **対策**: 
  - ユーザーに警告メッセージで通知
  - 将来的に古いworkspaceの自動削除機能を追加（別change）

### Risk 2: 同一change_idの複数workspace

- **リスク**: 複数回失敗すると、同一change_idに対して複数のworkspaceが存在する可能性
- **対策**: `add-workspace-resume` の設計で、最新のworkspaceを選択し古いものを削除する仕様になっている

## Dependencies

- `add-workspace-resume`: 保持されたworkspaceの自動検出・再利用機能
- `add-periodic-workspace-commits`: エラー時でも途中の進捗がコミットされている必要がある
