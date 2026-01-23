# Change: TUIログに展開済みエージェントコマンドを表示

## 背景
- TUI Logs Viewではapply/archive/resolveの実行コマンドがプレースホルダーのまま表示され、実際の実行内容を即座に確認できない
- 問題解析時にCLIログへ切り替える必要があり、オペレーション負荷が高い

## 変更内容
- apply/archive/resolveの実行前に展開済みコマンド文字列をイベント経由でTUI Logs Viewに表示する
- 既存のログレベル分類と`--logs`同期の挙動は維持する

## 影響範囲
- Affected specs: `observability`
- Affected code: `src/events.rs`, `src/agent/runner.rs`, `src/parallel/mod.rs`, `src/parallel/executor.rs`, `src/tui/state/events/stages.rs`, `src/web/state.rs`, `src/orchestration/state.rs`
