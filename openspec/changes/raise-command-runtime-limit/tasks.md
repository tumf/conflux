## Implementation Tasks

- [x] `DEFAULT_COMMAND_MAX_RUNTIME_SECS`とdefault getter characterizationを10,800秒へ更新し、明示値、`0`無効化、normal config precedence、inactivity timeoutとの独立性を維持する。完了条件は`src/config/defaults.rs`と`src/config/mod.rs`の期待値が一致し、config testが全契約を通すことである。(verification: unit - `cargo test --locked config`; verification-id: command-runtime-default-tests)
- [x] generated JSONC templates、`docs/guides/CONFIG.md`、`src/config/types.rs`のAPI commentsを10,800秒（3時間）の既定値へ同期する。完了条件は全テンプレートvariantとtracked guideが同じdefaultと`0` disable semanticsを示し、旧3,600秒を既定値として案内しないことである。(verification: unit - `cargo test --locked config`; verification-id: command-runtime-default-tests)
- [x] `skills/cflx-apply/SKILL.md`、そのtracked reference、`skills/cflx-proposal/SKILL.md`のbounded invocation guidanceを10,800秒へ同期し、single-run verification、no-change stability loop禁止、structured blocker handoffを維持する。完了条件はembedded skill testが配布内容を検証し、旧3,600秒を既定値として案内しないことである。(verification: integration - `cargo test --locked --test install_skills_test`; verification-id: embedded-runtime-guidance-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate raise-command-runtime-limit --archive-gate`.
