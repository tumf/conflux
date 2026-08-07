## 実装タスク

- [x] `command_max_runtime_secs`をconfiguration type、custom > project > globalのmerge precedence、default、validation、generated configuration example、`CommandQueueConfig`へ追加する。configuration testでdefault `3600`、`0`無効化、precedence、example generationを証明した時点で完了とする。validationはsibling command knob(`command_inactivity_timeout_secs`)と同じmodelに従い、任意の`u64`を受理して`0`を明示的なdisableとするため、強制されるruleは新しいrejection pathではなく`0`-disable semanticsである(verification: unit - `cargo test --locked config`; verification-id: config-runtime-tests)
- [x] common streaming command runnerへinvocation-wide typed absolute-runtime deadlineを追加する。deadlineはoutput、retry delay、child respawnでresetせず、後続retry admissionを全て閉じ、SIGTERM/SIGKILL cleanupでowned process groupを終了し、quiescenceを証明し、inactivity timeoutとcrashから区別できるoutcomeを返す。paused timeまたはcontrolled sub-second fixtureを使い、default verificationを1秒未満に保ち、timeoutはhang防止の十分余裕ある安全弁だけにする(verification: integration - `cargo test --locked --features heavy-tests --test process_cleanup_test absolute_runtime_limit`; verification-id: command-runtime-tests)
- [x] Apply cancellationとruntime-limit handlingをrefactorし、process-group quiescence後だけdirty managed-worktree progressをsnapshotし、clean worktreeではsnapshotをskipし、snapshot failure時はworkspace contentsを保持し、same-run redispatchを起こさないtyped terminal resultを返す(verification: unit - `cargo test --locked execution::apply::tests::interrupted_apply`; verification-id: apply-interruption-tests)
- [x] staged・unstaged・untracked progressがinterruption後のWIP commitへ残ること、clean interruptionでempty WIP commitを作らないこと、fresh processがworkspaceとGit evidenceだけからApply continuationを導出することを証明するrestart-focused Apply testを追加する。design.mdの通りこれらはGit-backedでintegration-shapedなevidenceであり、`interrupted_apply`のunit-scoped decision testと並べて`execution::apply::tests::interrupted_apply_restart`に置く(verification: unit + integration - `cargo test --locked execution::apply::tests::interrupted_apply`; verification-id: apply-interruption-tests)
- [x] TUI SIGINTとSIGTERMをprocess exit前に`TuiRunSupervisor` cancellationとrun-command-scope shutdown barrierへ接続する。active runがない場合のordinary-stop behavior、cleanup failure diagnostics、quiescenceを証明できない場合のnon-zero exitを含める(verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [x] external signalがcommand admissionを閉じ、retryを抑止し、registered executionをdrainし、owned process identityを残さず、既存idle/background-mutation stop classificationを保持し、second signalがquiescenceまたはWIP preservationを迂回せずcleanupをescalateできることを証明するTUI shutdown testを追加する(verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [x] `skills/cflx-apply/SKILL.md`とreference guidanceを更新し、single-run verificationをdefaultとし、no-change stability loopを禁止し、同一verification commandをevidence-bearing execution最大3回に制限し、`verification_timeout`または`verification_unstable`を既存structured blocker fields: category、command/output evidence、affected scope、prerequisiteまたはowner、unblock condition、next action、resumabilityとcompatible `BLOCKED` handoffで出力し、canonical lifecycle stateを主張しないようにする(verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [x] `skills/cflx-proposal/SKILL.md`を更新し、bounded repository-local pathがないheavy、Docker、database、credentialed、deployed、long-running repository-wide gateをCI、Acceptance、operational-observation ownershipへ割り当て、Apply-blocking checkbox taskへ置かないようにする(verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [x] embedded-skill contract testを拡張し、unchanged verification loopを許可するguidance、bounded blocker handoffまたは既存必須fieldを欠くguidance、non-local heavy gateをchange-blocking Apply workへ割り当てるguidanceをrejectする(verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)

## Future Work

- command runtime-limitの発生頻度とduration distributionをoperational monitoringへ追加できるが、metricsはnon-authoritativeであり本changeの必須条件ではない。
- headless `cflx run`のSIGINT/SIGTERMをactive Apply cancellationとWIP preservationへ接続するfollow-upを別changeで扱う。

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate prevent-runaway-apply-execution --archive-gate`.

Single-run verification evidence for this apply:

- `cargo test --locked` (default tier): pass, exit 0.
- `cargo test --locked --features heavy-tests --test process_cleanup_test absolute_runtime_limit`: 3 passed, 0 failed.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cflx openspec validate prevent-runaway-apply-execution --strict` and `--archive-gate`: validation passed.
