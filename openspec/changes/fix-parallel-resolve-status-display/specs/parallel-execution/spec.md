# parallel-execution Specification Delta

## MODIFIED Requirements

### Requirement: 衝突解決時のResolveStartedイベント送信

Parallel実行で衝突解決（conflict resolution）が開始される際、システムは対象 change_id を含む `ResolveStarted { change_id }` イベントを送信しなければならない（SHALL）。

これにより、TUI側で該当 change の状態を `QueueStatus::Resolving` に遷移させ、ユーザーに「どの change が解決中か」を視覚的に示すことができる。

#### Scenario: 自動衝突解決開始時にResolveStartedイベントを送信

- **GIVEN** parallel実行でmerge衝突が発生し、`resolve_conflicts_with_retry` が呼び出される
- **WHEN** 衝突解決が開始される直前
- **THEN** システムは対象 change_id を含む `ResolveStarted { change_id }` イベントを送信する
- **AND** TUIは該当 change の `queue_status` を `QueueStatus::Resolving` に遷移させる
- **AND** TUIには「resolving」ステータスが表示される

#### Scenario: 複数changeの順次マージで各changeにResolveStartedを送信

- **GIVEN** 複数の change を順次マージする `resolve_merges_with_retry` が実行される
- **WHEN** 各 change_id に対して衝突解決が開始される
- **THEN** 各 change_id ごとに `ResolveStarted { change_id }` イベントが送信される
- **AND** TUIでは対象 change が順番に「resolving」ステータスで表示される

#### Scenario: 解決完了時にResolveCompletedイベントを送信

- **GIVEN** 衝突解決が成功裏に完了する
- **WHEN** 解決処理が終了する
- **THEN** システムは `ResolveCompleted { change_id, worktree_change_ids }` イベントを送信する
- **AND** TUIは該当 change の `queue_status` を `QueueStatus::Archived` に遷移させる

#### Scenario: 解決失敗時にResolveFailedイベントを送信

- **GIVEN** 衝突解決が失敗する（最大リトライ回数到達など）
- **WHEN** 解決処理がエラーで終了する
- **THEN** システムは `ResolveFailed { change_id, error }` イベントを送信する
- **AND** TUIは該当 change の `queue_status` を `QueueStatus::MergeWait` に遷移させる
- **AND** エラーメッセージがTUIに表示される
