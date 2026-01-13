# parallel-execution 仕様変更

## ADDED Requirements

### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは各変更が archive 完了した時点で**即座に個別マージ**を実行しなければならない（SHALL）。

**Rationale**: グループ単位の一括マージでは、1つの変更が詰まると他の完了した変更も archive されない問題があった。個別マージにより、完了した変更を即座に本体ブランチに反映し、詰まり耐性を向上させる。

#### Scenario: Archive 完了後に個別マージが実行される

- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** archive の結果に `final_revision` が含まれている
- **WHEN** archive 処理が完了する
- **THEN** システムは即座に `merge_and_resolve(&[final_revision])` を呼び出す
- **AND** マージが成功した場合、変更 A が本体ブランチに反映される
- **AND** 他の変更の完了を待たずにマージが実行される

#### Scenario: 1つの変更が詰まっても他の変更は正常にマージされる

- **GIVEN** 並列実行モードでグループ内に変更 A, B, C がある
- **AND** 変更 A と B は正常に archive 完了した
- **AND** 変更 C の apply が詰まっている
- **WHEN** 変更 A と B の archive が完了する
- **THEN** 変更 A と B は即座に個別マージされる
- **AND** 変更 A と B は本体ブランチに反映される
- **AND** 変更 C の詰まりが他の変更に影響しない

#### Scenario: マージ失敗時は従来通り conflict resolution が実行される

- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **WHEN** 個別マージ中に conflict が検出される
- **THEN** `VcsError::Conflict` エラーが返される
- **AND** 既存の conflict resolution ロジックが実行される
- **AND** ワークスペースは保持される

#### Scenario: MergeStarted イベントがマージ開始時に発行される

- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **WHEN** 個別マージが開始される
- **THEN** `ParallelEvent::MergeStarted { change_id, revision }` が発行される
- **AND** TUI のログパネルに「Merging revision {revision}」が表示される

#### Scenario: MergeCompleted イベントがマージ成功時に発行される

- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** 個別マージが成功した
- **WHEN** マージが完了する
- **THEN** `ParallelEvent::MergeCompleted { change_id, merged_revision }` が発行される
- **AND** TUI のログパネルに「Merged as {merged_revision}」が表示される
