## 1. 実装
- [x] 1.1 キャンセル／イテレーション制御の判定をヘルパー関数に抽出する
  - 検証: `src/orchestrator.rs` の `run` がヘルパー経由で判定していることを確認する
- [x] 1.2 `ChangeProcessResult` の分岐処理をヘルパー関数に抽出する
  - 検証: `run` の match 本体がヘルパー呼び出しに置き換わっていることを確認する
- [x] 1.3 重複する状態更新（共有状態／Web更新）を共通化する
  - 検証: 重複していた更新処理が一箇所に集約されていることを確認する
- [x] 1.4 リファクタリング後の挙動が維持されることを検証する
  - 検証: `cargo fmt && cargo clippy -- -D warnings && cargo test --bin cflx orchestrator::`
