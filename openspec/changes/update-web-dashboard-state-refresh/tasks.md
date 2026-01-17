## 1. Implementation
- [ ] 1.1 WebStateに最新の変更一覧を読み込む更新関数を追加する（作業ツリー優先でタスク進捗を集計）
- [ ] 1.2 REST APIでレスポンス前にWebStateをリフレッシュする（/api/state, /api/changes, /api/changes/{id}）
- [ ] 1.3 WebSocket初期送信前に最新状態を反映する
- [ ] 1.4 Webサーバ起動時に定期更新タスクを追加する（過剰I/Oを避ける）

## 2. Validation
- [ ] 2.1 cargo test
- [ ] 2.2 cargo clippy
- [ ] 2.3 cargo fmt --check
