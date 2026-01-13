## 1. Implementation
- [x] 1.1 逐次applyループに反復ごとのWIPコミット作成を追加する
- [x] 1.2 parallel実行のWIPコミット作成を新規コミット方式に変更する
- [x] 1.3 apply成功時のWIP squash処理をGitの`reset --soft`ベースに統一する
- [x] 1.4 apply失敗時もWIPコミットを作成するようにする

## 2. Tests
- [x] 2.1 逐次実行のWIPコミット作成に関するテストを追加/更新する
- [x] 2.2 parallel実行のWIPコミット挙動（新規コミット/失敗時/allow-empty）を更新する

## 3. Validation
- [x] 3.1 `cargo test`
- [x] 3.2 `cargo clippy -- -D warnings`
