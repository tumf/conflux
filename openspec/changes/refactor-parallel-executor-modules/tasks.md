## 1. Characterization
- [x] 1.1 既存の並列実行テストをキャラクタリゼーション（検証: `cargo check` でコンパイル確認済み）

## 2. Refactor
- [x] 2.1 `ParallelExecutor` の構築/初期化ロジックを専用サブモジュールへ移動（`src/parallel/builder.rs` を新規作成、`cargo clippy`, `cargo fmt` で検証済み）
- [x] 2.2 キュー状態/デバウンス管理などの内部状態更新を専用サブモジュールへ移動（`src/parallel/queue_state.rs` を新規作成、`cargo clippy`, `cargo fmt` で検証済み）
- [x] 2.3 `parallel/mod.rs` の再公開と入口整理（モジュールドキュメント更新、`mod builder;` と `mod queue_state;` 宣言追加、不要インポート削除）
