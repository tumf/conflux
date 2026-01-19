## 1. Implementation
- [x] 1.1 並列実行のアクティブ状態判定を定義する（apply/archive/acceptance/resolve をアクティブ、merged/merge_wait/error/not queued を非アクティブ）。完了条件: アクティブ判定の関数/列挙を追加し、`src/vcs/mod.rs` か `src/parallel/mod.rs` に実装がある。
- [x] 1.2 空きスロット算出でアクティブのみをカウントするよう修正する。完了条件: `src/parallel/mod.rs` の available_slots 計算がアクティブ判定を使用する。
- [x] 1.3 既存の cleanup/merge 後の状態遷移と整合するように必要なステータス更新を整理する。完了条件: 対応する更新箇所が明示され、アクティブ判定の想定と矛盾しない。
- [x] 1.4 並列実行の挙動をテスト/検証できるよう最小のテストまたはログ観測点を追加する。完了条件: テスト追加またはログでアクティブ数が確認できる。
- [x] 1.5 `cargo test`（必要な範囲）を実行して結果を記録する。完了条件: 実行コマンドと結果がログに残る。


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) WorkspaceStatus enum lacks explicit states for Accepting, Archiving, Resolving, and MergeWait as required by specification line 7
  2) Specification states "apply / acceptance / archive / resolve が進行中の change" implying these should be distinct trackable states
  3) Current implementation lumps acceptance/archive/resolve under Applied status, which is semantically imprecise despite being functionally correct
  4) The TUI has these states (QueueStatus) but the core workspace manager (WorkspaceStatus) does not, creating a semantic mismatch


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) WorkspaceStatus states Accepting, Archiving, Resolving, and MergeWait are defined but NEVER SET in production code (dead code)
  2) No call to update_workspace_status with Accepting, Archiving, Resolving, or MergeWait exists outside of tests
  3) Specification line 7 states "apply / acceptance / archive / resolve が進行中" implying distinct states, but implementation only uses Created/Applying/Applied
  4) Task 1.3 claims status updates are properly integrated, but grep shows no production code sets the new states
  5) Task 1.1 completion claim is misleading: states are defined but not used in actual execution flow


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) Workspaces awaiting merge (MergeAttempt::Deferred) remain in Applied status and are counted as active, violating spec line 7: "merge_wait ... はアクティブとして扱ってはならない（MUST NOT）"
  2) When merge is deferred (at src/parallel/mod.rs:1977), workspace status is not updated to MergeWait
  3) Deferred workspaces continue to occupy execution slots (src/vcs/mod.rs:157 returns true for Applied), preventing new changes from starting
  4) Fix required: Set workspace status to MergeWait at src/parallel/mod.rs:1978 after inserting into merge_deferred_changes
