# Implementation Tasks

## 1. 失敗追跡の実装

- [x] 1.1 `src/parallel/types.rs` に `FailedChangeTracker` 構造体を追加
- [x] 1.2 `mark_failed` メソッドを実装（失敗した変更を記録）
- [x] 1.3 `should_skip` メソッドを実装（依存先が失敗しているかチェック）
- [x] 1.4 `set_dependencies` メソッドを実装（依存関係を設定）

## 2. 依存関係の抽出

- [x] 2.1 `src/analyzer.rs` に `extract_change_dependencies` 関数を追加
- [x] 2.2 グループベースの依存関係を変更レベルに変換するロジックを実装
- [x] 2.3 ユニットテストを追加

## 3. イベントの追加

- [x] 3.1 `src/parallel/events.rs` に `ChangeSkipped` イベントを追加
- [x] 3.2 `ChangeSkipped` イベントに `change_id` と `reason` フィールドを含める

## 4. ParallelExecutor の変更

- [x] 4.1 `ParallelExecutor` に `failed_tracker` フィールドを追加
- [x] 4.2 `execute_group` 内でスキップチェックを実装
- [x] 4.3 失敗した変更を `failed_tracker` に記録する処理を追加
- [x] 4.4 スキップされた変更のイベント発行を実装

## 5. 再分析モードの変更

- [x] 5.1 `execute_with_reanalysis` で失敗追跡を有効化
- [x] 5.2 スキップ対象の変更を除外して再分析するロジックを実装

## 6. TUI対応

- [x] 6.1 `src/tui/parallel_event_bridge.rs` で `ChangeSkipped` イベントを処理
- [x] 6.2 スキップされた変更をログペインに表示

## 7. テスト

- [x] 7.1 `FailedChangeTracker` のユニットテストを追加
- [x] 7.2 依存先失敗時にスキップされることを確認するテスト
- [x] 7.3 独立した変更が続行されることを確認するテスト
- [x] 7.4 再分析モードでの動作テスト

## 8. 検証

- [x] 8.1 `cargo fmt` と `cargo clippy` を実行
- [x] 8.2 `cargo test` で全テストがパスすることを確認
