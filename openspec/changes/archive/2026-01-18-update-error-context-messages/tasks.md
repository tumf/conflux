## 1. 仕様更新
- [x] 1.1 observability の要件を更新し、エラー文言の文脈情報要件を追加する
- [x] 1.2 変更内容に対応するシナリオを追加する

## 2. 実装
- [x] 2.1 キャンセル・失敗メッセージの固定文字列を文脈付きに置換する
- [x] 2.2 stdout/stderr 取得失敗など内部エラーにコマンドと cwd を付与する
- [x] 2.3 TUI/Parallel のイベントログとエラーメッセージの整合を取る

## 3. 検証
- [x] 3.1 cargo fmt
- [x] 3.2 cargo clippy -- -D warnings
- [x] 3.3 cargo test
