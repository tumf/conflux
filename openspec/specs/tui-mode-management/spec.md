## Requirements

### Requirement: resolve-merge-mode-transition

Mキーによるresolve開始時、TUIのAppModeがアクティブな作業を反映するようRunningに遷移する。

#### Scenario: resolve-merge-from-select-mode

**Given**: TUIが `AppMode::Select`（Ready表示）で、カーソル位置の変更が `QueueStatus::MergeWait` であり、resolveが未実行（`is_resolving == false`）
**When**: ユーザーがMキーを押して `resolve_merge()` が呼ばれる
**Then**: `app.mode` が `AppMode::Running` に遷移し、`TuiCommand::ResolveMerge` が返される

#### Scenario: resolve-merge-from-stopped-mode

**Given**: TUIが `AppMode::Stopped` で、カーソル位置の変更が `QueueStatus::MergeWait` であり、resolveが未実行
**When**: ユーザーがMキーを押して `resolve_merge()` が呼ばれる
**Then**: `app.mode` が `AppMode::Running` に遷移し、`TuiCommand::ResolveMerge` が返される

#### Scenario: resolve-merge-from-running-mode

**Given**: TUIが `AppMode::Running` で、カーソル位置の変更が `QueueStatus::MergeWait` であり、resolveが未実行
**When**: ユーザーがMキーを押して `resolve_merge()` が呼ばれる
**Then**: `app.mode` は `AppMode::Running` のまま変わらず、`TuiCommand::ResolveMerge` が返される
