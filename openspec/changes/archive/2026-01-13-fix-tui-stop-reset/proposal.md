# Change: TUI停止状態のリセットと並列再開の安定化

## Why
停止（Esc/Esc）後に `=` で parallel を有効化し、F5 で再開すると即座に停止してしまう。停止/キャンセル状態が新規実行に引き継がれているため、再開時に正しく処理が開始されず、ログや状態遷移が誤解を招く。

## What Changes
- F5 で新規実行を開始する際、停止/キャンセル状態を必ずリセットする
- Stopped モードで parallel へ切り替えた後も、F5 で正常に処理を開始する
- 直前の停止理由が次の実行の即時停止や成功メッセージ判定に影響しないようにする

## Impact
- Affected specs: `cli`, `parallel-execution`
- Affected code: TUI 状態管理、parallel 実行ループ、停止/キャンセルフラグ管理
