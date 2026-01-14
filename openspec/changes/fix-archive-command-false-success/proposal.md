# Change: archive_command が成功扱いでも未アーカイブなケースを再試行で吸収する

## Why
TUI/並列実行において、`archive_command` が終了コード 0 を返しても実際には `openspec archive` が実行されず、直後の検証で「未アーカイブ」と判定されてエラー表示になることがあります。ログ上は、その後の再実行で正常にアーカイブできており、ユーザー体験として誤検知に見えます。

## What Changes
- `archive_command` が成功（exit 0）したにもかかわらず、直後のアーカイブ検証で未アーカイブと判定された場合、オーケストレータは `archive_command` を一定回数まで再実行してから失敗扱いにします。
- 失敗扱いにする前に、再試行中であることをログに明示します。
- 追加の「待機（delay）」は導入しません（非同期実行の前提にしない）。

## Impact
- Affected specs: `openspec/specs/cli/spec.md`（TUI のアーカイブ追跡とエラー扱い）
- Affected code (planned): TUI の `archive_single_change` と、必要に応じて parallel 側の archive 実行フロー
- Risk: `archive_command` の冪等性に依存するため、再実行時の副作用を最小化する（例: 既に archived の場合は no-op になることを期待）
