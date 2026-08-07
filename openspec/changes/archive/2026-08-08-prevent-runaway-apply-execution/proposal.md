---
change_type: hybrid
priority: high
dependencies: []
references:
  - src/execution/apply.rs
  - src/ai_command_runner.rs
  - src/command_queue.rs
  - src/process_manager.rs
  - src/tui/run_supervisor.rs
  - src/main.rs
  - skills/cflx-apply/SKILL.md
  - skills/cflx-proposal/SKILL.md
verifications:
  - id: apply-interruption-tests
    requirement: 中断または実行時間上限に達したApplyが自動再投入なしでrepository progressを保存する
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/execution/apply.rs
    evidence: Apply loop unit testがcleanup、WIP snapshot、terminal classification、restart-visible progressを証明する
    rerun: cargo test --locked execution::apply::tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: config-runtime-tests
    requirement: 絶対実行時間設定のdefault、precedence、disable semantics、生成例が安定している
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/config/mod.rs
    evidence: configuration unit testがdefault、custom-project-global precedence、zero-disable、生成例を証明する
    rerun: cargo test --locked config
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: command-runtime-tests
    requirement: invocation全体で単一の絶対実行時間上限が出力とretry attemptに依存せずowned process groupを終了する
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/process_cleanup_test.rs
    evidence: 1秒未満のcontrolled process fixtureがtimeout、retry suppression、SIGTERM/SIGKILL escalation、quiescenceを短いwall-clock正否閾値なしで証明する
    rerun: cargo test --locked --features heavy-tests --test process_cleanup_test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: tui-shutdown-tests
    requirement: TUI external shutdownがrun command scopeをdrainしてowned descendantを残さない
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/run_supervisor.rs
    evidence: TUI supervisor testがSIGINTとSIGTERMで共通のbounded shutdown boundaryを使用することを証明する
    rerun: cargo test --locked tui::run_supervisor::tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: skill-contract-tests
    requirement: Applyとproposal guidanceが無制限または重複したverification workを防ぐ
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/install_skills_test.rs
    evidence: embedded skill contract testがbounded verification retry、blocker handoff、heavy-gate ownershipを検証する
    rerun: cargo test --locked --test install_skills_test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# 暴走するApply実行を防止する

**Change Type**: hybrid

## 問題と背景

Confluxは無出力timeoutだけを適用しており、invocation全体の絶対実行時間上限を持たないため、managed Apply commandが出力を続ける限り無期限に実行され得る。operatorがこの作業を中断すると、Apply cancellation pathがdirty workspace progressを保存する前にreturnし、次のrunがworkspace evidence上で最初のApply attemptに見える状態から再開する場合がある。portable Apply skillもforeground verificationと修正後の再実行を要求する一方、有限なretry境界を定めておらず、agentが独自のstability loopや数時間のrepository gateを作る余地がある。

TUI internal stopはbounded process-group cleanupを所有するが、external SIGINTまたはSIGTERMも同じrun cancellation、process quiescence、progress preservation境界へ到達する必要がある。これがないと、TUI process終了後もdetached agent、shell、Cargo、test descendantが残り得る。

## 提案する解決策

次の4つを1つのApply safety boundaryとして導入する。

1. Applyがcancel、TUI external signal、absolute runtime limitで停止した場合、owned process-group quiescence確認後にdirty managed-worktree progressをConflux WIP snapshotとして保存する。
2. output activityとinactivity timeoutから独立した`command_max_runtime_secs`を追加する。defaultは3,600秒、`0`は無効化であり、最初のchild spawnからretryを跨ぐlogical invocation全体を覆う。
3. `cflx-apply`と`cflx-proposal` guidanceを更新し、verificationはdefaultで1回、無変更stability loopは禁止、retryは新しいrepairまたはenvironment-recovery evidenceの後だけ許可し、完了しないverificationは既存schema互換のstructured blockerとして返す。
4. TUI SIGINTとSIGTERMをoperator stopと同じbounded shutdown boundaryへ接続し、command admission停止、owned process group終了とquiescence証明、Apply progress保存後に終了する。

これらは一体で提供する。progress保存のないdeadlineは作業を失い、quiescenceのない保存はrepository mutationと競合し、runtime enforcementのないguidanceだけではmisbehaving agentを有限時間に制限できないためである。

## 受け入れ基準

- dirty managed Applyをcancelすると、owned process groupを終了してquiescenceを証明し、staged・unstaged・untrackedのchange-owned progressを含むWIP snapshotを作成し、Acceptanceをdispatchしない。
- clean managed Applyのcancelまたはruntime-limit expiryでは空WIP commitを作成しない。
- fresh processはlogや外部durable stateを参照せず、保存されたworkspaceとGit stateから次のactionを導出する。
- `command_max_runtime_secs`はdefault 3,600秒、`0`で無効、custom > project > globalのprecedenceに従い、生成configuration exampleへ含まれ、output activityと独立する。
- 1つのabsolute deadlineがcommand-queue retry attempt全体を覆い、retryまたはrespawnでreset・乗算されない。
- 継続的に出力するagentもinvocation-wide absolute deadlineで終了する。
- runtime-limit terminationはordinary crashと区別され、同じrunで自動retryされない。
- TUI SIGINTとSIGTERMはrun command admissionを閉じ、runをcancelし、既存のgraceful-then-forceful pathでowned process groupを終了してquiescenceを証明し、dirty Apply progressを保存し、owned descendantを残さない。
- active runがないTUI signalは通常停止として扱われ、存在しないprocessのforce stopを主張しない。
- 二度目のsignalによるforceful escalationもquiescence証明とWIP保存を迂回しない。
- process quiescenceまたはWIP preservationを証明できない場合、Confluxはactionable diagnosticsを伴うnon-zeroで終了し、workspace contentsを保持し、正常cleanupを報告しない。
- `cflx-apply`はno-change stability loopを禁止し、同一verification commandをrepository repairまたは具体的なenvironment recovery後に限り最大3回実行できる。完了できない場合は、既存structured Implementation Blocker schemaとcompatible `BLOCKED` handoffで`verification_timeout`または`verification_unstable` factsを記録し、final `blocked`または`stalled` lifecycle stateを主張しない。
- `cflx-proposal`はbounded repository-local verification pathがないDocker、database、heavy、credentialed、deployed、long-running repository-wide gateをApply-blocking checkbox taskへ置かない。

## 明示的な完了条件

- Apply cancellationとabsolute-timeout branchは、WIP snapshot前にprocess-group cleanupを行い、same-run redispatchを抑止するtyped terminal outcomeを返すtested helperを共有する。
- configuration type、merge、default、generated example、command-runner wiringが`command_max_runtime_secs`を一貫して公開し、`cargo test --locked config`がconfiguration contractを証明する。
- timeout testはpaused timeまたはcontrolled sub-second process fixtureを使用し、短いwall-clock正否閾値ではなくstateとcleanup evidenceで判定する。default test targetは1秒未満を維持し、timeoutはhang防止の十分余裕ある安全弁だけに使う。
- TUI signal testはoperator stopと同じsupervisor shutdown boundaryを実行し、登録executionまたはprocess identityが残らないこと、idle/background mutationの既存分類、second-signal escalationを検証する。
- embedded skill sourceとinstalled-skill contract testがbounded verification、canonical blocker fields、heavy-gate ownershipを含む。
- `cargo test --locked execution::apply::tests`、`cargo test --locked config`、`cargo test --locked --features heavy-tests --test process_cleanup_test`、`cargo test --locked tui::run_supervisor::tests`、`cargo test --locked --test install_skills_test`が成功する。`tests/process_cleanup_test.rs`は実プロセスグループを駆動するため`heavy-tests` featureでgateされており、rerun commandはzero testを黙って実行しないようそのfeatureを明示する。
- Rust pathがstageされた場合のrepository-wide rustfmtとclippyは既存path-scoped pre-commit hookが所有し、本proposalは重複するApply checkbox taskを作らない。

## 対象外

- CorvusのCargo profile、build parallelism、test implementationの変更。
- 任意のagent shell commandをparseしてsemantic repetitionを検出すること。
- process-local retry counterまたはexecution stateをworkspace外へ永続化すること。
- common command deadlineを利用する範囲を超えてAcceptance、Archive、Resolve、Analyze固有のruntime limitを変更すること。
- headless `cflx run`へsignal-driven cancellationを追加すること。本changeのSIGINT/SIGTERM収束はTUIだけを対象とし、common absolute-runtime expiryはfrontend非依存とする。
- deployed-serviceまたはcredentialed post-integration verificationの追加。
