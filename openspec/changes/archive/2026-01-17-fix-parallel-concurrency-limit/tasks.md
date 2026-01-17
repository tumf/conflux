## 1. Implementation
- [x] 1.1 並列実行の同時数上限をworktree作成フェーズにも適用する
- [x] 1.2 execute_group内のworktree一括作成を、semaphore制御下で作成する方式に変更する
- [x] 1.3 TUI/CLIの並列実行フローで上限が反映されることを確認する
- [x] 1.4 既存の動的キュー/再分析の挙動が上限適用後も破綻しないことを確認する

## 2. Validation
- [x] 2.1 変更後に並列実行で同時worktree数が設定値を超えないことを検証する
- [x] 2.2 必要に応じて `cargo test` を実行する

## Implementation Summary

### Changes Made

1. **Refactored `execute_apply_and_archive_parallel()` signature** (src/parallel/mod.rs:1212-1228)
   - Changed from accepting `&[Workspace]` to `&[(String, Option<Workspace>)]`
   - Added `base_revision: &str` parameter for workspace creation
   - Added `cleanup_guard: &mut WorkspaceCleanupGuard` parameter

2. **Moved workspace creation under semaphore control** (src/parallel/mod.rs:1234-1330)
   - Semaphore permit acquired BEFORE workspace creation (line 1236)
   - Workspace creation/resume happens while holding permit
   - Permit moved into spawned task to control entire lifecycle

3. **Updated `execute_group()` to defer workspace creation** (src/parallel/mod.rs:747-862)
   - Removed upfront workspace creation loop
   - Changed to state detection + categorization approach
   - Only creates workspaces for archived state (immediate merge path)
   - Other changes passed as (change_id, None) for deferred creation

4. **Updated cleanup logic** (src/parallel/mod.rs:1133-1179)
   - Changed to get workspaces from workspace_manager
   - Handles both archived and apply-result workspaces

### Concurrency Control Flow

**Before:**
```
execute_group:
  for each change:
    create_workspace()  # All N workspaces created upfront

  execute_apply_and_archive_parallel:
    semaphore.acquire()  # Only controls apply+archive
    spawn(apply + archive)
```

**After:**
```
execute_group:
  for each change:
    detect_state()  # No workspace creation

  execute_apply_and_archive_parallel:
    for each change:
      semaphore.acquire()  # Controls everything
      create_workspace()   # Serial, under semaphore
      spawn(apply + archive)
```

### Benefits

1. **Strict concurrency limit**: At most `max_concurrent` workspaces exist
2. **Resource control**: Workspace creation rate controlled by semaphore
3. **Consistent behavior**: TUI and CLI respect the same limits
4. **No breaking changes**: All existing tests pass

### Testing

- All 755 unit tests pass
- All 25 E2E tests pass
- All 3 process cleanup tests pass
- No regressions in existing functionality
