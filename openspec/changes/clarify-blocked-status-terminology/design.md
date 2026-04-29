# Design: blocked terminology separation

## Premise

Conflux には少なくとも三つの異なる blocker 系事象が存在する。

1. dependency analysis が queued change をまだ dispatch できない queue wait
2. apply / rejecting review が change 自体を閉じずに一時停止させる resumable hold
3. acceptance が implementation blocker を返し follow-up routing を要求する verdict

これらを同じ `blocked` 語彙へ寄せると、scheduler semantics、failed-change tracking、resume policy、frontend display contract が混線する。

## Canonical Taxonomy

### `blocked` / `dependency-blocked`

- 意味: dependency wait
- owner: reducer wait reason / scheduler
- trigger: unresolved dependency, merge-wait dependency など dispatch 不可の queue-side condition
- lifecycle: queued のまま待機
- frontend contract: `blocked` は dependency wait のみを表す
- spec note: canonical spec prose では曖昧さ回避のため `dependency-blocked` と呼び、user-facing short label は `blocked` とする

### `stalled`

- 意味: apply/rejecting 由来の resumable hold
- owner: runtime activity / non-terminal hold state
- trigger: permission auto-reject、追加情報待ち、review hold、環境修復後に再開可能な apply-side blocker
- lifecycle: worktree / WIP / progress / reason metadata を保持して再開可能
- frontend contract: dependency blocked と同じ label にしない

### `gated` / `acceptance-gated`

- 意味: acceptance verdict / acceptance follow-up 由来の gate failure observation
- owner: acceptance parser と follow-up routing
- trigger: acceptance が blocker を検出し、repo remediation または blocker-only hold を要求する
- lifecycle: reroute 先は proposal/implementation に依存するが、観測語彙は dependency blocked / stalled と区別する
- frontend contract: logs/events/status surfaces が `gated` を distinguish できる
- spec note: canonical spec prose では曖昧さ回避のため `acceptance-gated` と呼び、user-facing short label は `gated` とする

## State Ownership Mapping

- `blocked`: `WaitState` 相当の queue-side reason
- `stalled`: `ActivityState` または同等の resumable non-terminal hold
- `gated` (`acceptance-gated`): acceptance result / event reason / transient display state

重要なのは、三者を同一 enum variant に押し込むことではなく、**user-facing semantics と reducer/frontend contract を分離すること** である。

## Active Proposal Alignment

### `classify-acceptance-followup-routing`

この active change は acceptance follow-up の blocker-only case を apply へ戻さず、resumable hold (`WorkspaceStatus::Blocked`) を経由させる前提を持つ。`clarify-blocked-status-terminology` ではこの hold を canonical に `stalled` として扱うため、acceptance follow-up 側の「blocked」は queue-side dependency wait ではなく apply-side resumable hold として読まれるべきである。

移行順序は以下を canonical とする。

1. queue-side wait reason を `dependency-blocked` / `blocked` に固定する。
2. acceptance gate observation を `acceptance-gated` / `gated` としてイベント化する。
3. apply-side resumable hold (`WorkspaceStatus::Blocked`) を display semantics 上 `stalled` として扱う。

この順序により、active proposal 間で `blocked` が指す責務境界（queue wait か apply hold か）を判別可能にし、実装 agent が vocabulary を誤用しないようにする。

### 補足: `separate-apply-block-from-reject` 依存の整理

`separate-apply-block-from-reject` proposal は本ブランチ上に存在しないため、本 change では依存列挙から除外し、`classify-acceptance-followup-routing` を active dependency として扱う。apply-side resumable hold の vocabulary は当該 change が `WorkspaceStatus::Blocked` を使用していても、core/frontend contract 上は `stalled` として取り扱う。

## Migration / Verification Notes

- reducer tests は queue wait と resumable hold を distinct state として確認する必要がある。
- frontend mapping tests は 3 種類の status を collapse しないことを確認する必要がある。
- active proposals が未実装のまま並存する期間は、proposal/design 側で canonical taxonomy を明記し、実装 agent が旧 wording を新 wording へ置換してよいことを repo evidence として残す。
