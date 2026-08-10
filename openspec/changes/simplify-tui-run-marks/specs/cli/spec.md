## MODIFIED Requirements

### Requirement: Dynamic Execution Queue

The orchestrator SHALL retain explicit DynamicQueue add/remove services for clients that intentionally invoke queue commands. Space and bulk `x` are execution-mark controls and MUST NOT add, remove, stop, dequeue, or otherwise mutate current-run work, including queued work that has not started and Applying/Accepting/Archiving/Resolving work.

Applying/Accepting/Archiving/Resolving changes MUST continue to reject `@` state mutation. Their independent per-change termination control is `K: kill`.

#### Scenario: Running Space does not remove queued current-run work

- **GIVEN** TUI is in Running mode
- **AND** a change is queued but has not started Processing
- **WHEN** the user unmarks it with Space
- **THEN** its execution mark becomes false
- **AND** the change remains admitted to the current run
- **AND** no DynamicQueue remove or dequeue command is emitted

#### Scenario: Running Space does not stop active work

- **GIVEN** TUI is in Running mode
- **AND** a change is Applying, Accepting, Archiving, or Resolving
- **WHEN** the user toggles its mark with Space
- **THEN** only its execution mark changes
- **AND** no stop or cancellation request is emitted
- **AND** `K: kill` remains the independent guarded termination control

#### Scenario: Processing 中の change で @ は無効

- **GIVEN** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが `@` キーを押す
- **THEN** queue_status と選択状態は変更されない

#### Scenario: Explicit queue command retains queue semantics

- **GIVEN** a client intentionally invokes an explicit queue add or remove command
- **WHEN** the shared queue service accepts it
- **THEN** DynamicQueue changes according to that explicit command
- **AND** the operation does not derive its effect from Space or bulk execution-mark state

### Requirement: Archived 状態の checkbox 表示

TUI は terminal row の checkbox / execution mark semantics を、その row が execution candidate かどうかに応じて表現しなければならない（SHALL）。

`archived`、`merged`、または `pushed` 状態の change は execution candidate ではないため、checkbox テキストとして `[x]` または `[ ]` を表示してはならない（MUST NOT）。TUI は既存 checkbox と同じ表示幅の空白を描画し、cursor、change ID、badge、status、progress、および preview の開始位置を詰めてはならない（MUST NOT）。Rejected 状態の change も execution candidate ではなく、以前の execution mark を保持したまま表示してはならない（MUST NOT）。

#### Scenario: rejected 状態では x マークを保持しない

- **GIVEN** TUI が change 一覧を表示している
- **AND** ある change が rejection flow 完了により `rejected` 状態へ遷移した
- **WHEN** 画面が次にレンダリングされる
- **THEN** その change は execution mark なし (`selected = false`) で表示される
- **AND** ステータス表示は `rejected` のままである

#### Scenario: 実行モードで archived 状態の checkbox を表示しない

- **GIVEN** TUI が実行モードである
- **AND** ある change の display status が `archived`、`merged`、または `pushed` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 領域に `[x]` も `[ ]` も表示されない
- **AND** checkbox と同じ幅の空白が表示される
- **AND** cursor、change ID、badge、status、progress、および preview の開始位置は非 terminal 行と同じ列に維持される

#### Scenario: 選択モードに戻った際も post-archive checkbox は非表示

- **GIVEN** 処理が完了し TUI が選択モードに戻った
- **AND** ある change の display status が `archived`、`merged`、または `pushed` である
- **WHEN** 画面がレンダリングされる
- **THEN** checkbox テキストは表示されない
- **AND** 行の残りの表示位置は詰められない

#### Scenario: post-archive 行の Space は silent no-op

- **GIVEN** cursor が `archived`、`merged`、または `pushed` 行にある
- **WHEN** ユーザーが Space を押す
- **THEN** execution mark、queue intent、runtime state、および表示状態は変化しない
- **AND** mark refusal warning は表示されない

### Requirement: Error Retry with F5 Key

Configured Start/F5 SHALL evaluate the authoritative marked target set at final admission. A marked retry-eligible change SHALL be retried from Ready/Select, Stopped, or process-wide Error without requiring a process-wide Error transition. A change carrying active-run Apply iteration-limit evidence SHALL remain ineligible until its owning run closes. When retry and ordinary-start marks coexist, the invocation SHALL dispatch only retry routes, preserve ordinary marks for a later Start, and report those ordinary rows as deferred.

#### Scenario: Ready re-mark and F5 retries failed change

- **GIVEN** a change-scoped failure moved `alpha` to `error`
- **AND** the process later projects Ready/Select
- **AND** the operator re-marks `alpha`
- **WHEN** the user presses configured Start/F5
- **THEN** `alpha` is added back through the typed retry route
- **AND** processing resumes without first entering process-wide Error

#### Scenario: Mixed retry and ordinary marks do not share retry budget

- **GIVEN** retry-eligible `alpha` and ordinary `not queued` `beta` are marked
- **WHEN** the user presses configured Start/F5
- **THEN** this invocation dispatches only `alpha` with explicit-retry semantics
- **AND** `beta` remains marked and is reported as deferred
- **AND** a later configured Start can admit `beta` normally

#### Scenario: F5 cannot target an active limited run

- **GIVEN** a marked error change carries Apply iteration-limit evidence owned by an active run
- **WHEN** the user presses configured Start/F5
- **THEN** no retry, queue, mark, explicit-retry, or scheduler effect occurs for that change
- **AND** the active-limit explanation remains visible

#### Scenario: F5 becomes available after boundary closure

- **GIVEN** the owning run closed and the active Apply-limit gate retired
- **AND** the still-retryable change is marked
- **WHEN** the user presses configured Start/F5
- **THEN** ordinary retry admission may start a later scheduler boundary
- **AND** the later boundary uses workspace-derived state and a fresh Apply budget

<!-- Expected canonical result after archive: `cli` will reserve DynamicQueue mutation for explicit queue commands, preserve rejected mark clearing, replace gray post-archive `[x]` rendering with a fixed-width blank checkbox placeholder, and make configured Start/F5 consume marked change-level retry routes from Ready/Select without applying retry semantics to deferred ordinary work. -->
