---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/specs/parallel-merge/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/orchestration/state.rs
---

# Update: Canonicalize parallel merge deferred contract

**Change Type**: hybrid

## Premise / Context

- ユーザは `openspec/specs` の矛盾整理に加えて、仕様通り未実装の箇所も修正したいと求めている
- セッション中の合意として、parallel merge では「resolve 優先」「dirty base は常に manual wait」「文字列解析による auto-resumable 判定をやめる」「pending merge task 中は scheduler を終了しない」を全部採用する
- `openspec/specs/parallel-merge/spec.md` には同名 Requirement の重複があり、canonical rule が 1 箇所にまとまっていない
- 実装では `src/parallel/merge.rs` と `src/parallel/queue_state.rs` が依然として `reason.contains("Resolve in progress")` による auto-resumable 判定を行っており、採用済み方針と一致していない
- 一方で `src/parallel/orchestration.rs` は `pending_merge_count` を scheduler exit 条件に含めており、仕様の一部はすでに実装されている

## Problem / Context

parallel merge deferred contract が canonical spec と実装の両方で分裂している。

特に以下が問題である。

1. `parallel-merge/spec.md` に同名 Requirement が重複し、どの版が正本か不明である
2. `MergeDeferred` の auto-resumable 判定が文字列 reason 依存のままで、仕様として不安定である
3. `parallel-execution` / `orchestration-state` / `parallel-merge` の wait state 契約が複数ファイルに跨っており、`MergeWait` と `ResolveWait` の使い分けが読み取りにくい
4. scheduler 側の pending merge task 継続条件は実装済みでも、spec 上の canonical contract が分散している

このままでは merge defer / resolve retry の保守で、spec と実装の両方が再びズレやすい。

## Proposed Solution

parallel merge deferred contract を 1 つの canonical rule に統合し、その rule に合わせて merge defer 実装を修正する。

具体的には以下を行う。

1. `parallel-merge/spec.md` の重複 Requirement を統合し、resolve 優先・dirty manual wait・pending merge scheduler wait を単一 contract として定義する
2. `parallel-execution` / `orchestration-state` の関連 requirement も canonical rule に合わせて参照関係を明確化する
3. `src/parallel/merge.rs` と `src/parallel/queue_state.rs` の auto-resumable 判定を reason 文字列解析から明示的な merge result contract へ置き換える
4. 既存テストを canonical rule に合わせて更新し、resolve active / dirty base / pending merge task の回帰を固定する

## Acceptance Criteria

1. `openspec/specs/parallel-merge/spec.md` 相当の canonical delta が、resolve 優先・dirty manual wait・pending merge task 継続条件を 1 つの contract として表現する
2. `parallel-execution` / `orchestration-state` の delta が `MergeWait` と `ResolveWait` の遷移責務を canonical rule と矛盾なく記述する
3. `src/parallel/merge.rs` と `src/parallel/queue_state.rs` から `reason.contains("Resolve in progress")` ベースの auto-resumable 判定が除去される
4. parallel merge 回帰テストが resolve active / dirty base / archive incomplete / pending merge scheduler wait をカバーする
5. `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-parallel-merge-deferred-contract --strict` が成功する
6. 実装時の品質確認として `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` が tasks に明記される

## Out of Scope

- serial mode の merge semantics 再設計
- conflict resolution retry policy 全体の redesign
- unrelated scheduler reanalysis rules の変更
