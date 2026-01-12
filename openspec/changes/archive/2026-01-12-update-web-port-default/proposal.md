# Change: Web監視のデフォルトポートを自動割り当てに変更

## Why
- 既定の`8080`は他ツールと衝突しやすく、起動失敗の原因になりやすい
- よくある手法（OSに空きポートを割り当てさせる）で、未使用ポートを確実に利用したい

## What Changes
- `--web`実行時、CLIや設定でポート未指定ならOSが空きポートを自動割り当てする
- 起動ログに実際のバインド先（アドレス/ポート）を明示する
- `--web-port`や設定ファイル指定時は従来どおり固定ポートを使用する

## Impact
- Affected specs: `web-monitoring`
- Affected code: `src/cli.rs`, `src/main.rs`, `src/web/*`, `README.md`, `.openspec-orchestrator.jsonc`（既定値説明）
