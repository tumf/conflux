# Design: acceptance state ignore を workspace local exclude へ移す

## Overview

`.cflx/acceptance-state.json` は parallel acceptance/archive 再開制御に必要な durable state だが、性質としては workspace local な内部運用ファイルであり、repository-tracked ignore に依存すべきではない。

この変更では、ignore 責務を `.gitignore` から各 workspace の実効 Git exclude (`info/exclude`) に移す。

## Goals

- acceptance state が dirty worktree 判定へ混入しないようにする
- repo tracked な ignore 変更を不要にする
- worktree / 通常 repo の両方で同じ挙動にする
- exclude 登録を idempotent にする

## Non-Goals

- `.cflx/` 全体の包括 ignore
- acceptance state persistence location の変更
- acceptance/archive state machine の変更

## Proposed Design

### Effective exclude target

ignore rule は repo root の `.gitignore` ではなく、workspace から見た実効 Git dir 配下の `info/exclude` に書く。

worktree では `.git` がファイルになりうるため、単純に `<workspace>/.git/info/exclude` を前提にせず、Git が解決する実効 git dir を元に exclude path を求める。

### Registration timing

次のどちらかで登録を保証する:

- acceptance state 初回保存直前
- workspace 作成直後

最小変更としては acceptance state 保存前の保証が有力である。これなら既存 lifecycle をほぼ変えず、state file が生成される全経路をカバーしやすい。

### Idempotency

exclude 追加処理は:

- `info/exclude` がなければ作成する
- 既存内容に `.cflx/acceptance-state.json` があれば追記しない
- 既存行を壊さない

## Test Strategy

1. 通常 repo で実効 `info/exclude` に rule が追加される単体テスト
2. git worktree で実効 `info/exclude` に rule が追加される単体テスト
3. 同じ rule を複数回登録しても重複しないテスト
4. acceptance state 保存後に `git status --porcelain` へ `.cflx/acceptance-state.json` が出ない回帰テスト

## Spec Impact

canonical spec では、internal acceptance state artifact は repo tracked ignore に依存せず workspace local exclude で扱うことを明示する。
