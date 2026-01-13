## 1. 実装

- [ ] 1.1 `src/tui/render.rs` で `UNCOMMITED` バッジの表示条件を `Archived` 状態から除外する
- [ ] 1.2 `Archived` 状態のチェックボックス表示が並列モードでも崩れないことを確認する

## 2. テスト

- [ ] 2.1 `src/tui/render.rs` に回帰テストを追加する（`Archived` 行に `UNCOMMITED` が出ないこと）
- [ ] 2.2 `UNCOMMITED` バッジが必要なケース（未コミット・キュー可能）では表示されることもテストする

## 3. 検証

- [ ] 3.1 `cargo test`
- [ ] 3.2 `cargo fmt`
