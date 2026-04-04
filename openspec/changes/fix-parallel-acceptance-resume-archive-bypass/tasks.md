## Implementation Tasks

- [x] 1. parallel resume / archive 遷移の現状を整理し、acceptance state の durable 保存先と更新契機を設計する (verification: `src/parallel/dispatch.rs`, `src/parallel/executor.rs`, `src/execution/state.rs` に対応する実装箇所が定まり proposal/design に反映されている)
- [x] 2. acceptance state を workspace に保存・読取する仕組みを追加する (verification: acceptance 開始・PASS・FAIL/中断時の state 更新を単体テストで確認できる)
- [x] 3. resume routing を更新し、archive 未完了 workspace は durable acceptance-pass state がない限り acceptance に戻す (verification: resumed `Applied` / `Archiving` workspace のルーティングをテストで確認できる)
- [x] 4. archive 開始直前に acceptance-pass guard を追加し、verdict 不在・中断・失敗 state では archive command を起動しない (verification: archive 起動抑止の回帰テストで `archive_command` 未実行を確認できる)
- [x] 5. TUI / tracing ログに archive 抑止理由と acceptance 再実行理由を出す (verification: 対応するイベント/ログ出力のテストまたは既存ログ検証テストが追加される)
- [x] 6. 再現ケースに対応する parallel resume 回帰テストを追加する (verification: acceptance 開始後に中断した workspace を再開すると archive ではなく acceptance に戻ることを `cargo test` で確認できる)
- [x] 7. 実装後に品質ゲートを実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- 実運用ログで同種の中断ケースが再発しないことの継続監視
