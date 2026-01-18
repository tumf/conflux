## 1. Implementation
- [ ] 1.1 config に acceptance_command / acceptance_prompt を追加し、デフォルト値・テンプレート更新を行う
- [ ] 1.2 acceptance 出力フォーマット（合否/指摘事項）と解析関数を追加する
- [ ] 1.3 逐次実行の apply→acceptance→archive ループを実装する
- [ ] 1.4 並列実行の apply→acceptance→archive ループを実装する
- [ ] 1.5 acceptance 結果を apply 履歴コンテキストへ反映する
- [ ] 1.6 acceptance 失敗時のログ/イベント/状態遷移を追加する
- [ ] 1.7 単体テストを追加する（解析、履歴、分岐）
- [ ] 1.8 `cargo test` を実行し結果を確認する
