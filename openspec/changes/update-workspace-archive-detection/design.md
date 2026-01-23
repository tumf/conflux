## Context
- 再起動時に archive 済みの worktree が acceptance から再開される
- worktree の HEAD ツリーでは `openspec/changes/<change_id>` が存在せず、archive 配下に移動済みである

## Goals / Non-Goals
- Goals: worktree の HEAD ツリー状態を用いて archived を判定し、archive 完了後から再開する
- Goals: `Archive: <change_id>` が HEAD 以外でも archived と判定できるようにする
- Non-Goals: archive コミット生成のルール変更や merge フローの変更

## Decisions
- Decision: archived 判定は `openspec/changes/<change_id>` が存在しないことと archive エントリの存在を必須条件にする
- Decision: `Archive: <change_id>` が履歴に存在すれば、HEAD 以外でも archived と判定できるようにする
- Alternatives considered: HEAD コミットの件名一致を必須にする（再起動時に判定できないため採用しない）

## Risks / Trade-offs
- archive エントリが存在するが commit 未完了の場合に誤判定するリスクがあるため、clean 状態や履歴確認を条件に含める

## Migration Plan
- 判定ヘルパーを更新し、workspace state 検出ロジックとテストを調整する

## Open Questions
- なし
