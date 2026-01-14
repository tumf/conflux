## 1. 実装

- [x] 1.1 `src/tui/render.rs` で `UNCOMMITED` バッジの表示条件を `Archived` 状態から除外する
- [x] 1.2 `Archived` 状態のチェックボックス表示が並列モードでも崩れないことを確認する

## 2. テスト

- [x] 2.1 `src/tui/render.rs` に回帰テストを追加する（`Archived` 行に `UNCOMMITED` が出ないこと）
- [x] 2.2 `UNCOMMITED` バッジが必要なケース（未コミット・キュー可能）では表示されることもテストする

## 3. 検証

- [x] 3.1 `cargo test`
- [x] 3.2 `cargo fmt`
