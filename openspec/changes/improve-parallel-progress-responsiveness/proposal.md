# Proposal: 並列モードの進捗表示を即座に反映する

## Why

現状、並列モードの TUI auto-refresh（5秒間隔）は、ベース作業ツリーの `openspec/changes/{change_id}/tasks.md` から進捗を取得しています。しかし、AI agent が worktree 内で `tasks.md` を更新しても、WIP commit が作成されるまで進捗が反映されません。

この遅延により、ユーザーは apply ループの進捗状況をリアルタイムで把握できず、UX が損なわれています。

Apply ループ内では `ProgressUpdated` イベントが送信されていますが、auto-refresh が古い進捗で上書きしてしまうため、イベント駆動の更新が無効化されています。

## What Changes

TUI auto-refresh が worktree 内の**未コミット** `tasks.md` を優先的に読み取るようにします。

**具体的な変更**:
1. Worktree path resolver を追加（`src/vcs/git/mod.rs`）
2. Worktree 優先の task parser を追加（`src/task_parser.rs`）
3. TUI auto-refresh で worktree 進捗を enrichment（`src/tui/runner.rs`）

**変更しないこと**:
- Apply ループ自体は変更しない（既に worktree から正しく読んでいる）
- `ProgressUpdated` イベントの送信ロジックは変更しない
- 実行対象の判定基準（`HEAD` コミットツリーベース）は変更しない

## Impact

**効果**:
- 進捗の遅延が WIP commit 作成待ち（数秒〜数十秒）から auto-refresh 間隔（最大 5 秒）に短縮
- ユーザーは apply ループの進行状況をほぼリアルタイムで把握可能

**影響範囲**:
- `src/vcs/git/mod.rs` - worktree path 解決ロジック追加
- `src/task_parser.rs` - worktree 優先読み取り関数追加
- `src/tui/runner.rs` - auto-refresh ループの更新

**パフォーマンス**:
- `git worktree list --porcelain` を 5 秒間隔で呼ぶオーバーヘッド（許容範囲）
- 必要に応じて将来的にキャッシュを追加可能

**破壊的変更**:
- なし（既存の動作を改善するのみ）
