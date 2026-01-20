## 1. Implementation
- [x] 1.1 parallel 内の責務（workspace/queue/merge/state）を整理し、分割対象と移動方針を文書化する (verify: proposal と design に対象モジュール一覧が明記されている)。
- [x] 1.2 並列実行の主要ロジックを専用サブモジュールに移動し、公開 API を mod.rs で再公開する (verify: src/parallel/mod.rs が再公開中心になっている)。
- [x] 1.3 動的キュー・マージ・ワークスペース管理のロジックを分割モジュールに移動する (verify: 新規ファイル src/parallel/workspace.rs, dynamic_queue.rs, merge.rs に関数が配置されている)。
- [x] 1.4 parallel のテストを tests サブモジュールへ移動し、対象別に分割する (verify: src/parallel/tests/* が追加され、cargo test の対象が移動している)。
- [x] 1.5 cargo fmt / cargo clippy -- -D warnings / cargo test を実行し、既存挙動が維持されていることを確認する。


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) テスト分割完了: `src/parallel/mod.rs` の #[cfg(test)] ブロック (1790行) を `src/parallel/tests/executor.rs` に移動し、`tests/mod.rs` でモジュールとしてインクルード (verify: `mod.rs` から test ブロックが削除され、`tests/executor.rs` に全テストが存在)
  2) ワークスペース分割の実統合完了: `workspace::get_or_create_workspace` を `mod.rs` の `dispatch_change_to_workspace` メソッド内で呼び出すように変更し、`mod.rs` の重複実装を削除。未使用の `workspace::dispatch_change_to_workspace` 関数を削除 (verify: `workspace.rs` に `#[allow(dead_code)]` が存在せず、`get_or_create_workspace` が実行パスで使用されている)


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) 個別モジュールのテスト追加完了: `src/parallel/tests/conflict.rs` を新規作成し、conflict モジュールの主要機能をテスト (verify: `cargo test parallel::tests::conflict` で6つのテストが実行される)
  2) テストモジュール構成: `src/parallel/tests/mod.rs` に conflict モジュールを追加し、executor と conflict の両方が個別にテスト可能になった (verify: `src/parallel/tests/mod.rs` に `mod conflict;` が存在し、`src/parallel/mod.rs` に `mod tests;` が宣言されている)
  3) 品質保証: cargo test (全830テスト成功)、cargo fmt --check、cargo clippy -- -D warnings を実行し、すべて成功
