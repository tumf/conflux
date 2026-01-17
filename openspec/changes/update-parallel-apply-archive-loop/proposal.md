# Change: serial の共通 apply/archive ループを正として parallel 実装を統合する

## Why

serial と parallel の実装で apply/archive の実行ループが分岐しており、retry/stagger、履歴注入、キャンセル、hook 実行やイベント通知の一貫性が損なわれている。現状でも `orchestration/apply.rs` と `orchestration/archive.rs` に共通ループの土台があるため、これを正として parallel を統合し、挙動差分と保守負荷を解消する。

## What Changes

- parallel 実行の apply/archive ループを `orchestration` の共通ループに寄せる
- worktree 実行や ParallelEvent など parallel 固有の差分は「入力/出力の変換レイヤ」で吸収する
- serial/parallel のループ責務を明確化し、重複実装を削除する
- 挙動は変更せず、実装経路の統合のみを行う

## Impact

- Affected specs: `parallel-execution`, `hooks`
- Affected code:
  - `src/orchestration/apply.rs` / `src/orchestration/archive.rs`
  - `src/parallel/executor.rs`
  - `src/parallel/mod.rs`
  - `src/tui/orchestrator.rs`
