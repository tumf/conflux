## 1. 実装
- [ ] 1.1 自動更新時の change 除外条件を整理する（「実体がない」判定と「apply開始済み」判定）
- [ ] 1.2 `AppState::update_changes` の retain 条件を変更し、未開始かつ実体なしのみを除外する
- [ ] 1.3 apply 開始のトラッキングが確実に入るようにイベントハンドリングを確認する（`ApplyStarted` 等）

## 2. テスト
- [ ] 2.1 refresh で未開始・実体なしの change が除外されるテストを追加/更新する
- [ ] 2.2 refresh で apply 開始済み・実体なしの change が保持されるテストを追加/更新する

## 3. 検証
- [ ] 3.1 `cargo test`
- [ ] 3.2 `cargo fmt --check`
- [ ] 3.3 `cargo clippy -- -D warnings`
