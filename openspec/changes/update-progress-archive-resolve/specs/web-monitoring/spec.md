## MODIFIED Requirements
### Requirement: WebSocket - Real-time Updates
The HTTP server SHALL broadcast state updates to the dashboard via WebSocket for both serial and parallel execution (`--parallel`) modes.
The HTTP server SHALL ensure that TUI and Web UI states remain consistent by broadcasting real-time state updates that occur in the TUI via WebSocket.
This broadcast MUST be based on a unified state model that includes not only change list progress but also TUI-visible states (queue status, logs, worktrees, running operations, etc.).
For dashboard compatibility, the `changes` field in `state_update` messages MUST always be a complete snapshot of the change list (MUST).
すべての状態で tasks.md から取得できる進捗を state_update に反映し、completed を 0 に上書きしてはならない（MUST NOT）。
進捗取得に失敗した場合でも completed を 0 に上書きしてはならない（MUST NOT）。取得失敗は 0 件完了とは別の状態として扱う。

#### Scenario: 任意の状態で progress を保持する
- **GIVEN** Web UI が state_update を受信している
- **AND** tasks.md から進捗が取得できる
- **WHEN** state_update が送信される
- **THEN** completed_tasks/total_tasks は最新の進捗を反映する
- **AND** completed を 0 に上書きしない

#### Scenario: 任意の状態で進捗取得失敗は直前値を保持する
- **GIVEN** Web UI が state_update を受信している
- **AND** tasks.md の読み取りに失敗する
- **WHEN** state_update が送信される
- **THEN** completed_tasks/total_tasks は直前の値を維持する
- **AND** 取得失敗を 0 件完了として扱わない
