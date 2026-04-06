# Design: managed worktree post-apply cleanup review

## Context

Conflux の parallel mode では apply が change ごとの git worktree で実行される。これにより base dirty の責任と apply dirty の責任は分離されるが、現行実装では apply 完了状態が「タスク完了 + Apply commit」に寄っており、「acceptance に handoff できる clean workspace」であることは保証していない。

acceptance には dirty worktree fail の安全網があるが、managed worktree apply が残した dirty を acceptance/archive 側で拾うのは遅く、ループ効率が悪い。かといって単純な自動 stage/commit は unsafe である。

## Goals

- managed worktree apply の dirty handoff を acceptance 前に処理する
- blind staging を避ける
- coarse-grained state machine は増やさない
- existing apply/accept/archive responsibilities との整合を保つ

## Non-Goals

- serial mode の適用
- archive 後 resolve の置き換え
- operator-facing new status の導入

## Proposed Design

### 1. cleanup review は apply の内部サブステップ

`execute_apply_in_workspace()` は apply loop 成功後、最終 `Apply:` 完了前に worktree clean を確認する。

- clean の場合: 従来どおり apply 完了を確定
- dirty の場合: cleanup review operation を起動

このため外から見える大きな状態は `applying` のままでよい。

### 2. cleanup review は cflx-workflow の新 operation

新しい skill を増やさず、`cflx-workflow` に operation を追加する。

理由:
- apply / rejecting / accept / archive と同じ orchestrator-managed operation である
- prompt routing は既存 `load skills: cflx-workflow` パターンに乗る
- change_id, tasks/proposal path, worktree context を共通運用できる

想定 operation 名:
- `cleanup-review`
- prompt prelude 例: `Cleanup review id:<change_id>`

### 3. cleanup review の責務

cleanup review agent は以下のみを行う:

- dirty file set を確認する
- acceptance handoff に含めてよい差分と、除外/巻き戻しすべき差分を区別する
- blind `git add -A` を行わない
- 判断不能・危険・秘密情報・一時生成物・スコープ外差分が残る場合は blocked を返す
- 成功時のみ clean handoff-ready 状態を作る

### 4. verdict contract

cleanup review は machine-readable final marker を 1 つだけ返す。

候補:
- `CLEANUP_REVIEW: CLEAN`
- `CLEANUP_REVIEW: BLOCKED`

意味:
- `CLEAN`: worktree は acceptance handoff 可能
- `BLOCKED`: safe cleanup を完了できず、acceptance に進めない

### 5. apply completion gating

managed worktree apply では、durable apply-complete / acceptance-pending 相当の確定は cleanup review 後に行う。

つまり:
- apply loop success
- if dirty -> cleanup review success required
- その後に `Apply:` 完了状態を記録
- acceptance state pending を記録

### 6. failure behavior

cleanup review が blocked の場合:
- acceptance は開始しない
- archive にも進まない
- current run では apply 側失敗として扱う
- workspace は follow-up のため保持される

## Alternatives Considered

### A. apply failure に戻して通常 apply を再実行

不採用。タスクがすでに完了しているため、現行 apply loop は cleanup 専用再実行に向かず、dirty 解消責務も曖昧になる。

### B. acceptance の前処理に吸収

不採用。acceptance の責務が handoff hygiene まで拡張され、apply/accept の責務分離が悪化する。

### C. 新しい独立 skill を追加

不採用。orchestrator-managed operation としては `cflx-workflow` 配下の operation 追加で十分であり、skill 分割の利点が小さい。
