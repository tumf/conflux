# Change: エラー文言の文脈情報強化

## Why
固定文字列のエラーが多く、運用時に原因や対象が特定しづらい。文脈情報（操作種別・change_id・workspace/cwd・失敗理由）を含めて、診断と復旧を容易にする必要がある。

## What Changes
- キャンセル・実行失敗・解析失敗などのエラーメッセージに文脈情報を追加する
- stdout/stderr 取得失敗などの内部エラーにコマンドと作業ディレクトリの情報を付与する
- エラーイベント/ログ出力で同一内容を使い、TUI とログの整合を保つ

## Impact
- Affected specs: observability
- Affected code: src/execution/*.rs, src/parallel/*.rs, src/ai_command_runner.rs, src/analyzer.rs, src/tui/orchestrator.rs
