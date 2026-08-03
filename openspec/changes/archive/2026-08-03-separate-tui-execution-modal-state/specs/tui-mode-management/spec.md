## MODIFIED Requirements

### Requirement: resolve-merge-mode-transition

Mキーによるresolve開始時、TUIの実行モードがアクティブな作業を反映するようRunningに遷移する。モーダル状態は実行モードとは独立して扱い、resolve開始可否は通常入力がモーダルに占有されていない場合にのみ評価する。

#### Scenario: resolve-merge-from-select-mode

**Given**: TUIの実行モードが `Select`（Ready表示）で、モーダルがなく、カーソル位置の変更が `QueueStatus::MergeWait` であり、resolveが未実行（`is_resolving == false`）
**When**: ユーザーがMキーを押して `resolve_merge()` が呼ばれる
**Then**: 実行モードが `Running` に遷移し、`TuiCommand::ResolveMerge` が返される

#### Scenario: resolve-merge-from-stopped-mode

**Given**: TUIの実行モードが `Stopped` で、モーダルがなく、カーソル位置の変更が `QueueStatus::MergeWait` であり、resolveが未実行
**When**: ユーザーがMキーを押して `resolve_merge()` が呼ばれる
**Then**: 実行モードが `Running` に遷移し、`TuiCommand::ResolveMerge` が返される

#### Scenario: resolve-merge-from-running-mode

**Given**: TUIの実行モードが `Running` で、モーダルがなく、カーソル位置の変更が `QueueStatus::MergeWait` であり、resolveが未実行
**When**: ユーザーがMキーを押して `resolve_merge()` が呼ばれる
**Then**: 実行モードは `Running` のまま変わらず、`TuiCommand::ResolveMerge` が返される

#### Scenario: modal-consumes-resolve-key

**Given**: TUIがいずれかのモーダル interaction を表示している
**When**: ユーザーがMキーを押す
**Then**: キー入力はモーダルに消費される
**And**: `TuiCommand::ResolveMerge` は返されず、実行モードも変更されない
