## Context
merge 完了後にフックが存在せず、現状は PostArchive を代用するしかない。parallel モードでは merge が別フェーズで実行され、TUI Worktree の手動マージも独立しているため、merge 完了後の明確なフックが必要である。

## Goals / Non-Goals
- Goals:
  - change が base branch にマージされた直後に on_merged を実行する。
  - parallel 自動マージと TUI Worktree 手動マージの両方で同一の on_merged を提供する。
  - 既存の hook の順序と意味を維持する。
- Non-Goals:
  - マージ戦略やマージ条件の変更。
  - 失敗したマージに対する新しい再試行戦略の追加。

## Decisions
- `HookType::OnMerged` と `hooks.on_merged` を追加し、既存の HookRunner と同じ設定形式で扱う。
- 発火タイミングは以下で統一する:
  - parallel: `MergeCompleted` の完了後、workspace cleanup 前。
  - TUI Worktree: `BranchMergeCompleted` の完了後。
  - serial(run): base branch が更新済みであることを確認できるタイミング（archive 成功直後）を merge 完了とみなす。
- HookContext は既存の change フックと同じプレースホルダーを提供する。apply_count が取得できない経路では 0 を設定する。
- change_id の解決ができない merge (手動作成の worktree など) では on_merged を実行せず、警告ログを残す。

## Risks / Trade-offs
- merge 完了イベントの多重発火による on_merged の二重実行リスクがあるため、イベント発火地点で 1 回だけ呼ぶ設計が必要。
- TUI のブランチ名から change_id を解決できない場合に on_merged が実行されないが、誤った change_id で実行するより安全。

## Migration Plan
- 既存の hooks 設定はそのまま動作する。on_merged は追加のオプションとして扱う。

## Open Questions
- なし
