---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/command_queue.rs
  - src/ai_command_runner.rs
  - src/agent/runner.rs
  - src/orchestrator.rs
  - src/parallel_run_service.rs
  - openspec/specs/code-maintenance/spec.md
verifications:
  - id: command-runner-config-tests
    requirement: Command queue and AI runner construction preserve every effective configuration value
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for command_queue and ai_command_runner plus successful cargo check
    rerun: cargo test command_queue ai_command_runner && cargo check --all-features
    prerequisites: []
---

# Command queue configuration constructionを共通化する

**Change Type**: implementation

## Problem/Context

`OrchestratorConfig` から `CommandQueueConfig` を組み立て、`AiCommandRunner` に追加設定を反映する処理が複数の現役実行経路へ複製されている。たとえば `src/agent/runner.rs:112-133` と `src/agent/runner.rs:161-182`、`src/orchestrator.rs:118-143` と `src/orchestrator.rs:218-243`、`src/parallel_run_service.rs:58-83` と `src/parallel_run_service.rs:111-136` が同じ設定項目を個別に転記している。

新しい queue 設定を追加した際に一部の呼び出し元だけ反映が漏れる可能性があり、同一設定から生成した runner が経路ごとに異なる挙動になる保守リスクがある。既存の `AiCommandRunner` 共通層や stagger 共有方式は維持し、設定変換だけを単一の正規経路へ集約する。

## Proposed Solution

- `OrchestratorConfig` から全 `CommandQueueConfig` フィールドを生成する小さな共通変換を既存モジュールへ追加する。
- stream JSON textification、strict process cleanup、command environmentを含めた設定済み `AiCommandRunner` の生成を、現役経路で再利用できる最小のconstructorまたはhelperへ集約する。
- 完全一致するproduction call siteだけを共通経路へ移し、timeoutやretry値を意図的に上書きするテストfixtureはそのまま残す。
- queue semantics、既定値、共有 stagger state、公開CLI/APIを変更しない。

## Acceptance Criteria

- 共通変換が `CommandQueueConfig` の全フィールドへ現在と同じ値とfallbackを設定する。
- 共通runner生成が `stream_json_textify`、`strict_process_cleanup`、command environmentsを現在と同じように反映する。
- `AgentRunner`、CLI orchestrator、parallel run serviceなどの完全一致するproduction初期化は同じ共通経路を使用する。
- 特殊なテスト設定や明示的な呼び出し側overrideは共通化によって失われない。
- command実行、retry、timeout、cleanup、staggerの外部観測可能な挙動は変わらない。

## Explicit Completion Conditions

- `src/command_queue.rs` または既存の適切な設定境界に、全queueフィールドを対象とする単一の変換実装とunit testがある。
- `src/ai_command_runner.rs` または既存の適切なrunner境界に、追加setterを漏れなく適用する共通生成処理とunit testがある。
- `src/agent/runner.rs`、`src/orchestrator.rs`、`src/parallel_run_service.rs` の重複初期化が共通処理へ置換され、意図的に異なるliteral設定は維持されている。
- `cargo test command_queue ai_command_runner`、`cargo check --all-features`、`cargo fmt --all -- --check` が成功する。

## Out of Scope

- `AiCommandRunner`、`CommandQueue`、shared stagger stateのアーキテクチャ変更。
- retry、timeout、cleanup、環境変数の意味や既定値変更。
- テスト専用の個別 `CommandQueueConfig` fixtureの一律置換。
- 新しい依存crateや設定項目の追加。
