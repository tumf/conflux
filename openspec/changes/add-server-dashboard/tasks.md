## 1. Frontend プロジェクトの初期化

- [ ] 1.1 `dashboard/` ディレクトリを Vite + React 19 + TypeScript で初期化する（verification: `dashboard/package.json` に react@19, vite, typescript が含まれること）
- [ ] 1.2 shadcn/ui を初期化する（Tailwind v4 + Nova プリセット）（verification: `dashboard/components.json` が生成され `dashboard/src/index.css` に `@import "tailwindcss"` が含まれること）
- [ ] 1.3 Lucide React と sonner をインストールする（verification: `dashboard/package.json` の dependencies に `lucide-react`, `sonner` が含まれること）
- [ ] 1.4 ダーク基調のカラートークンを `dashboard/src/index.css` の `@theme` ブロックに定義する（verification: `--color-background` がダーク値で定義されていること）
- [ ] 1.5 `dashboard/vite.config.ts` で `outDir: "dist"` および `base: "/dashboard/"` を設定する（verification: `npm run build` で `dashboard/dist/index.html` が生成されること）

## 2. API クライアントの実装

- [ ] 2.1 `dashboard/src/api/types.ts` に `RemoteProject`, `RemoteChange`, `RemoteLogEntry`, `RemoteStateUpdate` の型を定義する（verification: `src/remote/types.rs` の各フィールドと対応していること）
- [ ] 2.2 `dashboard/src/api/restClient.ts` に `fetchProjectsState`, `controlRun`, `controlStop`, `gitSync`, `deleteProject` を実装する（verification: 各関数が対応するエンドポイント URL を呼び出すこと）
- [ ] 2.3 `dashboard/src/api/wsClient.ts` に WebSocket クライアントを実装する。接続先は `/api/v1/ws`、exponential backoff 再接続（1s→2s→4s→max 30s）と `connected|reconnecting|disconnected` の接続状態管理を含む（verification: 接続・切断・再接続のサイクルが動作すること）

## 3. 状態管理の実装

- [ ] 3.1 `dashboard/src/store/useAppStore.ts` に `useReducer` ベースの状態管理を実装する。state は `projects`, `selectedProjectId`, `logsByProjectId`, `connectionStatus`、actions は `SET_FULL_STATE`, `APPEND_LOG`, `SET_CONNECTION_STATUS`, `SELECT_PROJECT`（verification: `SET_FULL_STATE` dispatch で `projects` が置き換わることを Vitest テストで確認）
- [ ] 3.2 `dashboard/src/hooks/useWebSocket.ts` を実装し、ws クライアントと store を接続する（verification: `FullState` 受信時に `projects` が更新されること）

## 4. コンポーネントの実装

- [ ] 4.1 `dashboard/src/components/Header.tsx` を実装する。Conflux Server タイトルと接続状態インジケータ（色付きドット + Connected/Reconnecting/Disconnected テキスト）を含む（verification: `connectionStatus` の値が表示に反映されること）
- [ ] 4.2 `dashboard/src/components/ProjectCard.tsx` を実装する。`repo@branch` 形式の名前表示、`idle/running/stopped` バッジ（shadcn `Badge`）、Run/Stop/GitSync/Delete の 4 ボタン（Lucide アイコン）、選択状態ハイライトを含む（verification: 4 ボタンに aria-label が付与されていること）
- [ ] 4.3 `dashboard/src/components/ProjectsPanel.tsx` を実装する。`ProjectCard` リストとクリック時の `SELECT_PROJECT` dispatch を含む（verification: プロジェクトが列挙されること）
- [ ] 4.4 `dashboard/src/components/ChangeRow.tsx` を実装する。変更 ID、shadcn `Progress` バー（`completed_tasks/total_tasks`）、`status:iteration` 形式ステータス表示を含む（verification: status=applying・iteration_number=2 のとき `applying:2` と表示されること）
- [ ] 4.5 `dashboard/src/components/ChangesPanel.tsx` を実装する。選択プロジェクトの変更リストと空状態表示を含む（verification: 選択変更時に一覧が切り替わること）
- [ ] 4.6 `dashboard/src/components/LogEntry.tsx` を実装する。タイムスタンプ・レベル・メッセージ表示、レベル別色分け（info=通常、warn=黄、error=赤）を含む（verification: 3 レベルで異なる色が適用されること）
- [ ] 4.7 `dashboard/src/components/LogsPanel.tsx` を実装する。選択プロジェクトのログ一覧（最新 500 件）、末尾オートスクロール、shadcn `ScrollArea` を含む（verification: 新規ログ追加時に最下部にスクロールすること）
- [ ] 4.8 Delete 確認ダイアログを shadcn `AlertDialog` で実装する（verification: ✕ ボタン押下時に確認ダイアログが表示されること）

## 5. 操作フローの実装

- [ ] 5.1 Run / Stop / Git Sync ボタンに REST API 呼び出し・toast 通知・実行中 disabled 制御を実装する（verification: API 呼び出し完了まで対象ボタンが disabled になること）
- [ ] 5.2 Delete フローを実装する。AlertDialog 確認後に `deleteProject(id)` を呼び出し、成功 toast を表示する（verification: AlertDialog「確認」後に DELETE API が呼ばれること）

## 6. レスポンシブ・アクセシビリティ

- [ ] 6.1 モバイル表示（<768px）でタブ切り替え（Projects / Changes / Logs）を実装する（verification: viewport 375px でタブが表示されること）
- [ ] 6.2 全インタラクティブ要素に `aria-label` を付与する（verification: Run/Stop/Sync/Delete ボタンにそれぞれ aria-label があること）
- [ ] 6.3 コントラスト比 4.5:1 以上を確保する（verification: ブラウザアクセシビリティ検査でコントラスト警告がないこと）

## 7. Rust 側への組み込み

- [ ] 7.1 `dashboard/build.sh` を作成し `npm install && npm run build` を実行するスクリプトを追加する（verification: `bash dashboard/build.sh` で `dashboard/dist/index.html` が生成されること）
- [ ] 7.2 `src/server/api.rs` または `src/server/mod.rs` に `GET /dashboard` と `GET /dashboard/assets/{filename}` のルートを追加し、ビルド済みファイルを `include_str!` / `include_bytes!` で埋め込んで配信する（verification: `cargo build` が通り `GET /dashboard` で 200 が返ること）
- [ ] 7.3 `AGENTS.md` の Commands セクションに `cd dashboard && npm run build` の手順を追記する（verification: `AGENTS.md` に dashboard build 手順が記載されていること）

## 8. 結合テスト

- [ ] 8.1 `cflx server` 起動後にプロジェクトを追加し、`http://localhost:39876/dashboard` でダッシュボードが表示されることを確認する（verification: ブラウザで UI が表示され WebSocket 接続が確立されること）
- [ ] 8.2 Run / Stop / Git Sync / Delete の各操作が UI から実行でき、状態が反映されることを確認する（verification: 各操作後に toast が表示され次の FullState でステータスが更新されること）
