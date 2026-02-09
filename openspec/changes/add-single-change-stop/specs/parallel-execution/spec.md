## ADDED Requirements
### Requirement: In-flight Change Cancellation
並列実行中にTUIから単体停止が要求された場合、対象changeの実行はキャンセルされなければならない（SHALL）。キャンセル完了後、当該changeは in-flight から除外され、queued が残っている場合は再分析が実行されなければならない（SHALL）。

#### Scenario: Cancel active change and re-analyze remaining queued
- **GIVEN** parallel execution is running with multiple queued changes
- **AND** one change is in-flight
- **WHEN** a stop request for the in-flight change is issued
- **THEN** the in-flight change is cancelled and removed from in-flight tracking
- **AND** analysis runs for remaining queued changes
- **AND** the remaining queued changes continue execution
