# Design: durable acceptance state を worktree 外へ移す

## Overview

parallel acceptance resume safety のため durable acceptance state は必要だが、worktree 内ファイルとして保存すると Git dirty 判定と merge readiness を汚染する。今回の要求は ignore 設定の調整ではなく、internal state artifact を worktree から排除することにある。

## Goals

- `.cflx/acceptance-state.json` を worktree 配下に生成しない
- durable acceptance semantics を維持する
- interrupted acceptance の resume / archive guard を壊さない
- merge 判定が Conflux 生成 artifact に影響されないようにする

## Non-Goals

- acceptance state model (`pending` / `running` / `passed` / `failed`) の廃止
- archive guard 要件の削除
- merge dirty 判定そのものの一般的緩和

## Proposed Design

### External persistence

acceptance state は worktree path を入力にする API を維持してもよいが、実際の保存先は worktree 外の Conflux 管理領域へ移す。

推奨保存先は `~/.local/state/cflx/acceptance-state/` 配下とする。既存の cflx runtime logs が `~/.local/state/cflx/` 配下に置かれている運用と整合し、Git worktree の dirty 判定から完全に分離できるためである。

最低限必要な保持情報:

- state
- revision
- updated_at
- workspace absolute path
- change_id

### Storage key

resume/archive guard で同じ workspace を再識別できる必要があるため、保存 key は workspace absolute path を主キーに固定する。change_id は補助情報として保持し、同一 change_id の再作成や branch 再利用時も path 単位で区別する。

revision は payload に保持し、`passed` でも current revision と不一致なら archive を解放しない。

### Cleanup / invalidation

- archive 完了後は対応する acceptance state を削除する
- workspace cleanup 完了後も state を削除または無効化する
- resume 時に state は存在しても revision mismatch なら stale として扱い、`passed` と見なさない

### Behavioral compatibility

以下の lifecycle は現行のまま維持する:

- apply 完了 → `pending`
- acceptance 開始 → `running`
- PASS → `passed`
- FAIL/BLOCKED/command failure/未完了再開 → `failed` または incomplete 扱い

差分は保存先だけであり、worktree には state file を残さない。

## Test Strategy

1. 新 persistence の roundtrip テスト
2. interrupted acceptance resume テスト
3. archive guard の durable pass requirement 維持テスト
4. worktree 配下に `.cflx/acceptance-state.json` が生成されない回帰テスト
5. merge readiness が internal artifact で dirty にならないテスト

## Spec Impact

parallel execution spec では durable acceptance state を要求しつつ、その artifact を worktree 内へ書かないことを明示する。
