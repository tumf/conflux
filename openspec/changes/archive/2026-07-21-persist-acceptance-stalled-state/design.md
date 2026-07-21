# Design: Workspace-local acceptance stalled evidence

## Context

Constitutionはnext actionをworkspace file/git stateから導出することを要求する。Reducer/UI stateだけのretry解除はmarkerが残るため不完全であり、markerの無条件削除は別origin blockerを破壊する。

## Decision

### Retry checkpoint

初回FAIL後、runtimeはprevious finding identities、semantic fingerprint、cycle countをnon-blocking workspace-local checkpointへatomicに保存する。Apply後またはacceptance再開前にcheckpointを読み、process restartでも初回FAILへresetしない。Stalled markerへ移行した時点でcheckpoint evidenceを引き継ぐ。

### Marker contract

既存`APPLY_BLOCKED/marker.md`を再利用し、machine-readable acceptance-owned sectionへ次を保存する。

- origin
- stable reason
- phase
- retry count
- normalized finding identitiesとsummary
- semantic progress result
- retained external blockers
- resumable
- recommended next action

Legacy/unknown markerはapply-generated相当として保守的に扱い、自動consumeしない。

### Routing

Workspace scanはcheckpointからretry contextを、markerからstalled observationを生成する。Serial/parallel ordinary dispatchはmarker存在中にapply、acceptance、archiveを開始しない。外部state削除で判断を変えない。

### Explicit retry

Explicit retryはmarker parse後、`origin=acceptance`かつ`resumable=true`の場合だけatomicにconsumeする。Reducer state clearとworkspace consumeは一つのretry preparation resultとして扱い、consume失敗時はdispatchしない。

## Alternatives Rejected

- Reducer stateだけで管理: Constitution違反。
- Markerを常に削除: unrelated apply blockerを破壊する。
- Acceptance専用DB: workspace-local routing lawに反する。
- 新しいmarker directory: 既存workspace scan contractを重複させる。
