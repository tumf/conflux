---
change_type: spec-only
priority: high
dependencies:
  - clarify-orchestration-hierarchy
references:
  - openspec/specs/frontend-abstraction/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/tui-resolve-queue/spec.md
  - openspec/specs/web-monitoring/spec.md
---

# Change: Core / Frontend 責務境界の仕様明確化

**Change Type**: spec-only

## Why

現在の仕様には以下の矛盾・曖昧さがある:

1. `frontend-abstraction/spec.md` は EventSink による **イベント配送の抽象化** だけを定義しており、Core / Frontend の **状態所有の境界** が未定義
2. `orchestration-state/spec.md` は「consumers never own an independent lifecycle copy」と述べるが、TUI 仕様が resolve queue / resolve serialization フラグを TUI-local state として記述しており矛盾
3. Web 側は「snapshot derives from the shared orchestration state」と一貫しているが、TUI 側は「shared state を読む」「local state も持つ」「resolve queue も持つ」が混在
4. Frontend が持ってよい「表示キャッシュ」と持ってはいけない「lifecycle state」の境界が不明確で、実装者が TUI に lifecycle state を追加してしまう原因になっている

## What Changes

### `frontend-abstraction/spec.md` — Core / Frontend 責務境界の総則を追加
- Core が所有する状態（Change lifecycle, resolve queue, execution state, display status の正規ソース）を定義
- Frontend が所有してよい状態（cursor, focus, panel, selection, popup, sort, filter, render cache, transport/session state）を定義
- Frontend が持ってはいけない状態（Change lifecycle の真実, resolve queue の真実, merge/resolve serialization）を明記

### `orchestration-state/spec.md` — resolve queue / serialization が Core 所有であることを再確認
- 既存の「Resolve Wait Queue Ownership」要件と新しい境界定義の整合を確認する注記

## Impact

- Affected specs: `frontend-abstraction`, `orchestration-state`
- Affected code: なし（spec-only）
- この仕様修正は `clarify-orchestration-hierarchy` と合わせて、is_resolving の誤用防止の根拠となる

## Acceptance Criteria

- Core が所有する状態と Frontend が所有してよい状態が `frontend-abstraction/spec.md` に明記されている
- Frontend が持ってはいけない状態が明記されている
- 既存の `orchestration-state` の「consumers never own an independent lifecycle copy」と矛盾しない

## Out of Scope

- TUI / Web の既存コードのリファクタリング
- `tui-architecture/spec.md` や `tui-resolve-queue/spec.md` 内の TUI-local 表現の書き換え（Future Work）
