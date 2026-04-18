## Implementation Tasks

- [ ] 1. `src/parallel/merge.rs` の archive 後 merge 完了検証経路を整理し、fast-forward 統合済みを success とみなす条件を明文化する (verification: manual - `verify_merge_commits()` と caller のレビュー)
- [ ] 2. `verify_merge_commits()` を修正し、merge commit message 不在でも change が base に統合済みなら成功扱いにする (verification: unit - fast-forward 検証テスト追加)
- [ ] 3. `Missing merge commit message containing change_id(s)` を返す条件を、未統合の change に限定する (verification: unit - 未統合ケースでは従来どおり失敗するテスト追加)
- [ ] 4. `src/vcs/git/commands/merge.rs` の補助関数や既存テストを見直し、parallel merge 経路でも fast-forward 統合を判定できるようにする (verification: unit - git command helper テスト更新)
- [ ] 5. archive 後 parallel merge の fast-forward 成功ケースを再現する回帰テストを `src/parallel/tests/*` に追加する (verification: integration - 対象 Rust テスト追加)
- [ ] 6. 関連 spec を更新し、parallel merge の最終検証が fast-forward 統合済み change を error にしないことを定義する (verification: manual - spec delta review)

## Verification Tasks

- [ ] 7. `cflx openspec validate fix-parallel-merge-fast-forward-verification --strict` を通す (verification: manual - strict validate)
- [ ] 8. 影響範囲の Rust テストを実行する (verification: integration - `cargo test` の対象テスト群)

## Future Work

- 実 runlog を用いた long-running parallel orchestration の end-to-end 検証
