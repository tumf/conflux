# Change: worktreeは原則保持し、merged成功時のみcleanup

## Why
並列実行の中断や失敗時にworktreeが削除されると、再開や調査ができず作業が失われる。現状はキャンセル時の経路でcleanupが走るため、保持方針に統一する必要がある。

## What Changes
- worktreeは原則保持し、削除はマージ成功時のみ実行する方針に統一する
- 早期終了/キャンセル時のcleanupは抑止し、再開可能性を維持する
- cleanupガードの挙動を「成功時のみ削除」に合わせて整理する

## Impact
- Affected specs: workspace-cleanup, parallel-execution
- Affected code: parallel executorのcleanup経路、cleanup guardのDrop経路
