# Implementation Tasks

## 1. コアロジック実装

- [ ] 1.1 `src/parallel/mod.rs` のクリーンアップロジックを変更（エラー時はスキップ）
- [ ] 1.2 エラー発生時にworkspace名を含むログ出力を追加
- [ ] 1.3 復旧方法のヒントメッセージをINFOレベルで出力
- [ ] 1.4 `src/parallel/cleanup.rs` の `CleanupGuard` を変更（エラー時は保持）
- [ ] 1.5 `WorkspaceResult` にworkspace名を追加（ログ出力用）

## 2. イベント通知

- [ ] 2.1 `ParallelEvent` に `WorkspacePreserved` イベントを追加
- [ ] 2.2 TUIでworkspace保持イベントを表示

## 3. テスト

- [ ] 3.1 エラー発生時にworkspaceが保持されることを確認するテスト
- [ ] 3.2 成功時にworkspaceがクリーンアップされることを確認するテスト
- [ ] 3.3 エラーログにworkspace名が含まれることを確認するテスト

## 4. 検証

- [ ] 4.1 `cargo fmt` と `cargo clippy` を実行
- [ ] 4.2 `cargo test` で全テストがパスすることを確認
- [ ] 4.3 最大イテレーション到達時にworkspaceが保持されることを手動確認
- [ ] 4.4 `add-workspace-resume` と組み合わせて自動復旧されることを確認
