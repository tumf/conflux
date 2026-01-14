## MODIFIED Requirements

### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは各変更が archive 完了した時点で**即座に個別マージ**を実行しなければならない（SHALL）。

ただし、統合先ブランチ（base）が dirty（未コミット変更/未追跡ファイルの存在、または Git がマージ進行中状態）である場合、システムは個別マージを実行してはならない（SHALL NOT）。

この場合、システムは対象 change を `MergeWait` 状態として保持し、worktree をクリーンアップせずに維持しなければならない（SHALL）。

#### Scenario: Archive 完了後のマージに OpenSpec の change_id を適用する
- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** 変更 A の worktree ブランチ名と OpenSpec の change_id が取得できる
- **WHEN** archive 処理が完了する
- **THEN** システムは worktree ブランチ名をマージ対象として `resolve_command` を実行する
- **AND** マージコミットには OpenSpec の change_id が含まれる

#### Scenario: base が dirty のとき個別マージを延期する
- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** base ブランチが dirty（例: `git status --porcelain` が空ではない、または `MERGE_HEAD` が存在する）である
- **WHEN** システムが変更 A の個別マージを開始しようとする
- **THEN** システムは変更 A の個別マージを実行しない
- **AND** `ExecutionEvent::MergeDeferred` を発行する
- **AND** 変更 A は `MergeWait` として保持される

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、自動的にスキップされなければならない（MUST）。

さらに、`MergeWait` により未統合の change を依存先に持つ変更は実行を保留し、今回の run では実行してはならない（MUST）。

#### Scenario: Dependent change skipped
- Given: `change-A` が失敗として記録されている
- And: `change-B` は `change-A` に依存している
- When: `change-B` の実行が開始されようとする
- Then: `change-B` はスキップされる
- And: `ChangeSkipped` イベントが発行される

#### Scenario: Independent change continues
- Given: `change-A` が失敗として記録されている
- And: `change-C` は `change-A` に依存していない
- When: `change-C` の実行が開始されようとする
- Then: `change-C` は通常通り実行される

#### Scenario: Skip reason logged
- Given: `change-B` が依存先 `change-A` の失敗によりスキップされる
- When: スキップが発生する
- Then: ログに「Skipping change-B because dependency change-A failed」が出力される

#### Scenario: `MergeWait` 依存の change はキューに残したまま実行しない
- **GIVEN** 変更 A が `MergeWait` であり base に未統合である
- **AND** 変更 B が変更 A に依存している
- **AND** 変更 B はキューに存在する
- **WHEN** 並列実行が次の実行対象を選定する
- **THEN** システムは変更 B を今回の run では実行しない
- **AND** 変更 B はキューから削除されない

### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination (cancellation, graceful stop, or normal completion) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

加えて、`merge_wait` を終了理由として区別し、成功完了と誤解される完了イベント/メッセージを送信してはならない（SHALL NOT）。

#### Scenario: Tracking stopped or cancelled state
- **Given** the parallel orchestration loop is running
- **When** the loop checks for cancellation or graceful stop
- **And** either condition is true
- **Then** a `stopped_or_cancelled` flag should be set to true
- **And** the loop should break
- **And** this flag should prevent sending completion events after the loop

#### Scenario: Tracking error state during batch processing
- **Given** the parallel orchestration loop is processing batches
- **When** a batch execution returns an error
- **Then** a `had_errors` flag should be set to true
- **And** processing should continue with remaining batches
- **And** this flag should affect the final completion message when all batches finish

#### Scenario: マージ待ちが残る場合は成功完了として扱わない
- **GIVEN** 並列実行で少なくとも 1 件の change が `MergeWait` で残っている
- **WHEN** 実行可能な queued change の処理が完了する
- **THEN** システムは `AllCompleted` 相当の成功完了を通知しない
- **AND** 停止/待機（merge 待ち）として扱われる

### Requirement: Parallel Execution Event Reporting

parallel 実行モジュールは、統一された `ExecutionEvent` 型を使用してイベントを発行しなければならない（SHALL）。

#### Scenario: Workspace 作成イベント
- **GIVEN** parallel executor が change 用のワークスペースを作成する
- **WHEN** ワークスペースの作成が完了する
- **THEN** `ExecutionEvent::WorkspaceCreated` が発行される
- **AND** イベントには change_id と workspace path が含まれる

#### Scenario: ProcessingStarted の早期発行
- **GIVEN** parallel executor が change のワークスペースを作成または再利用する
- **WHEN** change の処理準備が完了する
- **THEN** `ExecutionEvent::ProcessingStarted(change_id)` が発行される
- **AND** TUI は該当 change を processing 状態として表示する

#### Scenario: Apply 進捗イベント
- **GIVEN** parallel executor が change を処理している
- **WHEN** apply コマンドが完了し進捗が更新される
- **THEN** `ExecutionEvent::ProgressUpdated` が発行される
- **AND** イベントには completed と total タスク数が含まれる

#### Scenario: マージ完了イベント
- **GIVEN** parallel executor が複数の change をマージする
- **WHEN** マージが成功する
- **THEN** `ExecutionEvent::MergeCompleted` が発行される
- **AND** イベントにはマージされた change_ids とリビジョンが含まれる

#### Scenario: マージ延期イベント
- **GIVEN** 変更 A が archive 完了している
- **AND** base が dirty で個別マージが実行できない
- **WHEN** システムが個別マージを延期する
- **THEN** `ExecutionEvent::MergeDeferred { change_id, reason }` が発行される
- **AND** `reason` には dirty 判定の根拠が含まれる
