## 1. Implementation

- [ ] 1.1 `toggle_selection` で Running/Stopped モード時にも `is_new = false` を設定
- [ ] 1.2 `update_approval_status` で承認時に `is_new = false` を設定
- [ ] 1.3 `new_change_count` のデクリメント処理を両メソッドに追加

## 2. Testing

- [ ] 2.1 キュー追加時に NEW バッジが消えることをテスト
- [ ] 2.2 承認時に NEW バッジが消えることをテスト
- [ ] 2.3 既存のテスト（Select モードでの消去）が引き続きパスすることを確認

## 3. Validation

- [ ] 3.1 `cargo test` で全テストパス
- [ ] 3.2 `cargo clippy` でエラーなし
- [ ] 3.3 TUI で手動確認：承認時、キュー追加時に NEW が消えることを確認
