## MODIFIED Requirements

### Requirement: resolve-merge-reducer-sync

When a user triggers merge resolve (`M` key) on a `MergeWait` change, the shared orchestration reducer MUST be updated with `ResolveMerge` intent regardless of whether resolve executes immediately or is queued.

モジュール分割後も、resolve 処理のイベントハンドラは `state/event_handlers/completion.rs` に配置され、既存の挙動を維持しなければならない (SHALL)。

#### Scenario: リファクタリング後も resolve-merge 動作が維持される

- **GIVEN** TUI イベントハンドラが `state/event_handlers/` に分割済みである
- **WHEN** change が `MergeWait` で `M` キーを押下する
- **THEN** 分割前と同一の reducer 更新と ResolveWait 遷移が行われる
