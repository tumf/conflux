## Context

現在のターミナルセッション管理は「使い捨て」設計。PTYセッション自体はサーバープロセスの生存中は維持されるが、フロントエンドのタブ情報がReact stateのみで永続化されていない。ブラウザリロードでタブ情報が消失し、バックエンドに生存しているPTYセッションへ再接続する手段がない。

## Goals / Non-Goals

- Goals:
  - ブラウザリロード後に既存PTYセッションへ自動復元
  - Worktree切り替え時にセッションを破棄せずコンテキスト別にフィルタ表示
  - 再接続時に直近の出力を表示して文脈を復元
- Non-Goals:
  - サーバー再起動後のセッション復元（PTYが死ぬため不可能）
  - セッション情報のディスク永続化

## Design

### 1. バックエンド: セッションメタデータ拡張

`TerminalSessionInfo` に `project_id: String` と `root: String` を追加する。`create_session` の `CreateTerminalRequest` にもこれらを追加し、セッション作成時に保存する。`list_sessions` のレスポンスにこれらが含まれるため、フロントエンドがフィルタ可能になる。

### 2. バックエンド: スクロールバックバッファ

`TerminalSession` に `Arc<Mutex<VecDeque<u8>>>` のリングバッファ（64KB上限）を追加する。PTY出力読み取りスレッドが `broadcast::channel` に送信する際に、同時にリングバッファにも書き込む。

WebSocket接続ハンドラー (`handle_terminal_ws`) で、`subscribe_output` の前にリングバッファの内容を一括送信する。これにより再接続時に直近の出力が表示される。

新しいAPIメソッド: `get_scrollback(&self, session_id: &str) -> Result<Vec<u8>, String>`

### 3. フロントエンド: セッション復元

`TerminalPanel` のマウント時 (`useEffect`) に:

1. `listTerminalSessions()` で全セッション取得
2. `project_id` と `root` が現在のコンテキストに一致するセッションをフィルタ
3. 一致するセッションをタブとして復元（`TabInfo` を生成）
4. `TerminalTab` が WebSocket 接続時にスクロールバックを自動受信

### 4. Worktree切り替え時の挙動

`root` prop変更時:
- 既存タブを破棄しない（WebSocket接続もPTYも維持）
- 新しい `root` に一致するセッションのみタブバーに表示
- 一致するセッションがなく、パネルが展開中なら新規セッションを自動作成
- タブのラベルに worktree 名（`root` から抽出）を表示

## Alternatives Considered

1. **localStorage でタブ情報を永続化**: セッションIDのみ保存し、リロード時にバックエンドで生存確認。スクロールバックは取れないが実装が簡単。→ スクロールバック無しだと再接続後に空画面で文脈喪失するため不採用。
2. **全セッション常時表示**: worktree切り替えでフィルタせず全タブ表示。→ タブが増えすぎて混乱するため不採用。
