## 1. 依存関係の追加

- [ ] 1.1 `Cargo.toml` に `tui-textarea` クレートを追加

## 2. 設定の追加

- [ ] 2.1 `OrchestratorConfig` に `propose_command: Option<String>` を追加
- [ ] 2.2 `get_propose_command()` メソッドを実装（デフォルト値なし、未設定時は `None` を返す）
- [ ] 2.3 `expand_proposal()` 関数を `config/expand.rs` に追加（`{proposal}` プレースホルダー展開）
- [ ] 2.4 設定テンプレートにコメント付きで `propose_command` を追加

## 3. TUIモードの追加

- [ ] 3.1 `AppMode::Proposing` を `tui/types.rs` に追加
- [ ] 3.2 `AppState` に `propose_textarea: Option<TextArea<'static>>` フィールドを追加
- [ ] 3.3 `AppState::start_proposing()` メソッドを実装（TextArea作成とモード切替）
- [ ] 3.4 `AppState::cancel_proposing()` メソッドを実装（TextAreaクリアとモード復帰）
- [ ] 3.5 `AppState::submit_proposal()` メソッドを実装（テキスト取得とモード復帰）

## 4. テキスト入力レンダリング

- [ ] 4.1 `render_propose_modal()` 関数を `tui/render.rs` に実装
- [ ] 4.2 モーダルダイアログの枠線とタイトルをレンダリング
- [ ] 4.3 TextAreaをモーダル内にレンダリング
- [ ] 4.4 `frame.set_cursor_position()` でIME候補ウィンドウ用のカーソル位置を設定
- [ ] 4.5 Proposingモードのフッターキーヒントを実装

## 5. キーイベント処理

- [ ] 5.1 `runner.rs` に `+` キーハンドリングを追加（Selectモード時のみ有効）
- [ ] 5.2 Proposingモード時のキーイベント分岐を実装
- [ ] 5.3 `KeyEventKind::Press | KeyEventKind::Repeat` のフィルタリングを追加
- [ ] 5.4 `Event::Paste` ハンドリングを追加（IME確定テキスト対応）
- [ ] 5.5 IME末尾改行アーティファクトの除去処理を実装
- [ ] 5.6 Enter で確定、Esc でキャンセルの実装
- [ ] 5.7 propose_command 未設定時の警告メッセージを実装

## 6. コマンド実行

- [ ] 6.1 `TuiCommand::ProposeInput(String)` を `tui/events.rs` に追加
- [ ] 6.2 コマンド展開ロジックを実装（`{proposal}` → 入力テキスト）
- [ ] 6.3 `tokio::process::Command` でバックグラウンド実行を実装
- [ ] 6.4 コマンド出力のログ送信を実装（stdout/stderr → LogEntry）
- [ ] 6.5 コマンド完了・エラー時のログ表示を実装

## 7. テスト

- [ ] 7.1 設定パース・展開のユニットテスト（`{proposal}` プレースホルダー）
- [ ] 7.2 モード遷移のテスト（Select → Proposing → Select）
- [ ] 7.3 空テキスト確定時の警告テスト

## 8. ドキュメント

- [ ] 8.1 README に提案入力機能を追加（`+` キー、設定例）
- [ ] 8.2 設定ファイルサンプルに `propose_command` を追加
