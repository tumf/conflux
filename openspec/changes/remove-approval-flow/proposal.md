# Change: 承認フロー廃止と実行マーク初期化

## Why
現在の approved ファイルと承認操作は運用負担が大きく、実行判断に不要です。起動時に実行マークを必ずクリアし、承認概念を廃止してワークフローを簡素化します。

## What Changes
- `approved` ファイルの作成/検証/削除を廃止し、存在しても無視する
- `approve` サブコマンドと Web 承認 API を削除する
- TUI の `@` キーによる承認/非承認操作を削除する
- 起動時はすべて未選択で開始し、自動キュー投入を行わない
- 実行対象の判定は承認状態ではなく、選択/指定対象のみで決定する
- `.git/info/exclude` への `openspec/changes/*/approved` 自動追加を廃止する

## Impact
- Affected specs: cli, tui-key-hints, tui-architecture, web-monitoring
- Affected code: src/approval.rs, src/openspec.rs, src/orchestrator.rs, src/tui/*, src/cli.rs, src/main.rs, src/web/*, src/vcs/git/*
