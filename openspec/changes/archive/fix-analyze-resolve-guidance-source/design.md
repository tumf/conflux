# Design: analyze / resolve の prompt source-of-truth を単一化する

## Context

`split-workflow-skills-by-operation` によって dedicated skills は導入されたが、analyze / resolve では fixed guidance が skill と Rust prompt builder の両方に残っている。これは source-of-truth の境界が曖昧な状態であり、変更時に drift を起こしやすい。

## Goal

- `cflx-analyze` と `cflx-resolve` を fixed guidance の唯一の authoritative source にする
- Rust 側 prompt builder は runtime context injection に限定する
- 経路差（通常 conflict path / sequential merge path）によって authoritative source が変わらないようにする

## Boundary Definition

### Skill-owned fixed guidance

`skills/cflx-analyze/SKILL.md` / `skills/cflx-resolve/SKILL.md` が持つべき内容:

- analyze: selection priority, selection rules, output contract
- resolve: safety constraints, merge conflict resolution rules, sequential merge protocol, commit message conventions, retry interpretation guidance

### Rust-owned runtime context

Rust prompt builder が持つべき内容:

- prelude: `load skills: cflx-analyze` / `load skills: cflx-resolve`
- change list, progress, dependency metadata
- VCS status, VCS log, conflict file list
- merge plan, target branch, worktree locations
- previous attempt history / continuation context

Rust 側は固定ルールの文章そのものを再定義してはならない。

## Design Rules

1. Rust prompt builder は operation-specific skill prelude を付与する
2. Rust prompt builder は runtime state を列挙する
3. fixed rules の列挙・手順・出力契約は skill 側にのみ置く
4. tests は「必要な context は残っているが fixed guidance の重複はない」ことを確認する

## Risk

### Risk: context を削りすぎて agent が必要情報を失う

Mitigation:
- ルールではなく state/context だけを残す
- normal conflict path と sequential merge path の両方にテストを付ける

### Risk: skill 側変更と Rust 側変更が再び分離する

Mitigation:
- duplicated phrase regression tests を追加する
- canonical spec で boundary を明文化する
