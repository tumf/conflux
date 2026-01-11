# Change: TUIに提案入力機能を追加

## Why

現状、新しい提案を追加するには外部エディタやCLIから `openspec` コマンドを直接実行する必要がある。TUI内から直接提案を入力・実行できれば、ワークフローが効率化される。

## What Changes

- 設定ファイルに `propose_command` オプションを追加
- TUIで `+` キーを押すと複数行テキスト入力ボックスが出現
- 入力完了後、`propose_command` がバックグラウンドで実行される
- CJK文字（日本語・中国語・韓国語）に対応した文字幅計算
- 実行結果はログパネルに表示

## Impact

- Affected specs: `configuration`, `tui-editor` (新しいキーバインド追加)
- Affected code:
  - `src/config/mod.rs` - propose_command設定の追加
  - `src/tui/state/mod.rs` - Proposingモードの追加
  - `src/tui/runner.rs` - `+`キーハンドリングとコマンド実行
  - `src/tui/render.rs` - テキスト入力ボックスのレンダリング
