# Design: operation prompt surfaces を dedicated skills へ移す

## Context

`split-workflow-skills-by-operation` は、従来 `cflx-workflow` に集約されていた workflow 系 operation だけでなく、Rust 実装へ inline で埋め込まれている analyze / resolve prompt も含めて、Conflux の operation prompt surface 全体を dedicated skills へ寄せる変更である。

同時に、完了済みの `replace-cflx-py-with-native-cli` が導入した native CLI 方針を前提とし、bundled skill 配布物へ `scripts/cflx.py` を再導入しない。

## Goals

- analyze / apply / rejecting / cleanup-review / accept / archive / resolve それぞれに dedicated skill を持たせる
- Rust 側の prompt source を operation ごとの固定 guidance と可変コンテキストに分離する
- `cflx-workflow` を legacy compatibility router として維持する
- bundled skill 配布物は native CLI 前提を維持し、`scripts/cflx.py` を再導入しない

## Non-Goals

- workflow phases のさらなる細分化
- `cflx-workflow` へ analyze / resolve の legacy compatibility まで背負わせること
- `cflx-proposal` の役割変更

## Skill Topology

### Dedicated skills

- `cflx-analyze`
- `cflx-apply`
- `cflx-rejecting`
- `cflx-cleanup-review`
- `cflx-accept`
- `cflx-archive`
- `cflx-resolve`

これらは各 operation の fixed guidance と必要な auxiliary references を持つ。新規 dedicated skills は `scripts/cflx.py` を持たない。

### Compatibility router

- `cflx-workflow`

`cflx-workflow` は apply / rejecting / cleanup-review / accept / archive について、legacy prompt が `load skills: cflx-workflow` しか読まない場合でも従来同等の guidance を提供する self-contained router として残す。



## Prompt Ownership Model

### Fixed guidance belongs to skills

各 operation に共通で不変な rules / output contract / critical constraints は dedicated skill 側が primary source of truth になる。

対象:
- analyze の依存関係選択ルールと出力契約
- apply の implementation guidance
- rejecting の verdict contract
- cleanup-review の clean handoff contract
- accept の operation identity と補助 guidance
- archive の archive-specific guidance
- resolve の conflict resolution rules と retry continuation guidance

### Variable context belongs to Rust

Rust 側は実行時にしか分からない可変コンテキストを prompt に注入する。

例:
- analyze: change list, progress, dependency candidates
- resolve: conflicting files, revisions, target branch, VCS status/log, previous attempts
- acceptance: change_id, paths, diff context, prior findings
- archive/apply/cleanup-review/rejecting: change_id, paths, history context

## Acceptance Single-Source Constraint

`cflx-accept` を追加しても、固定 acceptance procedure の単一ソースは引き続き `.opencode/commands/cflx-accept.md` とする。

`cflx-accept` は operation identity と scoped guidance を提供できるが、固定 acceptance checklist / verdict workflow を command template から奪ってはならない。

## Packaging Contract

Bundled skill packaging / install-skills の契約は以下とする。

- dedicated skills は自分の `SKILL.md` と必要な auxiliary references を持つ
- bundled skill 配布物へ `scripts/cflx.py` は含めない
- legacy prompts that load only `cflx-workflow` remain supported
- new orchestrator-generated prompts depend on dedicated skills, not on a helper-script surface

## Migration Plan

1. dedicated skill directories を追加する
2. workflow 系 prompt builder を dedicated skill prelude へ切り替える
3. analyze / resolve の inline prompt body を dedicated skill 前提へ整理する
4. embedded skill / install-skills packaging を更新する
5. docs / specs を新 topology に合わせる

## Risks and Mitigations

### Risk: legacy compatibility regression

`cflx-workflow` を薄くしすぎると旧 prompt が壊れる。

Mitigation:
- apply / rejecting / cleanup-review / accept / archive の legacy-equivalent guidance を router に残す
- native CLI 前提の command guidance を router 内へ明示し、helper script なしでも互換 surface を維持する

### Risk: analyze / resolve migration scope expands too much

analyze / resolve は Rust inline prompt からの移行なので、単なる skill split より広い変更になる。

Mitigation:
- fixed guidance と variable context の境界を先に定義する
- acceptance criteria と tests を operation ごとに明示する
