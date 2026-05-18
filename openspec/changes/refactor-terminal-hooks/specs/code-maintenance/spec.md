## ADDED Requirements

### Requirement: Terminal hook refactor preserves session lifecycle

WebUI terminal component の hook 依存関係整理は、terminal session の restore、auto-create、active tab selection、WebSocket/xterm lifecycle の外部観測可能な挙動を変更してはならない。

#### Scenario: terminal panel の session 管理が維持される

**Given**: 既存 terminal session と現在の `projectId` / `root` がある
**When**: terminal panel が mount、expand、または root 切替される
**Then**: 既存と同じ条件で session が restore または auto-create される
**And**: active tab selection は分割前と同等である

#### Scenario: terminal tab の接続 lifecycle が維持される

**Given**: terminal tab が `sessionId` に対して WebSocket と xterm instance を持つ
**When**: tab が mount、resize、入力送信、unmount、または `sessionId` 変更される
**Then**: WebSocket 接続、resize 送信、入力送信、cleanup は分割前と同等である
**And**: REST/WebSocket payload とユーザー向け terminal message は変更されない
