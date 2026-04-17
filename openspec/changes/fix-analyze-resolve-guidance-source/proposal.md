---
change_type: hybrid
priority: high
dependencies: []
references:
  - skills/cflx-analyze/SKILL.md
  - skills/cflx-resolve/SKILL.md
  - src/orchestration/selection.rs
  - src/parallel/conflict.rs
  - openspec/specs/agent-prompts/spec.md
  - openspec/changes/archive/split-workflow-skills-by-operation/specs/agent-prompts/spec.md
---

# Change: analyze / resolve の fixed guidance source を単一化する

**Change Type**: hybrid

## Problem / Context

現在の Conflux では `cflx-analyze` と `cflx-resolve` が導入済みであり、canonical spec でも dedicated analyze / resolve skills が fixed operation guidance の primary source であることが要求されている。しかし実装を確認すると、analyze は `skills/cflx-analyze/SKILL.md` と `src/orchestration/selection.rs` の両方が選択基準と出力契約を持ち、resolve は `skills/cflx-resolve/SKILL.md` に加えて `src/parallel/conflict.rs` の経路ごとに固定手順が多く残っている。

この状態では、どの規則が authoritative source なのかが不明確になる。将来 skill だけ、または Rust 側だけを更新した場合に prompt drift が発生し、agent が矛盾した instructions を同時に読む危険がある。特に resolve は通常 conflict path と sequential merge path でコードに残っている fixed guidance の量が異なり、経路ごとに支配源が変わるため、保守・検証・デバッグの難易度が高い。

## Proposed Solution

- analyze / resolve について、fixed guidance の single source of truth を dedicated skill (`cflx-analyze`, `cflx-resolve`) へ明示的に統一する
- `src/orchestration/selection.rs` からは analyze の選択基準・出力契約などの固定 guidance を除去し、Rust 側は change list / progress / dependency metadata などの variable context 注入だけを担う
- `src/parallel/conflict.rs` からは resolve の safety rules / sequential merge protocol / commit convention などの固定 guidance を除去し、Rust 側は VCS 状態・競合ファイル・merge plan・retry history などの variable context 注入だけを担う
- analyze / resolve の prompt assembly tests を追加・更新し、Rust 側 prompt に fixed guidance が再侵入しないことを検証する
- canonical spec と docs を補足し、single-source boundary を「skill = fixed rules, Rust = runtime context」として明文化する

## Acceptance Criteria

- analyze prompt は `load skills: cflx-analyze` を含み、Rust 側 prompt から selection priority / selection rules / output contract の重複記述が除去される
- resolve prompt は `load skills: cflx-resolve` を含み、Rust 側 prompt から safety constraints / sequential merge protocol / commit message convention などの固定 guidance が除去される
- `skills/cflx-analyze/SKILL.md` と `skills/cflx-resolve/SKILL.md` が analyze / resolve fixed guidance の primary source として一貫して機能し、経路によって authoritative source が変わらない
- analyze / resolve の prompt builder tests は dedicated skill prelude と variable-context-only injection を検証し、固定 guidance の二重定義を防ぐ
- canonical spec は「analyze / resolve の fixed guidance は skill、Rust は variable context」という境界を実装と一致した形で満たす

## Out of Scope

- apply / archive / acceptance / rejecting / cleanup-review の fixed guidance source 再設計
- `cflx-workflow` compatibility router の役割変更
- analyze / resolve 以外の operation に対する skill topology の追加変更
