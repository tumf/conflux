# Change: TUIの `+` を worktree_command 実行に置き換える

## Why

TUIの `+` キーがプロポーザル本文の入力（Proposing）に紐づいており、Git worktree を用いた隔離された作業環境での提案作成フローと整合しません。

提案作成の起点を「一時ディレクトリに Git worktree を作成し、そのworktree内で設定コマンドを実行する」方式に統一して、提案作成の環境依存や手動手順を減らします。

## What Changes

- TUIのSelectモードで `+` を押した際の挙動を、提案入力モード起動から「worktree作成→`worktree_command` 実行」に変更する
- `worktree_command` を設定ファイルで定義できるようにする（プレースホルダー展開あり）
- Gitリポジトリ上でない、または `worktree_command` が未定義の場合、`+` は無操作とする（警告も表示しない）
- 作成した worktree は削除せず残す

## Impact

- Affected specs: `tui-propose-input`, `configuration`
- Affected code (planned): `src/tui/**`, `src/config/**`, `src/vcs/git/**`
- **BREAKING (behavioral)**: `+` は `propose_command` による提案入力モードを起動しなくなる
