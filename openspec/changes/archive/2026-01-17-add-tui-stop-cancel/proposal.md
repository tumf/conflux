# Change: TUIのStopping解除操作を追加

## Why
Stoppingモードに入った後に解除できないため、誤操作や状況判断の変更で継続したい場合でも復帰できません。
F5で停止解除できるようにし、操作の一貫性とユーザビリティを改善します。

## What Changes
- Stoppingモード中にF5で停止解除し、Runningへ復帰できるようにする
- Stoppingモード中のヘルプ文言に「F5: continue」を表示する
- 既に停止完了した場合は解除できない旨をログに残す

## Impact
- Affected specs: cli
- Affected code: src/tui/runner.rs, src/tui/render.rs, src/tui/state/modes.rs
