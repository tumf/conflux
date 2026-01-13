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

## URL変換ロジック

### 自動割当ポートとバインドアドレスからアクセスURLへの変換

Webサーバーはポート0を指定した場合、OSが空いているポートを自動割当する。
サーバー起動後に実際のポートを取得し、モバイルデバイスからアクセス可能なURLを構築する:

```rust
/// サーバー起動後に実際のポートを取得してURLを構築
fn build_access_url(bind_addr: &str, actual_port: u16) -> String {
    let host = match bind_addr {
        "0.0.0.0" => get_local_ip().unwrap_or_else(|| {
            tracing::warn!("Could not determine local IP, using localhost (limited accessibility)");
            "localhost".to_string()
        }),
        "127.0.0.1" | "localhost" => "localhost".to_string(),
        addr => addr.to_string(),
    };
    format!("http://{}:{}", host, actual_port)
}

fn get_local_ip() -> Option<String> {
    // ローカルネットワークのIPアドレスを取得
    // 例: 192.168.1.100
    local_ip_address::local_ip().ok().map(|ip| ip.to_string())
}

/// サーバー起動後に実際のポートを取得
async fn get_actual_port(listener: &TcpListener) -> u16 {
    listener.local_addr().unwrap().port()
}
```

### 変換ルール

| バインドアドレス | 実際のポート | 変換後のURL |
|------------------|--------------|-------------|
| `0.0.0.0:0` (自動) | 54321 | `http://192.168.1.100:54321` |
| `127.0.0.1:0` (自動) | 54321 | `http://localhost:54321` |
| `0.0.0.0:8080` (固定) | 8080 | `http://192.168.1.100:8080` |
| `127.0.0.1:8080` (固定) | 8080 | `http://localhost:8080` |

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
local-ip-address = "0.6"  # ローカルIP取得
```

### フィーチャーフラグ

QRコード機能は `web-monitoring` フィーチャーに含める:

```toml
[features]
web-monitoring = ["axum", "tower", "tower-http", "qrcode", "local-ip-address"]
```

## エラーハンドリング

1. **QRコード生成失敗**: URLが長すぎる場合など
   - フォールバック: URL文字列のみ表示

2. **Webサーバー未起動**: `web_url` が `None`
   - `w` キーを無視またはエラーメッセージ表示

3. **ターミナルサイズ不足**: QRコードが収まらない
   - 小さいバージョンのQRコードを生成（エラー訂正レベル調整）

4. **ローカルIP取得失敗**: ネットワークインターフェースがない場合
   - `localhost` にフォールバック（モバイルからはアクセス不可の警告表示）

## テスト戦略

1. **ユニットテスト**:
   - QRコード生成関数のテスト
   - URL変換ロジックのテスト
   - AppMode遷移のテスト

2. **統合テスト**:
   - `w` キー押下時のモード遷移
   - ポップアップからの復帰

## 将来の拡張

- クリップボードへのURL コピー (`c` キー)
- ネットワークインターフェース選択（複数IP対応）
- QRコードのカスタマイズ（サイズ、エラー訂正レベル）
