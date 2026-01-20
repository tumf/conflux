## 1. Implementation
- [x] 1.1 実行スロットが空いていない場合に re-analysis をスキップするガードを追加する（`src/parallel/mod.rs` の re-analysis 判定）。
      完了条件: `available_slots == 0` の場合は re-analysis 分岐に入らないことを確認できる。コード確認: `src/parallel/mod.rs`.
- [x] 1.2 スロットが空いたタイミングで re-analysis が再評価されるよう、既存の in-flight 完了トリガを維持する。
      完了条件: in-flight 完了で `needs_reanalysis` が再評価されるロジックが保持されている。コード確認: `src/parallel/mod.rs`.
- [x] 1.3 デバウンス挙動は維持しつつ、空きスロットなしの場合のログ/挙動が明確になるようログ文言を調整する（必要なら）。
      完了条件: 空きスロットなしで re-analysis を保留したことがログで判別できる。ログ確認: `src/parallel/mod.rs`.

## 2. Validation
- [x] 2.1 関連するユニット/挙動が変わるため、既存の parallel 実行のテストが通ることを確認する（必要なら追加）。
      完了条件: `cargo test` が成功する。実行コマンド: `cargo test`.
