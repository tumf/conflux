## Implementation Tasks

- [x] 1. `src/tui/state.rs` の `toggle_all_marks` を拡張し、Running モードでも一括トグル可能にする（verification: Running モード + `is_resolving=true` のユニットテストで selected が期待通り切り替わる）
- [x] 2. 一括トグル時の対象除外ルールを実装する（active 除外、`MergeWait/ResolveWait` は selected のみ変更）（verification: queue_status と DynamicQueue の不変条件テストを追加）
- [x] 3. `src/tui/render.rs` のキーヒント表示条件を更新し、Running モードでも対象がある場合は `x: toggle all` を表示できるようにする（verification: Running で表示/非表示の両ケースをテスト）
- [x] 4. `src/tui/key_handlers.rs` の `x` キー処理が Running モードでも新ルールに沿って `toggle_all_marks` を適用できることを確認する（verification: キー入力経由のテストまたは state 層テストで担保）
- [x] 5. 回帰テストを追加/更新し、既存の Space 個別操作および resolve 操作を壊さないことを確認する（verification: `cargo test` で関連テストが通る）

## Future Work

- TUI 実運用での操作性評価に基づく、より細かな一括対象ルール（例: queued のみ対象）の検討
