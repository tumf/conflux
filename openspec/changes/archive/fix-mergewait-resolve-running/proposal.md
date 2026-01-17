# Change: Allow MergeWait resolve in Running mode

## Why
TUI実行中にMergeWaitになったchangeでMを押してもresolvingに遷移せず、merge resolveを開始できません。表示上はM: resolveが出るため、操作と挙動が一致していません。

## What Changes
- RunningモードでもMergeWaitのchangeをMでresolveできるようにする
- resolve実行中は従来通りMをブロックし警告を表示する
- Mヒントの表示条件と挙動を一致させる

## Impact
- Affected specs: tui-key-hints
- Affected code: src/tui/state/mod.rs, src/tui/render.rs
