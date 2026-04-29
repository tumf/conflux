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
- `acceptance-blocked`: acceptance result / event reason / transient display state

重要なのは、三者を同一 enum variant に押し込むことではなく、**user-facing semantics と reducer/frontend contract を分離すること** である。

## Active Proposal Alignment

### `separate-apply-block-from-reject`

この active change は現在、apply/rejecting 側の resumable hold を `blocked` と呼んで実装・仕様化している。本 proposal ではまず dependency queue wait を `dependency-blocked`、acceptance gate failure を `acceptance-gated` / `gated` として固定し、そのうえで apply-side hold を canonical には `stalled` へ寄せる。したがって、当該 active change の runtime/display wording と spec delta には terminology refresh または明示的な移行注記が必要になる。

### `classify-acceptance-followup-routing`

この active change は acceptance follow-up から blocked/non-apply hold へ送る routing を扱う。本 proposal 後は、acceptance gate failure の観測語彙を `acceptance-gated` / `gated` に寄せ、最終 hold state が apply-side resumable hold なら `stalled` を使う。

## Migration / Verification Notes

- reducer tests は queue wait と resumable hold を distinct state として確認する必要がある。
- frontend mapping tests は 3 種類の status を collapse しないことを確認する必要がある。
- active proposals が未実装のまま並存する期間は、proposal/design 側で canonical taxonomy を明記し、実装 agent が旧 wording を新 wording へ置換してよいことを repo evidence として残す。
