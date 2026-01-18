## MODIFIED Requirements

### Requirement: Event-Driven State Updates
TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。

#### Scenario: Merged/Archived完了時にアーカイブ進捗を再取得する
- **GIVEN** 変更がアーカイブ済みで tasks.md が `openspec/changes/archive/{date}-{change_id}/tasks.md` に存在する
- **WHEN** TUI が `Merged` または `Archived` の完了イベントを受信する
- **THEN** TUI はアーカイブ内の tasks.md から `completed/total` を再取得して表示する
