# Tasks: TUI Web UI QRコードポップアップ

## 実装タスク

### 1. 依存関係の追加
- [x] Cargo.tomlに `qrcode` crateを追加（`web-monitoring` フィーチャーに含める）
- [x] Cargo.tomlに `local-ip-address` crateを追加（ローカルIP取得用）
- [x] フィーチャーフラグの設定を確認

### 2. AppMode の拡張
- [x] `src/tui/types.rs` に `QrPopup` バリアントを追加
- [x] 関連するmatch文の更新（全ファイル）

### 3. AppState の拡張
- [x] `src/tui/state/mod.rs` に `web_url: Option<String>` フィールドを追加
- [x] `previous_mode` フィールドを活用してQrPopupからの復帰を実装

### 4. URL変換モジュール
- [x] `src/web/url.rs` または該当モジュールにURL変換ロジックを追加
- [x] `build_access_url(bind_addr, port)` 関数を実装
- [x] `get_local_ip()` 関数を実装（`local-ip-address` crate使用）
- [x] ユニットテストを追加

### 5. QRコード生成モジュール
- [x] `src/tui/qr.rs` を新規作成
- [x] `generate_qr_string(url: &str) -> Result<String, String>` 関数を実装
- [x] ユニットテストを追加

### 6. キーバインドの実装
- [x] `src/tui/runner.rs` に `w` キーのハンドリングを追加
- [x] QrPopupモードでの `Esc` / 任意キーでのモード復帰を実装
- [x] `web_url` が `None` の場合の処理を追加

### 7. ポップアップレンダリング
- [x] `src/tui/render.rs` に `render_qr_popup()` 関数を追加
- [x] 中央配置のポップアップレイアウトを実装
- [x] QRコードとURL文字列の両方を表示

### 8. キーヒントの更新
- [x] Select/Running/Stoppedモードのキーヒントに `w: QR` を追加（Webサーバー有効時のみ）
- [x] QrPopupモードのキーヒント（`Esc: close`）を追加

### 9. Web URLの設定
- [x] TUI起動時にWebサーバーのURLを `AppState.web_url` に設定
- [x] `--web` フラグと `--web-port`, `--web-bind` の値からURLを構築
- [x] `0.0.0.0` バインド時はローカルIPに変換

### 10. テスト
- [x] QRコード生成のユニットテスト
- [x] URL変換ロジックのユニットテスト
- [x] モード遷移のユニットテスト
- [x] キーバインドの統合テスト

### 11. ドキュメント
- [x] README.mdにQRコード機能の説明を追加
- [x] キーバインド一覧の更新

## 検証項目

- [x] `cargo fmt --check` が成功する
- [x] `cargo clippy -- -D warnings` が成功する
- [x] `cargo test` が成功する
- [x] TUIで `w` キーを押すとQRコードポップアップが表示される
- [x] QRコードをスマートフォンでスキャンしてアクセスできる
- [x] `Esc` キーでポップアップが閉じる
- [x] Webサーバーが無効な場合、`w` キーが無視される（またはエラー表示）
- [x] `0.0.0.0` バインド時にローカルIPアドレスがURLに使用される
