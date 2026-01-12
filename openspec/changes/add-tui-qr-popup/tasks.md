# Tasks: TUI Web UI QRコードポップアップ

## 実装タスク

### 1. 依存関係の追加
- [ ] Cargo.tomlに `qrcode` crateを追加（`web-monitoring` フィーチャーに含める）
- [ ] フィーチャーフラグの設定を確認

### 2. AppMode の拡張
- [ ] `src/tui/types.rs` に `QrPopup` バリアントを追加
- [ ] 関連するmatch文の更新（全ファイル）

### 3. AppState の拡張
- [ ] `src/tui/state/mod.rs` に `web_url: Option<String>` フィールドを追加
- [ ] `previous_mode` フィールドを活用してQrPopupからの復帰を実装

### 4. QRコード生成モジュール
- [ ] `src/tui/qr.rs` を新規作成
- [ ] `generate_qr_string(url: &str) -> Result<String, String>` 関数を実装
- [ ] ユニットテストを追加

### 5. キーバインドの実装
- [ ] `src/tui/runner.rs` に `w` キーのハンドリングを追加
- [ ] QrPopupモードでの `Esc` / 任意キーでのモード復帰を実装
- [ ] `web_url` が `None` の場合の処理を追加

### 6. ポップアップレンダリング
- [ ] `src/tui/render.rs` に `render_qr_popup()` 関数を追加
- [ ] 中央配置のポップアップレイアウトを実装
- [ ] QRコードとURL文字列の両方を表示

### 7. キーヒントの更新
- [ ] Select/Running/Stoppedモードのキーヒントに `w: QR` を追加（Webサーバー有効時のみ）
- [ ] QrPopupモードのキーヒント（`Esc: close`）を追加

### 8. Web URLの設定
- [ ] TUI起動時にWebサーバーのURLを `AppState.web_url` に設定
- [ ] `--web` フラグと `--web-port`, `--web-bind` の値からURLを構築

### 9. テスト
- [ ] QRコード生成のユニットテスト
- [ ] モード遷移のユニットテスト
- [ ] キーバインドの統合テスト

### 10. ドキュメント
- [ ] README.mdにQRコード機能の説明を追加
- [ ] キーバインド一覧の更新

## 検証項目

- [ ] `cargo fmt --check` が成功する
- [ ] `cargo clippy -- -D warnings` が成功する
- [ ] `cargo test` が成功する
- [ ] TUIで `w` キーを押すとQRコードポップアップが表示される
- [ ] QRコードをスマートフォンでスキャンしてアクセスできる
- [ ] `Esc` キーでポップアップが閉じる
- [ ] Webサーバーが無効な場合、`w` キーが無視される（またはエラー表示）
