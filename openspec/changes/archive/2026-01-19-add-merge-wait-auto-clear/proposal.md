# Change: MergeWait の自動解除と再キュー

## なぜ
- MergeWait の worktree を削除して作業をリセットしても、TUI が MergeWait を保持し続け、解決操作が失敗しやすい。
- MergeWait のまま残ると、解決不能な状態が長時間残り、再実行待ちに戻せない。

## 何を変えるか
- 5秒ポーリングの自動更新で MergeWait を自動解除する。
- 解除条件は「worktree が存在しない」または「worktree が存在するが base に ahead していない（merged 相当）」のどちらかとする。
- 解除後は QueueStatus を Queued に戻し、再キュー待ち状態にする。
- 解除された change では `M` キーによる merge resolve を提示しない。

## 影響範囲
- TUI の自動更新ロジック（MergeWait の判定と状態遷移）
- TUI のキー表示条件（MergeWait の解除後に M を非表示）
- 変更状態の再キュー挙動とログ出力
