# Tasks: execution モジュールの基盤作成

## 1. モジュール構造の作成

- [ ] 1.1 `src/execution/mod.rs` を作成（モジュールルート）
- [ ] 1.2 `src/execution/types.rs` を作成（共通型定義）
- [ ] 1.3 `src/main.rs` に `mod execution;` を追加

## 2. 共通型の定義

- [ ] 2.1 `ExecutionContext` 構造体を定義
  - change_id, workspace_path（Option）, config への参照
  - hooks への参照（Option）
  - event_tx チャネル（Option）
- [ ] 2.2 `ExecutionResult` 列挙型を定義
  - Success, Failed, Cancelled 状態
- [ ] 2.3 `ProgressInfo` 構造体を定義
  - completed, total, percentage 計算

## 3. テストの作成

- [ ] 3.1 `types.rs` の基本的なユニットテストを作成
- [ ] 3.2 `ProgressInfo` の計算ロジックのテスト

## 4. 検証

- [ ] 4.1 `cargo build` が成功すること
- [ ] 4.2 `cargo test` が成功すること
- [ ] 4.3 `cargo clippy` が警告なしで通ること
