# Change: TUIモードのアーカイブループ修正

## Why

TUIモードで完了済み変更のアーカイブが正しくループ処理されていない。
アーカイブコマンドは実行されるが、実際にファイルが `openspec/changes/archive/` に移動されなくても「成功」として扱われ、次のアーカイブ対象が処理されない問題がある。

診断で発見された問題：
1. アーカイブ検証ロジックのパスが間違っている（`openspec/archive` vs `openspec/changes/archive`）
2. アーカイブ失敗時のループ継続が期待通りに動作していない可能性

## What Changes

- `src/tui/orchestrator.rs` のアーカイブパス検証を修正
- アーカイブループのデバッグログを追加してトレーサビリティを向上
- アーカイブ失敗時のリトライロジックを明確化

## Impact

- Affected specs: cli (TUI archive loop behavior)
- Affected code: `src/tui/orchestrator.rs`
