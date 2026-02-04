# Change: 未コミット change 判定を部分未コミットまで拡張する

## Why
change の一部だけが未コミットでも並列モードでは実行対象外にしたいが、現状は `HEAD` のツリー存在だけで判定しており、部分未コミットが `UNCOMMITED` にならないためです。

## What Changes
- 未コミット change の定義に「`openspec/changes/<change_id>/` 配下の未コミット・未追跡ファイル」を含める
- 並列モードの実行対象判定に、change 単位の作業ツリー汚れ検出を追加する
- TUI の `UNCOMMITED` バッジ/操作不可判定を上記定義に合わせる

## Impact
- Affected specs: `tui-key-hints`, `parallel-execution`
- Affected code: `src/vcs/git/commands/commit.rs`, `src/tui/runner.rs`, `src/tui/orchestrator.rs`, `src/parallel_run_service.rs`
