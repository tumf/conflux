## Implementation Tasks

- [x] 1. resumed workspace routing で tasks completion を評価する現行経路を特定し、implementation task incomplete を Apply 優先 gate とする設計を `src/parallel/dispatch.rs` / `src/parallel/executor.rs` に対応づける (verification: 対象 routing 箇所が proposal/design に反映されている)
- [x] 2. unchecked implementation task が残る resumed implementation workspace を Apply に戻すよう routing を更新する (verification: incomplete tasks の resume が Apply を選ぶ回帰テストが追加される)
- [x] 3. completed tasks の resumed workspace では既存の Acceptance > Archive routing が維持されることを確認する (verification: completed tasks の resume routing 回帰テストが追加される)
- [x] 4. tasks incomplete による Apply 再ルーティング理由をログまたはイベントで観測可能にする (verification: 対応ログ/イベントの検証が追加される)
- [x] 5. quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- spec-only changes や Future Work checkbox の resume policy を必要に応じて別 change で整理する
