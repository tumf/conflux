# 設計: Apply実行の有限化と中断時recovery

## Safety invariant

Conflux-owned repository mutationは、active agent process groupのquiescenceを証明した後にだけ開始できる。Applyがmanaged workspaceを変更した後に中断された場合、Confluxはrun終了前にrepository-visible changeを保存する。restart後の次actionはlogやprocess-local counterではなく、保存されたGit/workspace stateから決定する。

## 統一termination sequence

cancellation、TUI external shutdown、absolute runtime timeoutは次の順序を共有する。

1. run-command-scopeのspawnとretry admissionを閉じる。
2. active runner taskをcancelする。
3. owned process groupへSIGTERMを送る。
4. 既存grace period後にSIGKILLへescalateする。
5. 既存typed cleanup reportでprocess-group quiescenceを証明する。
6. managed-worktree statusを確認する。
7. dirtyの場合だけConflux-owned WIP iteration snapshotを1つ作成する。cleanの場合はsnapshot pathを呼ばず、空WIP commitを作らない。
8. same-run automatic redispatchを許可しないtyped terminal outcomeを返す。

cleanup failureはstep 6より前で停止する。snapshot failureはworkspaceを変更せず保持し、actionable diagnosticsを返す。いずれもsuccessful shutdownとして報告してはならない。

## Absolute runtime limit

`command_max_runtime_secs`は全AI commandが同じprocess ownership boundaryを使うため、common command-runner configurationへ置く。1つのdeadlineがlogical invocation全体を覆い、最初のchild spawn成功時に開始し、transport retry、inactivity retry、retry delay、child respawnを跨いで継続する。stdout、stderr、後続attemptでresetしない。`0`は無効化、defaultは3,600秒とする。

runtime-limit outcomeはinactivity timeout、transient error、generic non-zero crashではない。invocationの全後続retry admissionを閉じる。ApplyはさらにWIP progressを保存してactive runを停止し、operatorのinspectと明示的retryを待つ。

## WIP identityとrestart

WIP commitは既存`WorkspaceManager::create_iteration_snapshot` pathが作る通常のworkspace-local `WIP: <change-id> (...)` commitとする。timeout marker、retry counter、lifecycle stateをGit/workspace外へ保存しない。新しいConflux processは保存されたworktreeとbase comparisonからroutingを再計算する。

## Verification discipline

runtime enforcementはouter agent invocationを制限し、portable skillはagent内部のworkを制限する。

- verification commandはdefaultで1回実行する。
- retryは前回実行後のrepository repairまたは具体的なenvironment recoveryだけを根拠とする。
- 同一commandは1 Apply invocation内で最大3回まで実行できる。
- flaky testが疑われる場合は`verification_unstable`、invocation budget内に完了しない場合は`verification_timeout`を報告する。
- 両categoryは既存Implementation Blocker fact schemaとcompatible `BLOCKED` handoffを拡張する。Applyはfactsだけを報告し、ConfluxとAcceptanceがfinal `blocked`または`stalled` lifecycle classificationを所有する。
- heavyまたはnon-local gateはproposal-owned CI、Acceptance、operational observationへ割り当て、Apply loopへ置かない。

skillは特定harnessのtimeout commandへ依存しない。利用可能な場合はruntimeのmanaged execution facilityを使い、bounded executionを保証できない場合はstructured blocker evidenceとともに停止する。

## TUI signal integration

TUI keyboard stopとexternal SIGINT/SIGTERMは`TuiRunSupervisor`と`RunCommandScope`へ収束する。active runがある間、signal handlingはprocessを直接exitしてはならない。TUIは同じbounded cleanup barrierを待ち、lifecycle adapterをshutdownしてから終了する。二度目のsignalはforceful escalationを要求できるが、quiescence evidenceまたはWIP preservationを迂回できない。active runがないsignalは既存ordinary-stop classificationを維持する。

headless `cflx run`のsignal-driven cancellationはこのchangeの対象外とする。common command-runnerのabsolute runtime limitとApply WIP preservationはfrontend非依存だが、SIGINT/SIGTERM wiringはTUIだけに追加する。

## Test strategy

短いwall-clock閾値をcorrectness assertionに使わない。

- command deadlineはpaused Tokio timeまたはinjected clockとcontrolled sub-second child fixtureを使い、stateとcleanup evidenceで判定する。
- process cleanupは既存real process-group integration fixtureと十分余裕あるhang safeguardを使い、default target全体を1秒未満に保つ。
- Apply WIP preservationはordering用fake workspace managerとrestart-visible commit用Git-backed testを使い、clean worktreeでempty WIPを作らないcaseも検証する。
- TUI signal behaviorはsupervisor boundaryを直接driveし、scope/registry state transition、idle/background mutation classification、second-signal escalationを検証する。
- skill contract testはembedded source textと既存＋追加blocker schema fieldを検証する。
