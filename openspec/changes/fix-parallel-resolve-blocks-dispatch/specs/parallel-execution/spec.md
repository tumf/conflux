## MODIFIED Requirements

### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

merge/resolve の結果（成功・Deferred・失敗）はスケジューラループに非同期に通知され、適切に処理されなければならない（MUST）。

#### Scenario: Queued change dispatched during resolve

- **GIVEN** Change A のコンフリクト解決（resolve）が進行中で、queued に Change B が存在し、利用可能スロットが 1 以上ある
- **WHEN** スケジューラループの次の iteration が実行される
- **THEN** Change B の re-analysis と dispatch が実行される
- **AND** Change A の resolve は並行して継続する

#### Scenario: Merge result delivered after background completion

- **GIVEN** Change A の merge がバックグラウンドタスクで実行中
- **WHEN** merge が成功する
- **THEN** merge 結果がスケジューラループに通知される
- **AND** `retry_deferred_merges` が呼び出され、ResolveWait の change がリトライされる

#### Scenario: Merge deferred delivered after background attempt

- **GIVEN** Change A の merge がバックグラウンドで試行される
- **WHEN** merge が Deferred（resolve 進行中 or base dirty）となる
- **THEN** Deferred イベントがスケジューラループに通知される
- **AND** Change A は resolve_wait_changes または merge_wait_changes に追加される
