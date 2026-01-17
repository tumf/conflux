# Design

## 概要
archive 完了後の逆方向移動（archive → changes）が残存すると `MergeWait` で停止するため、archive/merge フローの検証を三層に分けて防止する。

## ガード設計
1. **Archive commit 作成前チェック**
   - `ensure_archive_commit` 実行時に `openspec/changes/<change_id>` が存在する場合は即座にエラー。
   - 逆方向の移動や手動復元を検知し、コミット作成を阻止する。

2. **Archive commit 完了判定の強化**
   - `is_archive_commit_complete` で作業ツリーのクリーンに加えて、`openspec/changes/<change_id>` が存在しないことを必須化する。
   - archive コミットが存在しても changes 側が残っている場合は未完了扱いにする。

3. **Merge 前検証**
   - `attempt_merge` の直前に `verify_archive_completion` を再実行。
   - 未アーカイブの場合は `MergeDeferred` を返し `MergeWait` に留める。

## 期待される効果
- archive 完了の誤判定を防止し、changes 復活を即時検知する。
- merge 実行の前段で未アーカイブを止め、`MergeWait` の原因を明確化する。

## リスク
- 既存の異常系フローでエラーが早期に発生するため、ログ出力が増える可能性がある。
- 既存の手動運用で changes 側を一時的に保持していた場合は、運用手順の見直しが必要。
