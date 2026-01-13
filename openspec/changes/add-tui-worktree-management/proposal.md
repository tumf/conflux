# Change: TUIのworktree管理表示と削除操作を追加

## Why
並列実行や再開のためのworktreeが残っているかがTUI上で把握しづらく、不要なworktreeの手動削除も煩雑になっています。変更一覧での可視化と、選択中changeに紐づくworktree削除操作を提供して運用を簡略化します。

## What Changes
- Change一覧に「worktree存在」を示すインジケータを表示する
- 選択中changeのworktreeを削除する操作（Dキー + 確認）を追加する
- worktreeが存在しない場合の安全な挙動（メッセージ表示/無操作）を定義する

## Impact
- Affected specs: `cli`
- Affected code: `src/tui/**`, `src/vcs/**`, `src/parallel/**`（worktree検出/削除の補助）
