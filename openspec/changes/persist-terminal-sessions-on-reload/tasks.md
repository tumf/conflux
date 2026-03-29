## Implementation Tasks

### Backend

- [ ] 1.1 `TerminalSessionInfo` に `project_id: String` と `root: String` フィールドを追加する (`src/server/terminal.rs:28-34`)
- [ ] 1.2 `CreateTerminalRequest` に `project_id: Option<String>` と `root: Option<String>` を追加し、`create_session` でセッション情報に保存する (`src/server/terminal.rs:37-45`, `src/server/terminal.rs:102-252`)
- [ ] 1.3 `create_terminal` API ハンドラーで `project_id`/`root` を `CreateTerminalRequest` に渡す (`src/server/api.rs:2508-2541`)
- [ ] 1.4 `TerminalSession` に `scrollback: Arc<Mutex<VecDeque<u8>>>` リングバッファ（64KB上限）を追加する (`src/server/terminal.rs:75-83`)
- [ ] 1.5 PTY出力読み取り関数 (`read_pty_output`) でリングバッファにも書き込む (`src/server/terminal.rs`)
- [ ] 1.6 `TerminalManager` に `get_scrollback(&self, session_id: &str) -> Result<Vec<u8>, String>` メソッドを追加する
- [ ] 1.7 `handle_terminal_ws` で WebSocket 接続直後にスクロールバック内容をバイナリメッセージとして送信する (`src/server/api.rs:2601-2699`)
- [ ] 1.8 バックエンドの単体テスト: セッション作成時に `project_id`/`root` が保持されることを確認 (verification: `cargo test terminal`)

### Frontend

- [ ] 2.1 `restClient.ts` の `TerminalSessionInfo` 型に `project_id` と `root` フィールドを追加する (`dashboard/src/api/restClient.ts:203-209`)
- [ ] 2.2 `TerminalPanel` のマウント時に `listTerminalSessions()` を呼び出し、現在の `projectId`/`root` に一致する既存セッションをタブとして復元するロジックを追加する (`dashboard/src/components/TerminalPanel.tsx`)
- [ ] 2.3 `TerminalPanel` の `root` prop変更時に、タブを破棄せず新しい `root` に一致するセッションのみ表示するフィルタロジックを実装する
- [ ] 2.4 タブのラベルにセッションIDの代わりにworktree名（`root` から抽出、例: `worktree:feature-x` → `feature-x`、`base` → `base`）を表示する (`dashboard/src/components/TerminalPanel.tsx:131-133`)
- [ ] 2.5 `TerminalTab` の WebSocket 接続後、スクロールバック（サーバーから送信される初回バイナリメッセージ群）が自動的に表示されることを確認する（既存の `ws.onmessage` ハンドラーで対応済みのはず、追加コード不要の可能性あり）

### Integration

- [ ] 3.1 手動検証: ブラウザリロード後に既存ターミナルタブが復元され、直近の出力が表示されることを確認
- [ ] 3.2 手動検証: worktree切り替え時に既存ターミナルが破棄されず、切り替え先のセッションのみ表示されることを確認
- [ ] 3.3 `cargo clippy -- -D warnings && cargo fmt --check` が通ることを確認
- [ ] 3.4 `dashboard/` で `npm run build` が成功することを確認

## Future Work

- サーバー再起動後のセッション復元（PTYが死ぬため現状不可能、tmux/screen統合が必要）
- セッション自動クリーンアップ（一定時間使用されていないセッションの自動削除）
