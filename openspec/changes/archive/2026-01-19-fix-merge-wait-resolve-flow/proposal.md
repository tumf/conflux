# 変更提案: archive後にmerge/resolve遷移が欠落する問題の修正

## Why
並列実行のorder-basedループでarchive完了後にmerge/resolve処理が走らず、MergeWait遷移やresolve開始が起きないままworktreeが削除されるため、ユーザーが復旧操作を取れません。

## What Changes
- order-based実行でもarchive完了後に個別mergeを実行し、MergeDeferred時はMergeWaitとして残す
- merge/resolveイベントの送信とworktree保護を既存フローと整合させる
- 終了時のcleanupがMergeWait対象のworktreeを削除しないようにする

## Impact
- 対象spec: parallel-execution, workspace-cleanup
- 影響範囲: `src/parallel/mod.rs` のorder-based再分析ループ、merge/cleanup処理
