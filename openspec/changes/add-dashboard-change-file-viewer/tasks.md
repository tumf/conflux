## 1. バックエンドAPI

- [ ] 1.1 `GET /api/v1/projects/:id/files/tree` エンドポイントを `src/server/api.rs` に追加。クエリパラメータ `root` で base worktree / 特定 worktree を切り替え。`.git`, `node_modules`, `.next`, `target`, `dist` を除外した再帰的ファイルツリーをJSON返却。パストラバーサル防止。
- [ ] 1.2 `GET /api/v1/projects/:id/files/content` エンドポイントを `src/server/api.rs` に追加。クエリパラメータ `root` + `path` でファイル内容を返す。1MB上限 + truncatedフラグ、バイナリ判定（NULバイト検出）、パストラバーサル防止。
- [ ] 1.3 `build_router` にルーティングを追加
- [ ] 1.4 バックエンドAPIのユニットテストを追加（正常系、パストラバーサル拒否、存在しないproject/file、サイズ上限、除外ディレクトリ、worktree rootの切り替え）

## 2. フロントエンド型定義・APIクライアント

- [ ] 2.1 `dashboard/src/api/types.ts` に `FileTreeEntry` 型と `FileBrowseContext` 型を追加
- [ ] 2.2 `dashboard/src/api/restClient.ts` に `fetchFileTree(projectId, root)` と `fetchFileContent(projectId, root, path)` を追加

## 3. フロントエンド状態管理

- [ ] 3.1 `useAppStore` に `fileBrowseContext: FileBrowseContext | null` と `setFileBrowseContext` アクションを追加
- [ ] 3.2 `ChangeRow` コンポーネントでチェックボックス以外の領域クリック時に `setFileBrowseContext({ type: 'change', changeId })` を呼び出す（選択中のChangeをハイライト表示）
- [ ] 3.3 `WorktreeRow` コンポーネントでアクションボタン以外の領域クリック時に `setFileBrowseContext({ type: 'worktree', worktreeBranch })` を呼び出す（選択中のWorktreeをハイライト表示）

## 4. FileViewPanelコンポーネント

- [ ] 4.1 `dashboard/src/components/FileViewPanel.tsx` を新規作成（ファイルツリー + ファイル内容表示の左右分割）
- [ ] 4.2 ファイルツリー（左ペイン ~200px）: ディレクトリ折りたたみ/展開、ファイル選択、lucide-reactアイコン（Folder, File, ChevronRight/ChevronDown）
- [ ] 4.3 ファイル内容（右ペイン）: font-mono text-xs、行番号付きプレーンテキスト表示、バイナリファイル時のプレースホルダー
- [ ] 4.4 コンテキスト未設定時・ファイル未選択時のプレースホルダー表示
- [ ] 4.5 Change選択時の自動展開ロジック: `openspec/changes/<change_id>/` までのツリーを自動展開し、`proposal.md` を自動オープン
- [ ] 4.6 Worktree選択時: worktreeルートからツリーを表示

## 5. App.tsx統合

- [ ] 5.1 デスクトップ右ペインにLogs/Filesタブ切り替えを追加（現在のLogsヘッダー部分をタブ化）
- [ ] 5.2 Filesタブ選択時に `FileViewPanel` を表示（`fileBrowseContext` をpropsとして渡す）
- [ ] 5.3 モバイルタブバーに「Files」タブを追加し、モバイルでも `FileViewPanel` を表示

## 6. ビルド・検証

- [ ] 6.1 `cd dashboard && npm run build` が成功することを確認
- [ ] 6.2 `cargo build` が成功することを確認
- [ ] 6.3 `cargo clippy -- -D warnings` が警告なしで通ることを確認
- [ ] 6.4 `cargo test` が全テスト通過することを確認
