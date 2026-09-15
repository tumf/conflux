## MODIFIED Requirements

### Requirement: Parallel execution enforces workspace concurrency limit

システムは parallel 実行時、workspace admission、worktree 作成、apply、acceptance、archive、background merge、automatic resolve、manual resolve を含む、change が terminal settlement に達するまでの全工程で `max_concurrent_workspaces` の上限を厳密に適用しなければならない（MUST）。同時に存在する managed worktree 数と同時に admitted lifecycle を所有する change 数は上限を超えてはならない（MUST NOT）。

change は workspace preparation の直前に lifecycle slot を取得し、phase または async task が切り替わっても同じ slot を移譲しなければならない（MUST）。apply/acceptance/archive task の完了、background merge の spawn、conflict 検出、resolve 開始、および `merge_wait` 遷移だけを理由に slot を解放してはならない（MUST NOT）。

slot は `merged`、terminal `error`、`rejected`、明示的 dequeue/not-queued、または設定された push/publication terminal settlement の repository-visible evidence が確定した時点で解放しなければならない（MUST）。process restart 後の occupancy は workspace、Git、および reducer evidence から再構成しなければならず（MUST）、out-of-worktree durable slot state を workflow authority として導入してはならない（MUST NOT）。

#### Scenario: worktree 作成も同時数上限の対象になる

- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** parallel 実行で 10 件の change が対象である
- **WHEN** worktree の作成と apply が進行する
- **THEN** 同時に作成・実行される worktree は最大 3 件までに制限される
- **AND** 残りの change はスロットが空くまで待機する

#### Scenario: archive completion transfers rather than releases the slot

- **GIVEN** `max_concurrent_workspaces` が 3 であり、3件の change が admitted lifecycle slot を所有している
- **AND** 1件が archive を完了し background merge を開始しようとしている
- **AND** queued に別の依存解決済み change が存在する
- **WHEN** workspace task が完了して merge task へ処理を移す
- **THEN** 完了した change の slot は merge task へ移譲される
- **AND** queued change は新規 dispatch されない
- **AND** admitted lifecycle と managed worktree の総数は3を超えない

#### Scenario: automatic resolve stays inside the existing lifecycle slot

- **GIVEN** admitted change が slot を保持して background merge 中である
- **WHEN** merge conflict が検出され automatic resolve が開始される
- **THEN** resolve は同じ lifecycle slot の内側で実行される
- **AND** resolve counter または表示用phaseによって二重計上されない
- **AND** applying と resolving の合計 admitted lifecycle 数は上限を超えない

#### Scenario: merge wait preserves backpressure

- **GIVEN** admitted change が merge conflict を解消できず `merge_wait` に遷移する
- **WHEN** queued change の dispatch capacity を計算する
- **THEN** `merge_wait` change は既存 lifecycle slot を保持する
- **AND** operator resolve、dequeue、rejection、terminal error、または successful merge がsettleするまで queued change はそのslotを利用できない

#### Scenario: terminal settlement releases capacity

- **GIVEN** admitted change が lifecycle slot を保持している
- **WHEN** `merged`、terminal `error`、`rejected`、明示的 dequeue/not-queued、または設定された push/publication terminal settlement が repository-visible evidenceで確定する
- **THEN** lifecycle slot は一度だけ解放される
- **AND** scheduler は既存のcompletionまたはslot-recovery edgeでqueued workを再評価する

#### Scenario: restart reconstructs retained merge-wait occupancy

- **GIVEN** process restart 前から managed worktree と reducer evidence が retained `merge_wait` change を示す
- **WHEN** scheduler が restart 後の dispatch capacity を分類する
- **THEN** retained change は lifecycle slot occupancy として再構成される
- **AND** 外部のdurable slot leaseを参照せず、新規dispatchをfail-closedに制限する

<!-- Expected canonical result after archive: concurrency is owned by each admitted change continuously through merge and resolve settlement, including retained merge_wait backpressure, with repository-derived restart recovery. -->

### Requirement: In-flight tracking and slot-based dispatch

システムは admitted lifecycle の change ID を一意に追跡し、空きスロット数を算出しなければならない（MUST）。

slot occupancy は workspace preparation、apply、acceptance、archive、background merge、automatic resolve、manual resolve、および retained `merge_wait` の change とし、同一changeをphase counterとlifecycle membershipで二重計上してはならない（MUST NOT）。`merged`、terminal `error`、`rejected`、明示的 dequeue/not queued、および設定された push/publication terminal settlement は occupancy として扱ってはならない（MUST NOT）。

空きスロット数は `max_concurrent_workspaces - unique_lifecycle_occupancy_count` で算出し、0未満にならないように扱わなければならない（MUST）。

re-analysis の `order` は依存関係の制約として扱い、依存解決済みの change だけを空きスロット数分 dispatch しなければならない（MUST）。

#### Scenario: 空きスロット数に応じてdispatchする

- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** lifecycle slot を所有する change が 2 件である
- **AND** queued に依存解決済みの change が 2 件ある
- **WHEN** re-analysis が dispatch を行う
- **THEN** 1 件のみ dispatch される

#### Scenario: in-flight に非アクティブ状態が含まれない

- **GIVEN** `merged`、terminal `error`、`rejected`、明示的 dequeue/not queued、および設定された push/publication terminal settlement の change が存在する
- **WHEN** 並列実行が lifecycle occupancy を算出する
- **THEN** それらの change は occupancy として数えられない
- **AND** 未settleの `merge_wait` は非アクティブでも occupancy として数えられる

#### Scenario: 手動 resolve は in-flight に含まれる

- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** apply/acceptance/archive で lifecycle slot を所有する change が 2 件である
- **AND** TUI から3件目の admitted change に対する手動 resolve が開始される
- **WHEN** 並列実行が空きスロット数を算出する
- **THEN** unique lifecycle occupancy は 3 件として扱われる
- **AND** 手動 resolve は対象 change が既に所有する slot の内側で実行され二重計上されない
- **AND** queued の change はスロットが空くまで dispatch されない

#### Scenario: available slots use unique lifecycle occupancy

- **GIVEN** `max_concurrent_workspaces` が3である
- **AND** applyingが2件、background mergeまたはresolveが1件である
- **WHEN** schedulerが空きスロット数を算出する
- **THEN** unique lifecycle occupancyは3件である
- **AND** queued changeはdispatchされない

#### Scenario: phase transition does not create an unowned interval

- **GIVEN** changeがarchive completionからbackground mergeへ遷移する
- **WHEN** scheduler eventとmerge task spawnがinterleaveする
- **THEN** change IDはすべてのobservable stateでlifecycle occupancyに1回だけ含まれる
- **AND** 0回または2回として扱われる中間状態は存在しない

#### Scenario: full-capacity manual resolve does not exceed the limit

- **GIVEN** `max_concurrent_workspaces` が3である
- **AND** manual resolve対象を含む3件のchangeがlifecycle slotを所有している
- **WHEN** TUIからmanual resolveが開始される
- **THEN** manual resolveは対象changeが保持するslot内で実行される
- **AND** admitted lifecycleの総数は3を超えない
- **AND** queued changeはslotがterminal settlementで解放されるまでdispatchされない

#### Scenario: terminal states are removed from occupancy once

- **GIVEN** changeがlifecycle slotを所有している
- **WHEN** changeが`merged`、terminal `error`、`rejected`、明示的dequeue/not queued、またはpush/publication terminal settlementへ遷移する
- **THEN** change IDはoccupancyから一度だけ除外される
- **AND** duplicate completion deliveryは追加slotを生成しない

<!-- Expected canonical result after archive: slot-based dispatch counts unique admitted change lifecycles through merge_wait and resolve, and releases capacity only at repository-visible settlement. -->
