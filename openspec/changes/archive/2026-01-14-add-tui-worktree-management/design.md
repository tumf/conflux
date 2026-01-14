# Design: TUI worktree管理表示と削除操作

## 目的
- 変更一覧からworktreeの有無を確認できるようにする
- 不要なworktreeをTUIから削除できるようにする

## 前提
- worktreeはGit backendで作成され、`ws-<change-id>-<suffix>` 形式のブランチ名で保持される
- 既存のresume挙動で `find_existing_workspace` が利用されている

## 方式
### worktree存在の判定
- Change一覧を描画するタイミングで `WorkspaceManager` を通じて worktree の存在を問い合わせる
- Gitの場合は `git worktree list --porcelain` の結果をキャッシュし、change_idごとに存在フラグを生成する
- TUI側は change row に「worktreeあり」インジケータ（例: `WT`）を表示する

### 削除操作
- Selectモードで `D` キーを押すと確認ダイアログを表示する
- 確認後、対象changeのworktreeが存在すれば削除を実行し、成功メッセージを表示する
- worktreeが存在しない場合は無操作で「存在しない」旨を通知する

### 安全性
- Running/Processing中のchangeでは削除操作を無効化し、明示的にエラーメッセージを出す
- 削除対象は「選択中changeに紐づくworktree」のみに限定し、他のworktreeは触らない

## 影響範囲
- `tui` のイベント/状態管理: worktree存在フラグ、確認ダイアログ、通知
- `vcs` のworkspace管理: change_id -> worktree存在/削除の補助API

## 代替案
- CLIコマンドとして削除を提供する案（TUI操作を簡略化するが一覧性が下がるため採用しない）
