---
change_type: implementation
priority: high
dependencies:
  - classify-acceptance-followup-routing
references:
  - src/main.rs
  - src/cli.rs
  - src/execution/archive.rs
  - src/orchestration/archive.rs
  - src/parallel/executor.rs
  - skills/cflx-archive/SKILL.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/cflx-proposal-validation/spec.md
---

# Change: archive validation mode と blocker reporting を canonical contract に揃える

**Change Type**: implementation

## Premise / Context

- 直近3時間の cflx 実行ログでは archive フェーズで `cflx openspec validate <id> --strict --evidence strict` が実行され、CLI が `--evidence` の許容値 `off|warn|error` と不一致のため validation failed を繰り返していた（`~/.local/state/cflx/logs/conflux-bda270b8/2026-04-29.log:433049-433053`）。
- canonical CLI/spec では `--evidence` の有効値は `off|warn|error` のみであり（`src/main.rs:946-956`, `openspec/specs/cflx-proposal-validation/spec.md:100-125`）、`strict` は invalid value である。
- archived change `align-archive-readiness-failure-reporting` は、archive failure が `not actually archived` 一般論へ潰れないよう root-cause surfacing と final error synthesis を既に扱っている（`openspec/changes/archive/2026-04-29-align-archive-readiness-failure-reporting/proposal.md:37-60`, `src/execution/archive.rs:694-720`）。
- したがって残っている未解決部分は、archive 実行系のどこかが unsupported evidence mode を生成している契約逸脱であり、failure-reporting 全体を再提案する必要はない。

## Requested Artifact

- implementation proposal for tracing and removing the remaining `--evidence strict` generation path in archive execution
- explicit contract alignment between archive guidance and native `cflx openspec validate`
- regression coverage proving archive-side validation never emits unsupported evidence mode names

## Problem / Context

archive phase では proposal/spec を strict validate しつつ canonical spec 更新まで確認する必要があるが、現在の実行経路の一部は `--evidence strict` という存在しない mode を使用している。そのため validation 自体が deterministic に失敗し、archive failure の試行がノイズ化する。

一方で root-cause preserving archive failure message そのものは archived `align-archive-readiness-failure-reporting` で既に整理済みであり、今回の問題はその上流で「そもそも無効な validator invocation が作られている」点に絞られる。

## Proposed Solution

archive validation command generation と skill guidance を native CLI contract に再同期し、unsupported evidence mode source を潰す。

- archive から native validator を呼ぶすべての経路で `--evidence warn|error` のみを使い、`strict` を mode 名として生成しないよう統一する。
- `skills/cflx-archive/SKILL.md` と archive prompt/runner guidance を更新し、archive authoring/verification で使う validator invocation を native CLI contract に一致させる。
- `--evidence strict` がどの builder/path から出ているかをテストで固定し、今後 CLI enum とズレても archive path からは出ないようにする。
- archived `align-archive-readiness-failure-reporting` が導入した root-cause preserving failure behaviorを非退行条件として保持する。

## Acceptance Criteria

- archive 実行系は `--evidence strict` を生成・実行しない。
- archive validation で evidence mode を使う場合、許容値 `off|warn|error` のいずれかだけが使われる。
- `skills/cflx-archive/SKILL.md` と関連 guidance は native validator enum 契約に一致し、存在しない mode を案内しない。
- regression tests が validation mode mismatch を再現し、再発を防ぐ。
- archived `align-archive-readiness-failure-reporting` が保証した root-cause preserving archive failure message は今回の変更で退行しない。

## Explicit Completion Conditions

- `openspec/specs/cflx-proposal-validation/spec.md` と必要なら `openspec/specs/agent-prompts/spec.md` に archive-side validation mode 契約が canonical rule として記述されている。
- `src/main.rs`, `src/cli.rs`, `src/orchestration/archive.rs`, `src/parallel/executor.rs` など evidence mode を生成・伝播しうる経路のどこを確認・修正するかが tasks に明記されている。
- `skills/cflx-archive/SKILL.md` の guidance 更新 task が含まれている。
- invalid evidence mode non-regression と archive failure reporting 非退行を検証する tests が tasks に含まれている。
- `cflx openspec validate fix-archive-validation-blocker-reporting --strict --evidence warn` が成功する。

## Out of Scope

- archived `align-archive-readiness-failure-reporting` が既に扱った final error synthesis の再設計
- archive phase 全体の retry policy 再設計
- pre-commit hook 自体の無効化や緩和
- acceptance follow-up routing 全体の再設計
