---
change_type: hybrid
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/main.rs
  - src/openspec.rs
  - src/embedded_skills.rs
  - skills/shared/cflx_spec_promotion.py
  - skills/cflx-proposal/SKILL.md
  - skills/cflx-workflow/SKILL.md
  - skills/cflx-run/SKILL.md
  - skills/README.md
  - openspec/changes/split-workflow-skills-by-operation/tasks.md
---

# Change: cflx.py helper を native CLI に置き換える

**Change Type**: hybrid

## Problem / Context

現在の Conflux skill 群は、proposal / workflow / run 系の OpenSpec 操作を skill-local な Python helper `scripts/cflx.py` に依存している。`cflx-proposal` と `cflx-workflow` には同名スクリプトが重複配置され、spec promotion の共有実装も `skills/shared/cflx_spec_promotion.py` に別管理されている。

この構成により、Conflux 本体に既に Rust 実装がある領域（change 列挙、archive orchestration、CLI dispatch）と、skill 向け Python helper の責務が二重化している。また、bundled skills の配布・README・検証手順が Python runtime を前提にしており、ユーザーが求める「cflx.py を廃止して skill 配布は維持する」方向とずれている。

加えて、進行中の `split-workflow-skills-by-operation` proposal を含む active artifacts でも `cflx.py` ベースの verification 手順が残っているため、native CLI 化では skill 本文だけでなく、現在の開発導線で使われる active documentation / proposal references も同時に移行する必要がある。

## Proposed Solution

- Conflux 本体に namespaced な `cflx openspec` subcommand 群を追加し、`list` / `show` / `validate` / `archive` を native Rust 実装として提供する
- 既存 `cflx.py` が担っている strict validation・JSON / deltas-only 出力・archive promotion・evidence mode を、既存 Rust modules と新規 native helpers へ移植する
- `skills/shared/cflx_spec_promotion.py` の spec promotion engine を Rust へ移し、archive path から Python runtime 依存を除去する
- bundled skill source / references / README / active proposal guidance を `python3 "<SKILL_ROOT>/scripts/cflx.py" ...` から `cflx openspec ...` へ置換する
- embedded skill packaging と `install-skills` 配布物から `scripts/cflx.py` を除去し、skill 配布は継続しつつ Python helper 配布だけを廃止する

## Acceptance Criteria

- `cflx openspec list`, `show`, `validate`, `archive` が追加され、現在 skill が必要とする flag surface（`--specs`, `--json`, `--deltas-only`, `--strict`, `--evidence`, `--yes`, `--skip-specs`）を native CLI で提供する
- proposal / workflow / run 系 skill source と skill-facing docs は `cflx openspec ...` を参照し、active workflow guidance に `scripts/cflx.py` 依存が残らない
- Conflux の strict proposal validation と archive promotion は Python runtime なしで実行できる
- bundled skill installation 後の配布物に `scripts/cflx.py` が含まれず、`install-skills` 関連テストは新構成に追随する
- archived な過去提案を除く active repo artifacts について、現在の開発導線で参照される `cflx.py` 呼び出しは native CLI 呼び出しへ移行される

## Out of Scope

- `cflx-proposal` / `cflx-workflow` / `cflx-run` 自体の配布停止
- workflow skill の operation 分割方針そのものの再設計
- archive 配下の historical proposal / tasks / docs を一括で書き換えるドキュメント掃除
