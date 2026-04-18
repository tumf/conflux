## Implementation Tasks

- [ ] 1. resolve 完了判定の現行経路を整理し、fast-forward 成功を `base に統合済み` として受理する要件を設計に反映する (verification: manual - `src/parallel/conflict.rs` と既存 resolve 検証経路の差分レビュー)
- [ ] 2. `src/parallel/conflict.rs` の resolve 後検証を修正し、merge commit 不在だけでは失敗にせず fast-forward 統合済みを成功扱いにする (verification: integration - fast-forward 成功時の resolve テスト追加)
- [ ] 3. `Missing merge commits for change_ids` を返す条件を、fast-forward 成功ケースを除外した未完了ケースに限定する (verification: unit - resolve 継続理由の判定テスト追加)
- [ ] 4. `src/tui/runner.rs` / reducer reconciliation を修正し、merged 済み change を `merge wait` 復元対象に含めない (verification: unit - `ChangesRefreshed` 後に merged が維持されるテスト追加)
- [ ] 5. merged / merge_wait / archived の整合性を `src/orchestration/state.rs` か関連 reducer テストで固定する (verification: unit - terminal merged が observation で退行しないテスト追加)
- [ ] 6. 関連ログ・resolve context を見直し、fast-forward 成功ケースで誤った `Missing merge commits` 再試行理由が出ないことを確認する (verification: integration - resolve retry 判定ログ/文脈テストまたは既存テスト拡張)

## Verification Tasks

- [ ] 7. `cflx openspec validate fix-fast-forward-resolve-regression --strict` を通す (verification: manual - strict validate)
- [ ] 8. 影響範囲の Rust テストを実行する (verification: integration - `cargo test` の対象テスト群)

## Future Work

- 実リポジトリの long-running orchestration で、既存 runlog を用いた end-to-end 再現確認
