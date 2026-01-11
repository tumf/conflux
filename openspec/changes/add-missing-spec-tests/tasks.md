## 1. 仕様シナリオの分析とマッピング更新

- [ ] 1.1 hooks spec の全シナリオをリストアップし、既存テストとマッピング
- [ ] 1.2 parallel-execution spec のシナリオをリストアップし、既存テストとマッピング
- [ ] 1.3 tui-editor spec のシナリオをリストアップし、既存テストとマッピング
- [ ] 1.4 workspace-cleanup spec のシナリオをリストアップし、既存テストとマッピング
- [ ] 1.5 tui-key-hints spec のシナリオをリストアップし、既存テストとマッピング
- [ ] 1.6 tui-architecture spec のシナリオをリストアップし、既存テストとマッピング
- [ ] 1.7 docs/test-coverage-mapping.md を全仕様カバーに更新

## 2. hooks spec のテスト追加

- [ ] 2.1 on_queue_add フック実行のテスト
- [ ] 2.2 on_queue_remove フック実行のテスト
- [ ] 2.3 on_approve フック実行のテスト（コンテキスト変数含む）
- [ ] 2.4 on_unapprove フック実行のテスト
- [ ] 2.5 on_change_start フック実行のテスト（change_id 変数含む）
- [ ] 2.6 on_change_end フック実行のテスト
- [ ] 2.7 フック実行順序のテスト（on_change_start → pre_apply → post_apply → ...）
- [ ] 2.8 TUI/CLI フック実行パリティのテスト

## 3. configuration spec の不足テスト追加

- [ ] 3.1 max_iterations 設定のテスト（デフォルト50、0で無制限）
- [ ] 3.2 iteration_limit 終了ステータスのテスト
- [ ] 3.3 VCS backend 設定のテスト（auto/jj/git）
- [ ] 3.4 approved ファイルフォーマットのテスト
- [ ] 3.5 approval 検証ロジックのテスト（tasks.md 除外）
- [ ] 3.6 apply_prompt と hardcoded system prompt の結合テスト

## 4. parallel-execution spec のテスト追加

- [ ] 4.1 VCS バックエンド自動検出のテスト（jj 優先）
- [ ] 4.2 Git worktree 作成・削除のテスト
- [ ] 4.3 jj workspace 作成・削除のテスト
- [ ] 4.4 uncommitted changes エラー検出のテスト
- [ ] 4.5 並列グループ依存関係解析のテスト
- [ ] 4.6 max_concurrent_workspaces 制限のテスト

## 5. workspace-cleanup spec のテスト追加

- [ ] 5.1 正常完了時のワークスペースクリーンアップテスト
- [ ] 5.2 エラー時のワークスペース保持テスト
- [ ] 5.3 ブランチ削除のテスト

## 6. tui-editor spec のテスト追加

- [ ] 6.1 エディタコマンド検出のテスト（EDITOR/VISUAL 環境変数）
- [ ] 6.2 change ディレクトリでのエディタ起動テスト
- [ ] 6.3 エディタ終了後の状態維持テスト

## 7. tui-key-hints spec のテスト追加

- [ ] 7.1 モード別キーヒント表示のテスト（Select/Running/Stopped/Error）
- [ ] 7.2 パラレルモードトグル表示のテスト

## 8. approval spec のテスト追加

- [ ] 8.1 MD5 チェックサム生成のテスト
- [ ] 8.2 tasks.md 除外検証のテスト
- [ ] 8.3 ファイル追加/削除/変更による承認無効化テスト
- [ ] 8.4 ネストディレクトリ内ファイルの処理テスト

## 9. 検証

- [ ] 9.1 cargo test で全テスト実行
- [ ] 9.2 cargo llvm-cov でカバレッジ確認
- [ ] 9.3 docs/test-coverage-mapping.md の最終更新
