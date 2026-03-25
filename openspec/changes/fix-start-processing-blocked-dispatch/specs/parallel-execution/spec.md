## MODIFIED Requirements

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、自動的にスキップされなければならない（MUST）。

さらに、`MergeWait` により未統合の change を依存先に持つ変更は実行を保留し、今回の run では実行してはならない（MUST）。依存未解決により実行できない change は queued 状態のまま保持され、ステータス表示は依存待ちであることを示さなければならない（MUST）。

TUI の `start_processing()` は、`Blocked` 状態を含むアクティブ・終端状態の変更を `Queued` に遷移させてはならない（MUST NOT）。`start_processing()` は `NotQueued` 状態の変更のみを `Queued` に遷移させなければならない（SHALL）。

TUI の `handle_stopped()` および `handle_all_completed()` は、`Blocked` 状態の変更を `NotQueued` にリセットしなければならない（SHALL）。これにより、次回の実行開始時に依存関係が再分析され、正しい実行順序が保証される。

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

#### Scenario: 依存待ち状態が表示される
- **GIVEN** 変更 A が base に未統合であり依存関係が未解決である
- **AND** 変更 B が変更 A に依存している
- **AND** 変更 B はキューに存在する
- **WHEN** 並列実行が次の実行対象を選定する
- **THEN** 変更 B は依存待ちとしてマークされる
- **AND** 変更 B のステータス表示は依存待ちであることを示す

#### Scenario: start_processing が Blocked 状態の変更を Queued にしない
- **GIVEN** 変更 B が依存関係により `Blocked` 状態であり `selected=true` である
- **AND** TUI が Select モードである
- **WHEN** ユーザーが F5 を押して `start_processing()` が呼ばれる
- **THEN** 変更 B のステータスは `Queued` に遷移しない
- **AND** 変更 B は `StartProcessing` コマンドの対象に含まれない

#### Scenario: handle_stopped が Blocked 状態を NotQueued にリセットする
- **GIVEN** 変更 B が `Blocked` 状態である
- **WHEN** オーケストレーションが停止して `handle_stopped()` が呼ばれる
- **THEN** 変更 B のステータスは `NotQueued` にリセットされる
- **AND** `selected` フラグは保持される

#### Scenario: handle_all_completed が Blocked 状態を NotQueued にリセットする
- **GIVEN** 変更 B が `Blocked` 状態である
- **WHEN** すべての処理が完了して `handle_all_completed()` が呼ばれる
- **THEN** 変更 B のステータスは `NotQueued` にリセットされる
