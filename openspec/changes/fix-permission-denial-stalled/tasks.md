## Implementation Tasks

- [x] Define shared permission/policy execution-blocker classification. (verification: unit - add/extend `src/permission.rs` tests for `permission requested` + `auto-reject`, file Read permission denied, tool access denied, command-level harness rejection, and non-permission failures that must not match)
- [x] Track denial signatures and progress evidence for retry classification. (verification: unit - add tests in `src/permission.rs` or a new adjacent classifier test module proving first denial, changed denied target, and repository-visible progress do not produce stalled, while repeated same denial without progress does)
- [x] Stop apply retry only on repeated unresolved permission/policy blockers. (verification: unit/integration - add/extend apply-loop coverage in `src/execution/apply.rs` or `src/parallel/tests/executor.rs` proving first matching denial may retry, progress resets the blocker, and repeated same denial exits apply as stalled without another apply iteration, without empty-WIP escalation, and without terminal error)
- [x] Preserve normal apply failure behavior for non-permission errors. (verification: unit/integration - add/extend apply-loop coverage in `src/execution/apply.rs` or `src/parallel/tests/executor.rs` proving an unmatched non-zero apply command still follows the existing failure path)
- [x] Classify acceptance command failures before terminal error handling. (verification: integration - add/extend `src/parallel/tests/executor.rs` or `src/orchestration/acceptance.rs` tests proving first command-level permission denial does not immediately stall, while repeated unresolved command denial emits stalled state and does not return `Acceptance command failed` as terminal error)
- [x] Classify acceptance FAIL findings before follow-up retry handling. (verification: integration - add/extend `src/parallel/tests/executor.rs` dispatch coverage proving first permission-denial findings follow the existing non-blocker path, while repeated unresolved permission-denial findings become stalled without `record_acceptance_follow_up` effects or apply-loop continuation)
- [x] Preserve normal acceptance FAIL retry behavior. (verification: integration - add/extend `src/parallel/tests/executor.rs` dispatch coverage proving ordinary implementation findings still append follow-up tasks and return to apply)
- [x] Wire reducer/event state for repeated unresolved permission/policy blockers as non-terminal stalled holds. (verification: unit - add/extend `src/orchestration/state.rs` tests proving blocker events produce `display_status() == "stalled"`, `TerminalState::None`, and metadata with permission/operator guidance)
- [x] Surface operator guidance without dependency-blocked terminology. (verification: integration/manual - inspect emitted `LogEntry`/runtime metadata in `src/parallel/tests/executor.rs` or a local dry-run fixture to confirm status/reason mentions repeated unresolved permission/tool policy remediation and does not label the condition as dependency `blocked`)
- [x] Verify cycle-limit protection. (verification: integration - add/extend `src/parallel/tests/executor.rs` dispatch/apply+acceptance cycle test proving repeated unresolved permission denial does not continue until `Max apply+acceptance cycles reached`)

## Future Work

- Operator must update the actual local harness/tool permission policy outside Conflux before resuming a stalled change.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-permission-denial-stalled --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] `cargo test permission` が失敗しており、実装タスクで要求された permission 関連ユニット検証が通っていません。実行コマンド: `agent-exec run -- cargo test permission`（job_id `05b839d9eabd9fd48bb2c2a1f1b6dc08`, exit_code 101）。失敗1: `src/permission.rs:356` の `permission::tests::test_classifies_tool_access_denied` で `Tool access denied: Bash` が `ToolAccess` ではなく `FileRead` に分類されています（stdout.log lines 22-28）。`file_read_denied_pattern` が広すぎて tool denial を先に捕捉しているため、分類順序または正規表現境界を修正してください。
- [x] `cargo test permission` の失敗2: `src/orchestration/state.rs:2384` の `orchestration::state::tests::test_execution_blocked_permission_denial_transitions_to_stalled_with_operator_guidance` で `metadata.contains("operator action")` が false です（stdout.log lines 30-34）。`PermissionDenial::format_guidance()` は `Operator action required` と大文字で返している一方、テストは小文字の `operator action` を要求しています。実際の operator guidance メタデータが仕様どおり検証されるよう、出力またはテスト期待値を整合させてください。
- [x] 提案の明示完了条件では `cflx openspec validate fix-permission-denial-stalled --strict --evidence warn` と関連 Rust tests の成功が要求されています。OpenSpec strict/evidence warn と archive-gate は通過しましたが、関連 Rust tests が失敗しているため archive-ready ではありません。

## Acceptance #2 Failure Follow-up
- [x] Acceptance #1 Failure Follow-up の3件目を behavior-bearing checkbox として解釈されないよう整理し、同セクションの検証結果記録を自己参照の最終検証タスクから分離する。 (verification: manual - `openspec/changes/fix-permission-denial-stalled/tasks.md` の Failure Follow-up と `## Final Validation` セクションを確認)
