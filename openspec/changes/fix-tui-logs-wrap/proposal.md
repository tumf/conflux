# Change: Logsビューの長文折り返しで表示行がずれる問題の修正

## Why
Logsビューで長い1行ログが折り返されると、表示行がずれて最新ログが見えにくくなり、スクロール位置も不安定になります。視認性と操作性を回復するために表示行計算と折り返し整形を修正します。

## What Changes
- Logsビューの表示範囲計算を「ログ件数」から「折り返し後の表示行数」基準に変更する
- 折り返し行はタイムスタンプとヘッダ幅分のインデントを維持して表示する
- ログバッファは保持し、表示のみを整形する

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/render.rs (render_logs) と関連テスト
