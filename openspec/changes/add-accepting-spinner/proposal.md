# Change: TUIのaccepting時にスピナーを表示

## Why
acceptingステータス中の進行状況が視覚的に分かりにくく、処理が動作中であることを即座に判断できないため。

## What Changes
- runningモードのChange一覧で、acceptingステータスにProcessingと同じスピナー表示を追加する。
- 表示タイミングはacceptance実行中に限定する。

## Impact
- Affected specs: cli
- Affected code: src/tui/render.rs, src/tui/state/*.rs
