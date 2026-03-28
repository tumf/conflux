## Implementation Tasks

- [ ] `src/parallel/merge.rs` で、手動 resolve 時の `MergeAttempt::Deferred(reason)` を dirty reason に応じて `auto_resumable=true/false` に分類する（verification: `resolve_merge_for_change` の deferred 分岐が `ResolveFailed` 一律ではなく分類送出になる）
- [ ] `src/parallel/merge.rs` で、自動再評価可能な deferred resolve は `MergeDeferred` イベントで返し、手動介入が必要な deferred resolve のみ `merge wait` 復帰にする（verification: deferred resolve の戻り値とイベント送出が意図どおり分岐する）
- [ ] `src/orchestration/state.rs` と `src/web/state.rs` の reducer で、自動再評価可能な deferred resolve が `resolve pending` を維持し、手動介入待ちのみ `merge wait` になることを確認・調整する（verification: 対応ユニットテストを追加/更新）
- [ ] `src/tui/state.rs` で、手動 resolve 起点の deferred が理由別に `ResolveWait` / `MergeWait` へ遷移することを確認・調整する（verification: TUI state テストで両ケースを追加/更新）
- [ ] `src/parallel/queue_state.rs` または関連経路で、先行 merge / resolve 完了後の再評価により auto-resumable deferred resolve が自動復帰することを回帰テストで固定する（verification: 自動リトライ経路のテスト追加/更新）
- [ ] `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings` を実行し、変更が既存フローを壊していないことを確認する（verification: コマンド成功）

## Future Work

- 実運用で dirty reason の粒度を増やしたくなった場合は、文字列ベース判定から enum ベース判定への置換を別 change として検討する
