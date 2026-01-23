## MODIFIED Requirements
### Requirement: Queue State Synchronization
システムは、UI上のキュー状態とDynamicQueueの状態を常に同期させなければならない（SHALL）。

`ResolveWait`はresolve待機中の状態であり、Space/@によるキュー操作でDynamicQueueを変更してはならない（MUST NOT）。TUIは`ResolveWait`を`resolve wait`として表示し、操作対象外であることが明確でなければならない（MUST）。

#### Scenario: Unapproveによるキューからの削除
- **WHEN** ユーザーが@キーでqueuedのchangeをunapprove
- **THEN** `QueueStatus::NotQueued` に変更され、DynamicQueueからも削除される

#### Scenario: Spaceキーによるキューからの削除
- **WHEN** ユーザーがRunningモード中にSpaceキーで [x] のchangeをdequeue
- **THEN** `QueueStatus::NotQueued` に変更され、DynamicQueueからも削除される

#### Scenario: 削除操作のログ記録
- **WHEN** DynamicQueueからchangeが削除される
- **THEN** ログに削除操作が記録される

#### Scenario: ResolveWait中はキュー状態を変更できない
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `ResolveWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for the change

### Requirement: Event-Driven State Updates
TUI は 5 秒ごとの自動更新で `MergeWait` を評価し、以下のいずれかを満たす場合は `Queued` に戻さなければならない（MUST）。

- 対応する worktree が存在しない
- 対応する worktree が存在し、worktree ブランチが base に ahead していない

自動解除された change では `MergeWait` ではないため、`M` による merge resolve の操作ヒントや実行を行ってはならない（MUST NOT）。

さらに、resolveがシリアライズされて待機状態となっているchangeは`ResolveWait`として保持され、自動更新で`NotQueued`に戻してはならない（MUST NOT）。

#### Scenario: worktree がない場合は MergeWait を解除する
- **GIVEN** change が `MergeWait` である
- **AND** 対応する worktree が存在しない
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `Queued` に戻る

#### Scenario: ahead なしの worktree は MergeWait を解除する
- **GIVEN** change が `MergeWait` である
- **AND** 対応する worktree が存在する
- **AND** worktree ブランチが base に ahead していない
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `Queued` に戻る

#### Scenario: MergeWait が解除された change では M を使えない
- **GIVEN** change が `MergeWait` から `Queued` に戻っている
- **WHEN** TUI のキー表示が描画される
- **THEN** `M` による merge resolve のヒントは表示されない

#### Scenario: ResolveWait は自動更新で保持される
- **GIVEN** change が `ResolveWait` である
- **AND** resolveが別changeで進行中である
- **WHEN** 5秒ポーリングの自動更新が実行される
- **THEN** change のステータスは `ResolveWait` のまま維持される

#### Scenario: WorkspaceState::Archived を持つ change は ResolveWait として識別される
- **GIVEN** worktree が存在し、`detect_workspace_state` が `WorkspaceState::Archived` を返す
- **AND** change が merge されていない（base に ahead している）
- **WHEN** TUI の自動更新が実行される
- **THEN** change のステータスは `ResolveWait` として表示される
- **AND** Space/@キーによる操作は受け付けない
