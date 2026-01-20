## 1. Implementation
- [x] 1.1 依存待ちを示す queue status と表示語彙を追加する（確認: src/tui/types.rs と Web のステータス表示）
- [x] 1.2 依存関係でブロックされた change を状態更新イベントで通知する（確認: src/parallel/mod.rs と TUI/Web の state 更新）
- [x] 1.3 依存関係が解決されたら blocked を解除して queued に戻す（確認: analysis ループと queued 状態更新）
- [x] 1.4 TUI の描画と key hints を blocked 非アクティブ扱いに合わせる（確認: 表示とアクティブ判定）
- [x] 1.5 Web ダッシュボードのステータス語彙に blocked を追加する（確認: change 行の表示）
- [x] 1.6 blocked 状態の遷移テストを追加する（確認: cargo test もしくは該当テスト）

## 2. Validation
- [x] 2.1 cargo test を実行しステータス処理の挙動を確認する
