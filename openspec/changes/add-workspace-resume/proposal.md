# Change: Workspace レジューム機能

## Why

並列実行モードで作業中にプロセスが中断（クラッシュ、Ctrl+C、システム終了など）した場合、作成されたworkspaceがディスク上に残ることがある。現在の実装では、同じchange_idで再実行すると既存のworkspaceを削除して新規作成するため、途中まで進んでいたタスクの進捗が失われる。

ユーザーは中断した作業を効率的に再開したいが、現状ではゼロからやり直す必要がある。

## What Changes

- 既存workspaceの検出・再利用機能を追加
- workspace内のタスク進捗を確認して再利用可否を判断
- CLI/TUIで再利用オプションを提供
- jj/Git両バックエンドで対応

### 詳細

1. **Workspace検出**: `WorkspaceManager` traitに `find_existing_workspace(change_id)` メソッドを追加
2. **進捗確認**: 既存workspaceのtasks.mdを読み取り、進捗状態を確認
3. **再利用判断**: 進捗が0%でない、またはファイル変更がある場合は再利用を提案
4. **CLI/TUIオプション**: `--resume` フラグまたは自動検出で再利用を選択可能に

## Impact

- 影響するspec: `parallel-execution`
- 影響するコード:
  - `src/vcs/mod.rs` (WorkspaceManager trait)
  - `src/vcs/jj/mod.rs` (JjWorkspaceManager)
  - `src/vcs/git/mod.rs` (GitWorkspaceManager)
  - `src/parallel/mod.rs` (ParallelExecutor)
  - `src/cli.rs` (CLIオプション)
  - `src/tui/` (TUIイベント処理)
