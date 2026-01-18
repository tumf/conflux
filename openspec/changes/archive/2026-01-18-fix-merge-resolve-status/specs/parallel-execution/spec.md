## MODIFIED Requirements
### Requirement: 衝突解決時のResolveStartedイベント送信

Parallel実行で `MergeWait` の change をユーザーが resolve した場合、resolve 完了後に TUI は `Merged` 状態を表示しなければならない（SHALL）。

#### Scenario: `MergeWait` からの resolve 完了後に Merged を表示する
- **GIVEN** TUI の変更が `MergeWait` である
- **AND** ユーザーが `M` キーで resolve を開始する
- **WHEN** resolve が正常に完了する
- **THEN** `ExecutionEvent::MergeCompleted { change_id, revision }` が TUI に送信される
- **AND** TUI は該当 change のステータスを `Merged` に設定する
