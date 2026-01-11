# Design: 提案入力機能

## Context

OpenSpec Orchestratorは変更提案を管理するツールである。現在、TUIは変更の選択・実行・アーカイブに対応しているが、新しい提案の作成は外部ツールに依存している。

CJK文字（日本語・中国語・韓国語）を扱うプロジェクトでは、これらの文字が全角（2セル幅）で表示されるため、正しい文字幅計算が必須となる。

参考: `~/work/study/openspec-tui/docs/rust-tui-best-practices.md`

## Goals / Non-Goals

**Goals:**
- TUI内で提案テキストを入力し、設定されたコマンドを実行
- 複数行テキスト入力のサポート
- CJK文字およびIME入力の完全サポート
- バックグラウンドでコマンドを実行し、結果をログに表示

**Non-Goals:**
- 構文ハイライト
- オートコンプリート
- ファイル選択UI

## Decisions

### テキスト入力ウィジェット

**Decision:** `tui-textarea` クレートを使用する

**Rationale:**
- IMEサポートが組み込み済み
- マルチバイト文字のカーソル位置計算が自動
- Undo/Redo機能が標準装備
- ratatui v0.29+との互換性あり
- 935K+ダウンロード、468スター、活発にメンテナンス

**Alternatives considered:**
- カスタム実装 → IME/CJKサポートが複雑、バグが発生しやすい
- `tui-input` → 単一行のみ、CJKサポートが不完全

### tui-textarea の使用方法

```rust
use tui_textarea::{TextArea, Input};

pub struct AppState {
    propose_textarea: Option<TextArea<'static>>,
}

impl AppState {
    fn create_textarea() -> TextArea<'static> {
        let mut textarea = TextArea::default();
        // ボーダーは親コンテナが提供するため、TextAreaには設定しない
        // （カーソル位置計算を正確にするため）
        textarea.set_style(Style::default().fg(Color::White));
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default().fg(Color::White).bg(Color::Gray));
        textarea
    }
}
```

### IME対応の重要ポイント

1. **KeyEventKind フィルタリング**: Press と Repeat のみ処理

```rust
if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
    return;
}
```

2. **Paste イベント処理**: IME確定テキストを正しく受け取る

```rust
Event::Paste(text) => {
    // IME確定時の末尾改行を除去
    let cleaned = text.trim_end_matches('\n');
    self.textarea.insert_str(cleaned);
}
```

3. **カーソル位置の明示的設定**: IME候補ウィンドウの表示位置を正確に

```rust
fn render(frame: &mut Frame, area: Rect, textarea: &TextArea) {
    frame.render_widget(textarea, area);
    // 注意: cursor() は (row, col) を返す
    let (row, col) = textarea.cursor();
    frame.set_cursor_position((area.x + col as u16, area.y + row as u16));
}
```

### コマンド実行

**Decision:** tokio::process::Command でバックグラウンド実行

**Flow:**
1. ユーザーが `+` を押す → Proposingモードに切り替え
2. テキスト入力 → Enter で確定、Esc でキャンセル
3. 確定時、propose_command を `{proposal}` プレースホルダーで展開
4. コマンドをスポーン、出力をログに送信
5. Selectモードに戻る

### キーバインド

| Key | Action |
|-----|--------|
| `+` | 提案入力モードを開始 |
| `Enter` | 確定してコマンド実行 |
| `Ctrl+Enter` | 改行挿入（tui-textareaのデフォルト動作） |
| `Esc` | キャンセル |
| `Backspace` | 文字削除（グラフェムクラスター単位） |
| `←/→/↑/↓` | カーソル移動 |
| `Ctrl+U` | Undo |
| `Ctrl+R` | Redo |

### 設定テンプレート

```jsonc
{
  // 提案コマンド（+キーで起動）
  "propose_command": "opencode run '{proposal}'"
}
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| 長いテキスト入力によるUI崩れ | tui-textareaの自動スクロール機能を活用 |
| 特殊文字によるコマンドインジェクション | シェルエスケープ処理 |
| コマンド長時間実行 | タイムアウト設定（デフォルト5分） |
| IME候補ウィンドウの位置ずれ | frame.set_cursor_position()を明示的に呼び出し |

## Dependencies

```toml
[dependencies]
tui-textarea = "0.7"  # または最新バージョン
```

## Open Questions

- (解決済み) Shift+Enter vs Ctrl+Enter → tui-textarea のデフォルト動作（Enter で改行）を使用
