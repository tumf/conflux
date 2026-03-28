# Change: ダッシュボードLOGSエリアにファイルビュータブを追加

## Why

サーバーモードのダッシュボードでChangeやWorktreeの中身を確認するにはエディタを別途開く必要がある。ダッシュボード内でプロジェクトのファイルをブラウズ・閲覧できるようにすることで、運用効率を向上させる。

## What Changes

- **バックエンドAPI（2本追加）**: 任意のworktreeパス配下のファイルツリー取得APIとファイル内容取得APIを `src/server/api.rs` に追加
- **フロントエンド状態**: ファイルビューアのコンテキスト（Change選択 or Worktree選択）をstoreで管理
- **右ペインタブ化**: デスクトップの右ペイン（現在Logsのみ）に「Logs / Files」タブ切り替えを追加
- **FileViewPanelコンポーネント**: ファイルツリー（左）+ ファイル内容（右）の分割表示コンポーネントを新規作成
- **Change選択時の自動展開**: Change選択時にプロジェクトルートから表示し、`openspec/changes/<change_id>/` ノードを自動展開、`proposal.md` を自動オープン
- **Worktree選択時**: Worktreeクリック時にそのworktreeルートから表示
- **モバイル対応**: モバイルタブバーに「Files」タブを追加

## Impact

- Affected specs: `server-api`, `server-mode-dashboard`
- Affected code: `src/server/api.rs`, `dashboard/src/App.tsx`, `dashboard/src/components/`, `dashboard/src/api/`, `dashboard/src/store/`
