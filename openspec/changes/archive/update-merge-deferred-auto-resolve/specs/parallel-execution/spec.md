## MODIFIED Requirements

### Requirement: Parallel Execution Event Reporting

order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。
MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、待機状態の表示は TUI 仕様に従って `MergeWait` または `ResolveWait` を判定しなければならない（SHALL）。

さらに、`MergeDeferred` のうち先行 merge / resolve の完了で再評価可能な change は、自動再評価対象として保持されなければならない（MUST）。
先行 merge または resolve が完了したとき、システムは自動再評価対象の change を再評価し、競合が残る場合は `ResolveWait` または `Resolving` に進め、merge 再試行可能な場合は `MergeWait` に留めてはならない（MUST）。
手動介入が必要な change のみが `MergeWait` に留まらなければならない（MUST）。

#### Scenario: 先行 merge 完了後に deferred change が自動再評価される
- **GIVEN** change B が `MergeDeferred` となっている
- **AND** その理由は先行している change A の merge / resolve 完了待ちである
- **WHEN** change A の merge または resolve が完了する
- **THEN** システムは change B を自動再評価する
- **AND** change B は `MergeWait` のまま放置されない

#### Scenario: 自動再評価後に競合が残る change は resolve 待機へ進む
- **GIVEN** change B が先行 merge 完了待ちの `MergeDeferred` として保持されている
- **WHEN** 再評価時点でも change B に解消すべき競合が残っている
- **THEN** change B は `ResolveWait` または `Resolving` に進む
- **AND** 手動 `M` を押さなくても次の解決フローに乗る

#### Scenario: 手動介入が必要な deferred change だけが MergeWait に残る
- **GIVEN** change B が `MergeDeferred` となっている
- **AND** システムが競合原因を再評価しても自動再開条件を満たさない
- **WHEN** 待機状態が更新される
- **THEN** change B は `MergeWait` に留まる
- **AND** TUI は手動 resolve 対象として表示する
