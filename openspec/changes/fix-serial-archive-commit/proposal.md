# Change: シリアルモードのアーカイブ後コミット漏れを修正

## Why
シリアルモードで archive 実行後に作業ツリーの変更がコミットされず、変更履歴が残らないため、期待される進捗管理ができません。

## What Changes
- シリアルモードのアーカイブ完了後に、未コミット変更がある場合のコミット作成を共通処理に追加する。
- Git backend で `Archive: {change_id}` のコミットが作成される挙動を明確化する。

## Impact
- Affected specs: `code-maintenance`
- Affected code: `src/execution/archive.rs`, `src/orchestrator.rs`
