## MODIFIED Requirements

### Requirement: In-flight tracking and slot-based dispatch

システムは in-flight の change を追跡し、空きスロット数を算出しなければならない（MUST）。

in-flight は apply/acceptance/archive/resolve の change とし、resolve には並列実行による自動 resolve と TUI からの手動 resolve の両方を含めなければならない（MUST）。merged/merge_wait/error/not queued を in-flight として扱ってはならない（MUST NOT）。

空きスロット数は `max_concurrent_workspaces - in_flight_count` で算出し、0 未満にならないように扱わなければならない（MUST）。

re-analysis の `order` は依存関係の制約として扱い、依存解決済みの change だけを空きスロット数分 dispatch しなければならない（MUST）。

#### Scenario: 空きスロット数に応じてdispatchする
- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** in-flight が 2 件である
- **AND** queued に依存解決済みの change が 2 件ある
- **WHEN** re-analysis が dispatch を行う
- **THEN** 1 件のみ dispatch される

#### Scenario: in-flight に非アクティブ状態が含まれない
- **GIVEN** merged/merge_wait/error/not queued の change が存在する
- **WHEN** 並列実行が in-flight を算出する
- **THEN** それらの change は in-flight として数えられない

#### Scenario: 手動 resolve は in-flight に含まれる
- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** apply/acceptance/archive で in-flight が 2 件である
- **AND** TUI から手動 resolve が開始される
- **WHEN** 並列実行が空きスロット数を算出する
- **THEN** in-flight は 3 件として扱われる
- **AND** queued の change はスロットが空くまで dispatch されない
