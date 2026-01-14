# Change: Resolve に目標（完了条件）を定義し、収束までループする

## Why

現状の `resolve_command` は「コンフリクトが解消されたかどうか」だけを目安に成功扱いされることがあり、Git が `MERGE_HEAD` を保持したまま（`All conflicts fixed but you are still merging.`）の状態で処理が先へ進んでしまうことがあります。

Apply / Archive と同様に、Resolve も「何をもって完了とするか（目標）」を明確に定義し、その目標が満たされるまで（または最大リトライまで）同じループを回す必要があります。

## What Changes

- `resolve_command` の完了条件（目標）を仕様として明文化する
  - コンフリクトが無いことに加えて、Git マージが完了していること（`MERGE_HEAD` が存在しない 等）
  - 逐次マージ対象の各 `change_id` について、`Merge change: <change_id>` を含むマージコミットが作成されていること
- Resolve 成功判定を「コマンドの exit code」ではなく「目標の達成」で行う
- 目標未達の場合、`resolve_command` を再実行して収束させる（最大リトライまで）
- archive 後に `openspec/changes/{change_id}` が残存する場合の扱いを明確化する
  - `approved` だけが残っている場合は、ディレクトリごと削除して完了とする

## Impact

- Affected specs:
  - `parallel-execution`
- Affected code (implementation phaseで変更対象になりうる):
  - `src/parallel/conflict.rs`（resolve の成功判定とリトライ）
  - `src/vcs/git/commands.rs`（マージ進行中判定などの Git 状態確認）
  - `src/parallel/mod.rs`（マージ検証のエラーメッセージ/判定強化の可能性）

## Notes

- この変更は「Resolve も Apply/Archive と同様に、目標達成までループする」という振る舞いを仕様化し、実装をそれに揃えることを目的とします。
