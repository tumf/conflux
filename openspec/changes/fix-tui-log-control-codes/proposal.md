# Change: TUIログ表示から制御コードを除去する

## Why
TUI の Logs パネルに、ANSI の装飾シーケンス等の制御コードが文字として混入し、可読性が大きく低下することがあります。
例: `21:31:32 [96m[1m| [0m[90m Read     [0msrc/error.rs` のように、色情報がそのまま表示されます。

## What Changes
- TUI の Logs パネルに表示されるログメッセージから、ANSI エスケープシーケンス（特に SGR/CSI）や非表示制御文字を除去してから表示する
- 文字幅計算・省略（truncation）は、除去後の文字列に対して行う

## Impact
- Affected specs: `openspec/specs/cli/spec.md`（TUIログ表示・文字幅計算に関する要件）
- Affected code (planned): `src/tui/render.rs`（ログ描画）, `src/events.rs`（LogEntry生成）, 可能なら `src/tui/utils.rs`（サニタイズ処理共通化）
- User-visible impact: Logs が読みやすくなり、`[96m` 等のノイズが表示されなくなる
- Compatibility: ログメッセージの装飾（色付け）そのものは Ratatui 側の `entry.color` を引き続き利用し、ログ本文の ANSI 装飾は表示しない
