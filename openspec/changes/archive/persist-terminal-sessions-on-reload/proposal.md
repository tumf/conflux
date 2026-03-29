# Change: ブラウザリロード・worktree切り替え時のターミナルセッション維持

## Why

サーバーモードWebUIのシェルターミナルが、ブラウザリロードやworktree再選択（クリック）でリセットされる。バックエンドのPTYセッションはサーバープロセスが生存する限り維持されているが、フロントエンドがリロード後に既存セッションを復元しないため、ユーザーは作業中のシェルを失う。

## What Changes

- バックエンドの `TerminalSessionInfo` に `project_id` と `root` フィールドを追加し、セッション作成時に保存する
- バックエンドにPTY出力のスクロールバックバッファ（リングバッファ）を追加し、WebSocket接続時に過去出力を送信する
- フロントエンドの `TerminalPanel` がマウント時に既存セッションを取得し、現在のコンテキスト（project_id + root）に一致するセッションのタブを自動復元する
- Worktree切り替え時に既存セッションを破棄せず、新しいworktreeのセッションをフィルタ表示する（他worktreeのセッションはバックグラウンド維持）

## Impact

- Affected specs: `web-terminal`
- Affected code:
  - `src/server/terminal.rs` — `TerminalSessionInfo`, `TerminalSession`, `TerminalManager`
  - `src/server/api.rs` — `create_terminal` ハンドラー
  - `dashboard/src/components/TerminalPanel.tsx` — セッション復元ロジック
  - `dashboard/src/components/TerminalTab.tsx` — スクロールバック受信対応
  - `dashboard/src/api/restClient.ts` — 型定義更新
