## MODIFIED Requirements

### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは各変更が archive 完了した時点で、原則として個別マージを実行しなければならない（SHALL）。

ただし、統合先ブランチ（base）が dirty（未コミット変更/未追跡ファイルの存在、または Git がマージ進行中状態）である場合、システムは個別マージを実行してはならない（SHALL NOT）。

この場合、システムは対象 change を `MergeWait` 状態として保持し、worktree をクリーンアップせずに維持しなければならない（SHALL）。

#### Scenario: base が dirty のとき個別マージを延期する
- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** base ブランチが dirty（例: `git status --porcelain` が空ではない、または `MERGE_HEAD` が存在する）である
- **WHEN** システムが変更 A の個別マージを開始しようとする
- **THEN** システムは変更 A の個別マージを実行しない
- **AND** `ExecutionEvent::MergeDeferred` を発行する
- **AND** 変更 A は `MergeWait` として保持される

### Requirement: Dependent Change Skipping

システムは、実行不能な依存関係が存在する change を安全側に倒して実行してはならない（SHALL NOT）。

ここでの「実行不能な依存関係」には、失敗した change だけでなく、`MergeWait` により未統合の change を含めなければならない（MUST）。

#### Scenario: `MergeWait` 依存の change はキューに残したまま実行しない
- **GIVEN** 変更 A が `MergeWait` であり base に未統合である
- **AND** 変更 B が変更 A に依存している
- **AND** 変更 B はキューに存在する
- **WHEN** 並列実行が次の実行対象を選定する
- **THEN** システムは変更 B を今回の run では実行しない
- **AND** 変更 B はキューから削除されない

### Requirement: Independent change continues

システムは、`MergeWait` の change が存在しても、それに依存しない queued change の処理を継続しなければならない（SHALL）。

#### Scenario: `MergeWait` があっても独立 change は継続する
- **GIVEN** 変更 A が `MergeWait` である
- **AND** 変更 C は変更 A に依存しない
- **AND** 変更 C はキューに存在する
- **WHEN** 並列実行が継続する
- **THEN** システムは変更 C の apply/archive を通常通り実行できる

### Requirement: Loop termination reason must be tracked and distinguished

システムはループ終了理由として `merge_wait` を区別し、成功完了と誤解される完了イベント/メッセージを送信してはならない（SHALL NOT）。

#### Scenario: マージ待ちが残る場合は成功完了として扱わない
- **GIVEN** 並列実行で少なくとも 1 件の change が `MergeWait` で残っている
- **WHEN** 実行可能な queued change の処理が完了する
- **THEN** システムは `AllCompleted` 相当の成功完了を通知しない
- **AND** 停止/待機（merge 待ち）として扱われる

### Requirement: Parallel Execution Event Reporting

parallel 実行モジュールは、マージ延期を示すイベントを発行しなければならない（SHALL）。

#### Scenario: マージ延期イベント
- **GIVEN** 変更 A が archive 完了している
- **AND** base が dirty で個別マージが実行できない
- **WHEN** システムが個別マージを延期する
- **THEN** `ExecutionEvent::MergeDeferred { change_id, reason }` が発行される
- **AND** `reason` には dirty 判定の根拠が含まれる
