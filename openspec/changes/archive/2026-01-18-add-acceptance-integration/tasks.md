## 1. Implementation
- [x] 1.1 逐次実行ループに acceptance を統合し、apply 成功後に必ず acceptance が呼ばれることを確認する（`src/orchestrator.rs` の実行経路）
- [x] 1.2 acceptance の結果分岐を実装し、PASS なら archive、FAIL なら apply に戻る動作を確認する（逐次）
- [x] 1.3 並列実行ループに acceptance を統合し、apply 成功後に必ず acceptance が呼ばれることを確認する（`src/parallel/*` の実行経路）
- [x] 1.4 acceptance の結果分岐を実装し、PASS なら archive、FAIL なら apply に戻る動作を確認する（並列）
- [x] 1.5 acceptance 成功時に acceptance 履歴がクリアされることを確認する（履歴の参照経路を含む）
- [x] 1.6 acceptance 失敗/コマンド失敗のログ・イベント・状態遷移が明確に記録されることを確認する
- [x] 1.7 必要な単体テストを追加する（分岐/履歴/失敗パス）
- [x] 1.8 `cargo test` を実行して結果を確認する
