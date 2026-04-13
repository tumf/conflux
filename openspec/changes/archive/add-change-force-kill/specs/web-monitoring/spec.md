## MODIFIED Requirements

### Requirement: Control API State Transitions
The web monitoring API SHALL enforce execution-mode-aware control transitions.

`POST /api/control/run` MUST only succeed when `app_mode` is `select`, `stopped`, or `error`, and MUST start orchestration on success.
`POST /api/control/stop` MUST only succeed when `app_mode` is `running`, and MUST initiate graceful stop.
`POST /api/control/cancel-stop` MUST only succeed when `app_mode` is `stopping`, and MUST resume execution.
`POST /api/control/force-stop` MUST only succeed when `app_mode` is `running` or `stopping`, and MUST terminate the global execution.

For single-change stop-and-dequeue requests, the project-scoped API MUST distinguish between queued and active changes. When the target change is active, the server MUST force-kill the in-flight execution associated with that change before completing the request. The server MUST NOT report successful dequeue for an active change until the force-kill has succeeded. If force-kill fails, the server MUST return an error and preserve the active execution state.

The WebUI MUST treat active-change stop as a destructive action. Clicking the stop control for an active change MUST first open a confirmation dialog, and only an explicit confirm action from that dialog MAY invoke the stop-and-dequeue API.

#### Scenario: 強制停止要求
- **WHEN** クライアントが `POST /api/control/force-stop` を送信する
- **AND** `app_mode` が `stopping` または `running` である
- **THEN** サーバーは実行中プロセスを終了し停止状態へ遷移する
- **AND** 成功時は HTTP 200 を返す

#### Scenario: 強制停止不可の状態
- **WHEN** `app_mode` が `select` または `stopped` である
- **AND** クライアントが `POST /api/control/force-stop` を送信する
- **THEN** サーバーは HTTP 409 を返す
- **AND** 実行状態を変更しない

#### Scenario: WebUI requires confirmation before active change stop
- **GIVEN** a change is active in project execution
- **WHEN** the user clicks the Stop control in the WebUI
- **THEN** the UI SHALL open a confirmation dialog
- **AND** the stop-and-dequeue API SHALL NOT be called yet

#### Scenario: Active change stop-and-dequeue force-kills execution after confirmation
- **GIVEN** a change is active in project execution
- **AND** the user confirmed stop in the WebUI dialog
- **WHEN** the client sends `POST /api/v1/projects/{project_id}/changes/{change_id}/stop-and-dequeue`
- **THEN** the server SHALL force-kill the in-flight execution for that change
- **AND** only after successful kill SHALL the response indicate `status = not queued` and `selected = false`

#### Scenario: WebUI cancel leaves active change untouched
- **GIVEN** a change is active in project execution
- **AND** the WebUI is showing the stop confirmation dialog
- **WHEN** the user cancels the dialog
- **THEN** the stop-and-dequeue API SHALL NOT be called
- **AND** the change SHALL remain active

#### Scenario: Queued change stop-and-dequeue does not require force-kill
- **GIVEN** a change is queued but has not started execution
- **WHEN** the client sends `POST /api/v1/projects/{project_id}/changes/{change_id}/stop-and-dequeue`
- **THEN** the server MAY dequeue the change without a process kill
- **AND** the response SHALL indicate `status = not queued`

#### Scenario: Active change stop-and-dequeue surfaces kill failure
- **GIVEN** a change is active in project execution
- **WHEN** the client sends `POST /api/v1/projects/{project_id}/changes/{change_id}/stop-and-dequeue`
- **AND** the backend cannot force-kill the in-flight execution
- **THEN** the server SHALL return an error response
- **AND** the change SHALL remain active rather than being reported as dequeued
