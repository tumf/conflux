# Change: Git検知時の自動parallel起動

## Why
Gitリポジトリ上で起動した際にparallelを既定有効にすることで、毎回の切り替え操作を不要にし、TUIとrunの体験を統一するため。

## What Changes
- Gitリポジトリ検知時、`parallel_mode` 未設定ならparallelを自動有効化
- 設定ファイルの `parallel_mode` が最優先で既定挙動を決定
- CLIの `--parallel` フラグは設定より優先してparallelを強制
- TUIとrunの両方に同じ優先順位を適用

## Impact
- Affected specs: configuration, cli, tui-editor
- Affected code: src/main.rs, src/orchestrator.rs, src/tui/state/mod.rs, src/tui/runner.rs, config handling
