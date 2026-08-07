## MODIFIED Requirements

### Requirement: TUI Stop Processing with Escape Key

The TUI MUST converge keyboard stop, SIGINT, and SIGTERM on the same bounded run-supervisor shutdown boundary. When agent execution is active, shutdown MUST close command admission, cancel the run, terminate owned process groups, prove quiescence, preserve dirty Apply progress through the interruption-recovery policy, and only then exit. External signals MUST NOT bypass child cleanup or WIP preservation. If cleanup or preservation cannot be proven, the TUI process MUST exit non-zero with actionable diagnostics.

#### Scenario: 強制停止で子プロセスが残らない

- **GIVEN** 現在のエージェントプロセスまたはin-flight実行が存在する
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** command admission が閉じられる
- **AND** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** process-group quiescence が確認される
- **AND** dirty Apply progress は終了前にWIP snapshotへ保存される
- **AND** 変更の状態はNotQueuedに戻る
- **AND** 実行マークは保持される

#### Scenario: SIGTERM uses the TUI shutdown boundary

**Given**: `cflx tui` owns an active Apply command and descendant processes
**When**: the TUI process receives SIGTERM
**Then**: the signal requests supervisor cancellation instead of immediate process exit
**And**: retry and spawn admission are closed
**And**: all owned process groups are terminated and proven quiescent
**And**: dirty Apply progress is preserved before the TUI exits

#### Scenario: SIGINT cleanup failure is visible

**Given**: `cflx tui` receives SIGINT during active execution
**When**: bounded cleanup cannot prove that the owned process group is empty
**Then**: the TUI exits non-zero with cleanup diagnostics
**And**: it does not claim that processing stopped cleanly
**And**: it retains workspace contents for recovery
