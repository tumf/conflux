## 1. Characterization
- [x] 1.1 既存の並列実行テストをキャラクタリゼーション（検証: `cargo check` でコンパイル確認済み）

## 2. Refactor
- [x] 2.1 `ParallelExecutor` の構築/初期化ロジックを専用サブモジュールへ移動（`src/parallel/builder.rs` を新規作成、`cargo clippy`, `cargo fmt` で検証済み）
- [x] 2.2 キュー状態/デバウンス管理などの内部状態更新を専用サブモジュールへ移動（`src/parallel/queue_state.rs` を新規作成、`cargo clippy`, `cargo fmt` で検証済み）
- [x] 2.3 `parallel/mod.rs` の再公開と入口整理（モジュールドキュメント更新、`mod builder;` と `mod queue_state;` 宣言追加、不要インポート削除）

## 3. Follow-up（Acceptance #1 失敗対応）
- [x] 3.1 テストコンパイルエラー修正: `src/parallel/tests/executor.rs` に `use crate::vcs::Workspace;` を追加してコンパイル回復（`cargo check --tests` で確認済み）
- [x] 3.2 `mod.rs` の残存詳細実装をサブモジュールへ移動:
  - `handle_merge_and_cleanup` / マージ関連メソッド群 → `src/parallel/merge.rs`（新規サブモジュール）に移動済み
  - `execute_with_order_based_reanalysis` → `src/parallel/orchestration.rs`（新規サブモジュール）に移動済み
  - `dispatch_change_to_workspace`（apply+acceptance+archive パイプライン）→ `src/parallel/dispatch.rs`（新規サブモジュール）に移動
  - `execute_changes_dispatch`, `execute_apply_and_archive_parallel`（dead_code）→ `mod.rs` から削除
  - `mod.rs` を 134 行の入口ファイルに整理（モジュール宣言・struct 定義・グローバルロックのみ）
- [x] 3.3 全検証完了（`cargo test`: 924 テスト全成功、`cargo clippy -- -D warnings`: 警告ゼロ、`cargo fmt --check`: 差分なし）
