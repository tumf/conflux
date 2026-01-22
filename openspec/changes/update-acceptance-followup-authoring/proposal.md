# Change: Acceptance failure follow-up authoring in tasks.md

## Why

acceptance 出力が安定せず、FAIL 時に `ACCEPTANCE: FAIL` や `FINDINGS:` が tasks.md に混入する。
follow-up の作業項目を正確に残すため、acceptance エージェント自身が tasks.md を更新する流れに変更する。

## What Changes

- acceptance システムプロンプトに、FAIL 時の tasks.md 追記手順を明記する
- オーケストレーターによる acceptance 出力の追記更新を停止し、エージェントが tasks.md を直接更新する
- 受け入れ失敗時のテストを新仕様に合わせて更新する

## Impact

- Affected specs: `cli`, `agent-prompts`
- Affected code: `src/config/defaults.rs`, `src/orchestration/acceptance.rs`, `src/parallel/executor.rs`, `src/serial_run_service.rs`
