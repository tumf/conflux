## 1. REST APIの実装
- [x] 1.1 `api.rs`に`approve_change`ハンドラーを追加
- [x] 1.2 `api.rs`に`unapprove_change`ハンドラーを追加
- [x] 1.3 `mod.rs`にPOSTルート `/api/changes/{id}/approve` を追加
- [x] 1.4 `mod.rs`にPOSTルート `/api/changes/{id}/unapprove` を追加

## 2. WebState連携
- [x] 2.1 `WebState`に承認操作用メソッドを追加
- [x] 2.2 承認状態変更時のWebSocket通知を実装

## 3. フロントエンドUI
- [x] 3.1 変更カードに承認ボタンを追加（`app.js`）
- [x] 3.2 承認ボタンのクリックハンドラーを実装
- [x] 3.3 承認ボタンのスタイリング（`style.css`）
- [x] 3.4 タッチデバイス向けの最小タップターゲットサイズを確保

## 4. テスト
- [x] 4.1 APIエンドポイントの単体テストを追加
- [x] 4.2 WebSocket通知のテストを追加

## 5. ドキュメント
- [x] 5.1 新しいAPIエンドポイントをコードコメントに記載
