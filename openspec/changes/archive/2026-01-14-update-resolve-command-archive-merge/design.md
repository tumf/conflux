## Context
並列実行では archive 完了直後に個別マージが行われるが、archive のコミット失敗や merge で使用する change_id の誤りがあると、再開時に誤って archived 扱いになり cleanup が走る。

## Scope
- 既存の merge 委譲（`resolve_command` 実行）は完了済みのため本変更の対象外とする。
- 本変更は archive コミットの委譲、merge の change_id 正規化、resume 判定の強化に限定する。

## Approach
- archive フェーズのコミット作成を `resolve_command` に委譲し、pre-commit による中断があっても再ステージ・再コミットで収束させる。
- merge フェーズの `change_id` は `openspec/changes/{change_id}` を正とし、worktree ブランチ名は merge 対象の識別子として扱う。
- resume 時の archived 判定は、ファイル移動だけでなく archive コミットの存在を条件に含める。

## Archive Commit Delegation
- archive 完了後に `resolve_command` を worktree ルートで実行し、次の成功条件を満たすことを要求する:
  - `git status --porcelain` が空
  - 直近コミットの subject が `Archive: <change_id>`
- pre-commit がコミットを中断した場合は、`git add -A` の再実行と同一メッセージでの再コミットを行う。

## Merge change_id Normalization
- merge 対象は worktree ブランチ名（`ws-...`）とし、`change_id` は OpenSpec の変更IDを別途渡す。
- `Merge change: <change_id>` の検証対象は OpenSpec の `change_id` とし、SHA やサニタイズ済み ID を混在させない。

## Resume Behavior
- resume で `archive` をスキップするのは、`Archive: <change_id>` コミットが確認できた場合に限定する。
- 未コミットの archive が残っている場合は、`resolve_command` による archive コミット完了を再実行する。

## Trade-offs
- `resolve_command` に archive コミットを委譲することで LLM の責務は増えるが、pre-commit 由来の中断を確実に収束させられる。
- 追加の検証により resume 時の分岐が増えるが、マージ前の cleanup を防止できる。
