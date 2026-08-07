## 実装タスク

- [ ] `command_max_runtime_secs`をconfiguration type、custom > project > globalのmerge precedence、default、validation、generated configuration example、`CommandQueueConfig`へ追加する。configuration testでdefault `3600`、`0`無効化、precedence、example generationを証明した時点で完了とする(verification: unit - `cargo test --locked config`; verification-id: config-runtime-tests)
- [ ] common streaming command runnerへinvocation-wide typed absolute-runtime deadlineを追加する。deadlineはoutput、retry delay、child respawnでresetせず、後続retry admissionを全て閉じ、SIGTERM/SIGKILL cleanupでowned process groupを終了し、quiescenceを証明し、inactivity timeoutとcrashから区別できるoutcomeを返す。paused timeまたはcontrolled sub-second fixtureを使い、default verificationを1秒未満に保ち、timeoutはhang防止の十分余裕ある安全弁だけにする(verification: integration - `cargo test --locked --test process_cleanup_test`; verification-id: command-runtime-tests)
- [ ] Apply cancellationとruntime-limit handlingをrefactorし、process-group quiescence後だけdirty managed-worktree progressをsnapshotし、clean worktreeではsnapshotをskipし、snapshot failure時はworkspace contentsを保持し、same-run redispatchを起こさないtyped terminal resultを返す(verification: unit - `cargo test --locked execution::apply::tests`; verification-id: apply-interruption-tests)
- [ ] staged・unstaged・untracked progressがinterruption後のWIP commitへ残ること、clean interruptionでempty WIP commitを作らないこと、fresh processがworkspaceとGit evidenceだけからApply continuationを導出することを証明するrestart-focused Apply testを追加する(verification: unit - `cargo test --locked execution::apply::tests`; verification-id: apply-interruption-tests)
- [ ] TUI SIGINTとSIGTERMをprocess exit前に`TuiRunSupervisor` cancellationとrun-command-scope shutdown barrierへ接続する。active runがない場合のordinary-stop behavior、cleanup failure diagnostics、quiescenceを証明できない場合のnon-zero exitを含める(verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [ ] external signalがcommand admissionを閉じ、retryを抑止し、registered executionをdrainし、owned process identityを残さず、既存idle/background-mutation stop classificationを保持し、second signalがquiescenceまたはWIP preservationを迂回せずcleanupをescalateできることを証明するTUI shutdown testを追加する(verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [ ] `skills/cflx-apply/SKILL.md`とreference guidanceを更新し、single-run verificationをdefaultとし、no-change stability loopを禁止し、同一verification commandをevidence-bearing execution最大3回に制限し、`verification_timeout`または`verification_unstable`を既存structured blocker fields: category、command/output evidence、affected scope、prerequisiteまたはowner、unblock condition、next action、resumabilityとcompatible `BLOCKED` handoffで出力し、canonical lifecycle stateを主張しないようにする(verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [ ] `skills/cflx-proposal/SKILL.md`を更新し、bounded repository-local pathがないheavy、Docker、database、credentialed、deployed、long-running repository-wide gateをCI、Acceptance、operational-observation ownershipへ割り当て、Apply-blocking checkbox taskへ置かないようにする(verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [ ] embedded-skill contract testを拡張し、unchanged verification loopを許可するguidance、bounded blocker handoffまたは既存必須fieldを欠くguidance、non-local heavy gateをchange-blocking Apply workへ割り当てるguidanceをrejectする(verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)

## Future Work

- command runtime-limitの発生頻度とduration distributionをoperational monitoringへ追加できるが、metricsはnon-authoritativeであり本changeの必須条件ではない。
- headless `cflx run`のSIGINT/SIGTERMをactive Apply cancellationとWIP preservationへ接続するfollow-upを別changeで扱う。

## Final Validation

archive validationを最終OpenSpec gateとする。想定command: `cflx openspec validate prevent-runaway-apply-execution --archive-gate`。
