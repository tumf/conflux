---
change_type: hybrid
priority: medium
dependencies: []
references:
  - src/config/defaults.rs
  - src/config/types.rs
  - src/config/mod.rs
  - src/templates.rs
  - docs/guides/CONFIG.md
  - skills/cflx-apply/SKILL.md
  - skills/cflx-proposal/SKILL.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/command-queue/spec.md
verifications:
  - id: command-runtime-default-tests
    requirement: 未設定時のAI command absolute runtime limitが10,800秒となり、明示値、0無効化、設定優先順位、inactivity timeoutとの独立性を維持する
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/config/mod.rs
    evidence: config unit testsがdefault 10,800秒、明示値、0無効化、merge precedence、生成例を検証する
    rerun: cargo test --locked config
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: embedded-runtime-guidance-tests
    requirement: 配布される設定例、利用者向け文書、埋め込みoperation skillが10,800秒の既定値を一貫して案内する
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/install_skills_test.rs
    evidence: tracked sourceの表記確認とembedded skill contract testsが旧3,600秒の既定値表記を残さない
    rerun: cargo test --locked --test install_skills_test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# AI commandの既定実行時間上限を3時間へ延長する

**Change Type**: hybrid

## Problem / Context

Confluxの共通AI command runnerは、出力が継続する実装作業も有限時間で終了させるため、`command_max_runtime_secs`を絶対実行時間上限として適用している。現在の既定値3,600秒では、依存関係の構築、修正、repository-local verificationまでを一つのApply invocationで完了するtask実装が途中終了しやすい。

設定はApplyだけでなくAnalyze、Archiveなど共通runnerを使うAI commandへ伝播する。したがって、Applyだけに別のdeadlineを追加せず、既存の単一設定の既定値を3倍の10,800秒へ変更し、仕様・設定例・利用者向け文書・埋め込みskillを同じ契約へ揃える。

## Proposed Solution

1. `DEFAULT_COMMAND_MAX_RUNTIME_SECS`を3,600秒から10,800秒へ変更する。
2. 未設定時のgetter、command queue wiring、各operation runnerが同じ10,800秒を受け取る既存経路を維持する。
3. canonical configuration specとcommand-queue specの既定値を10,800秒へ変更する。
4. generated JSONC templates、configuration guide、Rust API comments、`cflx-apply`と`cflx-proposal` guidanceを3時間の既定値へ同期する。
5. 既存の明示設定、`0`無効化、custom > project > global precedence、inactivity timeoutからの独立性、runtime expiry後のprocess-group cleanupとretry抑止を変更しない。

仕様と実装は同じ既定値を公開しなければ不整合になるため、単一のhybrid changeとして扱う。

## Acceptance Criteria

- `command_max_runtime_secs`が全設定layerで省略された場合、effective valueは10,800秒となる。
- 明示された正の値は10,800秒より優先され、`0`は引き続きabsolute runtime limitを無効化する。
- custom > project > globalのmerge precedenceと、上位layerの省略時に下位値を保持する挙動を維持する。
- stdout/stderr activityはabsolute deadlineを延長せず、inactivity timeoutとexplicit cancellationは独立して機能する。
- runtime-limit expiryは引き続きowned process groupをgraceful-then-forceful pathで終了し、同一invocationのretry admissionを閉じる。
- generated configuration examples、configuration guide、Rust comments、embedded `cflx-apply`/`cflx-proposal` sourceに、既定値10,800秒または3時間が一貫して記載される。
- 3,600という数値を別用途で使うtest fixtureや時刻換算は、この既定値変更だけを理由に変更しない。

## Explicit Completion Conditions

- `src/config/defaults.rs`の既定値と、`src/config/mod.rs`のdefault characterization testsが10,800秒で一致する。
- `openspec/specs/configuration/spec.md`と`openspec/specs/command-queue/spec.md`へ本changeのdeltaが昇格可能である。
- `src/templates.rs`の全generated example、`docs/guides/CONFIG.md`、`src/config/types.rs`、関連embedded skill sourceが新しい既定値を案内する。
- `cargo test --locked config`が、default、明示値、`0`無効化、precedence、inactivity timeoutとの独立性を検証して成功する。
- `cargo test --locked --test install_skills_test`がembedded skill contractを検証して成功する。
- tracked pre-commit hookはproposal-only commitでRust gateを選択しない。実装時のRust変更では既存path-scoped `rustfmt`と`clippy`がworkspace全体を検証する。

## Out of Scope

- `command_max_runtime_secs`の名前、型、設定階層、`0` semanticsの変更。
- Apply専用timeoutやoperation別timeoutの追加。
- command queue retry、process cleanup、WIP preservation、interruption classificationのロジック変更。
- 明示的に3,600秒を設定している利用者設定のmigrationまたは上書き。
- inactivity timeoutの既定値変更。
