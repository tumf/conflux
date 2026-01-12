# Tasks: TUI Web UI QRコードポップアップ

## 実装タスク

### 1. 依存関係の追加
- [x] Cargo.tomlに `qrcode` crateを追加（`web-monitoring` フィーチャーに含める）
- [x] フィーチャーフラグの設定を確認

### 2. AppMode の拡張
- [x] `src/tui/types.rs` に `QrPopup` バリアントを追加
- [x] 関連するmatch文の更新（全ファイル）

### 3. AppState の拡張
- [x] `src/tui/state/mod.rs` に `web_url: Option<String>` フィールドを追加
- [x] `previous_mode` フィールドを活用してQrPopupからの復帰を実装

### 4. QRコード生成モジュール
- [x] `src/tui/qr.rs` を新規作成
- [x] `generate_qr_string(url: &str) -> Result<String, String>` 関数を実装
- [x] ユニットテストを追加

### 5. キーバインドの実装
- [x] `src/tui/runner.rs` に `w` キーのハンドリングを追加
- [x] QrPopupモードでの `Esc` / 任意キーでのモード復帰を実装
- [x] `web_url` が `None` の場合の処理を追加

### 6. ポップアップレンダリング
- [x] `src/tui/render.rs` に `render_qr_popup()` 関数を追加
- [x] 中央配置のポップアップレイアウトを実装
- [x] QRコードとURL文字列の両方を表示

### 7. キーヒントの更新
- [x] Select/Running/Stoppedモードのキーヒントに `w: QR` を追加（Webサーバー有効時のみ）
- [x] QrPopupモードのキーヒント（`press any key to close`）を追加

### 8. Web URLの設定
- [x] TUI起動時にWebサーバーのURLを `AppState.web_url` に設定
- [x] `--web` フラグと `--web-port`, `--web-bind` の値からURLを構築

### 9. テスト
- [x] QRコード生成のユニットテスト
- [x] モード遷移のユニットテスト
- [x] キーバインドの統合テスト

### 10. ドキュメント
- [x] README.mdにQRコード機能の説明を追加
- [x] キーバインド一覧の更新

## 検証項目

- [x] `cargo fmt --check` が成功する
- [x] `cargo clippy -- -D warnings` が成功する
- [x] `cargo test` が成功する
- [x] Webサーバーが無効な場合、`w` キーが無視される（実装済み）
