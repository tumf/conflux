## MODIFIED Requirements

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、実行マーク可能な change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select/Stopped/Running モードで有効でなければならない（SHALL）。Stopping/Error では無効でなければならない（SHALL）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。

Running モードでは、bulk toggle は単一行の `Space` 操作と同じキュー変更規則を適用しなければならない（SHALL）。`NotQueued` の対象を全マークする場合は各行を動的キューへ追加し、`Queued` の対象を全アンマークする場合は各行を動的キューから削除しなければならない（SHALL）。

Running モードの bulk toggle は active な change を停止要求へ変換してはならない（MUST NOT）。`MergeWait` および `ResolveWait` の行については、実行マークのみを切り替え、動的キューの membership を変更してはならない（MUST NOT）。

#### Scenario: 未マークが残っている場合は全マークする
- **GIVEN** the TUI is in select mode
- **AND** at least one eligible change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all eligible changes SHALL be marked

#### Scenario: すべてマーク済みの場合は全アンマークする
- **GIVEN** the TUI is in stopped mode
- **AND** all eligible changes are marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all eligible changes SHALL be unmarked

#### Scenario: Running モードで一括マークがキュー追加になる
- **GIVEN** the TUI is in running mode
- **AND** at least one eligible change is `NotQueued` and unmarked
- **WHEN** the user triggers the bulk toggle
- **THEN** each affected `NotQueued` change SHALL be added to the dynamic queue
- **AND** each affected row SHALL become queued through the same reducer-visible command path used by single-row `Space`

#### Scenario: Running モードで一括アンマークがキュー削除になる
- **GIVEN** the TUI is in running mode
- **AND** all eligible queue-mutating targets are currently `Queued` and marked
- **WHEN** the user triggers the bulk toggle
- **THEN** each affected `Queued` change SHALL be removed from the dynamic queue
- **AND** no active change SHALL receive a stop request as a side effect of bulk toggle
