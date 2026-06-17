## ADDED Requirements

### Requirement: Blocked-only drain excludes pending resolve/reject waiters

内部的な `is_blocked_only_scheduler_state` チェックは blocked-only drain の判定時に executor-local の `resolve_wait_changes` および `reject_wait_changes` が空であることを確認しなければならない（MUST）。これらのセットが空でない場合、blocked-only 判定は `false` を返さなければならない（MUST）。

#### Scenario: resolve wait が存在する場合 blocked-only 判定は false

- **GIVEN** executor-local `resolve_wait_changes` に change `alpha` が存在する
- **AND** `alpha` に対する pending merge task は存在しない（`pending_merge_count == 0`）
- **AND** queued に `alpha` に依存する dependency-blocked な change `beta` が存在する
- **AND** in-flight workspace tasks、manual resolves は存在しない
- **WHEN** `is_blocked_only_scheduler_state` が評価される
- **THEN** `false` が返される
- **AND** スケジューラは終了せず、resolve の dispatch または完了を待つ

#### Scenario: reject wait が存在する場合 blocked-only 判定は false

- **GIVEN** executor-local `reject_wait_changes` に change `alpha` が存在する
- **AND** `alpha` に対する pending merge task は存在しない
- **AND** queued に `alpha` に依存する dependency-blocked な change `beta` が存在する
- **AND** in-flight workspace tasks、manual resolves は存在しない
- **WHEN** `is_blocked_only_scheduler_state` が評価される
- **THEN** `false` が返される

#### Scenario: resolve/reject wait が空で他の条件が blocked-only の場合 true

- **GIVEN** executor-local `resolve_wait_changes` と `reject_wait_changes` が空である
- **AND** in-flight workspace tasks、manual resolves、pending merge tasks が存在しない
- **AND** queued には manual `MergeWait` または dependency-blocked な change のみが存在する
- **WHEN** `is_blocked_only_scheduler_state` が評価される
- **THEN** `true` が返される（既存の blocked-only drain 動作を維持）
