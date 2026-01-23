## Context
- 再起動時に archive 済みの worktree が acceptance から再開される
- worktree の HEAD ツリーでは `openspec/changes/<change_id>` が存在せず、archive 配下に移動済みである
- 現行の判定はコミットメッセージに依存しており、再起動時に安定しない

## Goals / Non-Goals
- Goals: コミットメッセージではなく、コミットされたファイル状態のみで archived を判定する
- Goals: worktree の HEAD ツリー状態を用いて archived を判定し、archive 完了後から再開する
- Non-Goals: archive コミット生成のルール変更や merge フローの変更

## Decisions
- Decision: すべての状態判定はコミットメッセージを使用せず、ファイル状態のみで行う
- Decision: archived 判定条件は (1) worktree が clean、(2) `openspec/changes/<change_id>` が存在しない、(3) archive エントリが存在する の3点
- Decision: archiving 判定条件は (1) worktree が dirty、(2) `openspec/changes/<change_id>` が存在しない、(3) archive エントリが存在する の3点
- Alternatives considered: コミットメッセージ一致を必須にする（再起動時に安定しないため不採用）

## Risks / Trade-offs
- dirty + archive エントリなし の場合は archiving ではなく applying/created と判定される（正しい動作）
- archive エントリが手動で削除された場合は archived と判定されない（再実行が必要）

## Migration Plan
- 判定ヘルパーを更新し、workspace state 検出ロジックとテストを調整する

## Open Questions
- なし
