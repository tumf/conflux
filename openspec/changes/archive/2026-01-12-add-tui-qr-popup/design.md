# Design: TUI Web UI QRコードポップアップ

## アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────────┐
│                      TUI Runner                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  AppState                            │   │
│  │  ├── mode: AppMode (Select, Running, QrPopup, etc.) │   │
│  │  ├── web_url: Option<String>                         │   │
│  │  └── qr_visible: bool                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Render Layer                            │   │
│  │  ├── render_main_content()                           │   │
│  │  └── render_qr_popup_overlay()  ← 新規追加           │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## コンポーネント設計

### 1. AppMode の拡張

```rust
pub enum AppMode {
    Select,
    Running,
    Stopped,
    Error,
    Proposing,
    QrPopup,  // 新規追加
}
```

### 2. AppState の拡張

```rust
pub struct AppState {
    // 既存フィールド...
    
    /// Web UIのURL（Webサーバーが有効な場合に設定）
    pub web_url: Option<String>,
}
```

### 3. QRコード生成

`qrcode` crateを使用してASCII形式のQRコードを生成:

```rust
use qrcode::QrCode;
use qrcode::render::unicode;

fn generate_qr_string(url: &str) -> Result<String, QrCodeError> {
    let code = QrCode::new(url)?;
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();
    Ok(image)
}
```

### 4. ポップアップレンダリング

Ratatuiの `Clear` + `Block` でオーバーレイを実現:

```rust
fn render_qr_popup(f: &mut Frame, area: Rect, url: &str) {
    // 画面中央にポップアップを配置
    let popup_area = centered_rect(60, 80, area);
    
    // 背景をクリア
    f.render_widget(Clear, popup_area);
    
    // QRコードを含むブロックを描画
    let qr_string = generate_qr_string(url).unwrap_or_default();
    let block = Block::default()
        .title("Web UI QR Code")
        .borders(Borders::ALL);
    
    let paragraph = Paragraph::new(format!("{}\n\n{}", qr_string, url))
        .block(block)
        .alignment(Alignment::Center);
    
    f.render_widget(paragraph, popup_area);
}
```

## キーバインド設計

| キー | モード | アクション |
|------|--------|------------|
| `w`  | Select/Running/Stopped | QrPopupモードに遷移 |
| `Esc` / 任意キー | QrPopup | 元のモードに戻る |

### 条件付き表示

- `web_url` が `Some` の場合のみ `w` キーが有効
- キーヒントに `w: QR code` を表示（Webサーバー有効時のみ）

## 依存関係

### 新規crate

```toml
[dependencies]
qrcode = "0.14"  # QRコード生成
```

### フィーチャーフラグ

QRコード機能は `web-monitoring` フィーチャーに含める:

```toml
[features]
web-monitoring = ["axum", "tower", "tower-http", "qrcode"]
```

## エラーハンドリング

1. **QRコード生成失敗**: URLが長すぎる場合など
   - フォールバック: URL文字列のみ表示
   
2. **Webサーバー未起動**: `web_url` が `None`
   - `w` キーを無視またはエラーメッセージ表示

3. **ターミナルサイズ不足**: QRコードが収まらない
   - 小さいバージョンのQRコードを生成（エラー訂正レベル調整）

## テスト戦略

1. **ユニットテスト**:
   - QRコード生成関数のテスト
   - AppMode遷移のテスト

2. **統合テスト**:
   - `w` キー押下時のモード遷移
   - ポップアップからの復帰

## 将来の拡張

- クリップボードへのURL コピー (`c` キー)
- ネットワークインターフェース選択（複数IP対応）
- QRコードのカスタマイズ（サイズ、エラー訂正レベル）
