---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/server/api/control.rs
  - src/server/api/test_support.rs
  - openspec/specs/code-maintenance/spec.md
---

# server control API の責務分割

**Change Type**: implementation

## Problem / Context

`src/server/api/control.rs` は 1,200 行超の単一ファイルに、change selection、global control、stats/logs、stop/dequeue、認証付き route テストが同居している。特にテスト領域だけでも `test_stats_and_logs_endpoints_require_auth`、`test_toggle_change_selection_emits_change_update_without_waiting_full_state`、`test_stop_and_dequeue_change_deselects_and_returns_ok` など多数の API 経路を同一ファイルで保持しており、制御経路ごとの変更影響範囲が見えにくい。

証拠:

- `src/server/api/control.rs:74` 以降に change selection toggle の handler がある。
- `src/server/api/control.rs:668` 以降に認証・選択・run/stop/dequeue の async route tests がまとまっている。
- `src/server/api/control.rs:1160` 以降に stop/dequeue 系テストがあり、同一ファイル内の他 control API と密結合している。

## Proposed Solution

外部 API、JSON schema、認証要件、ログ/DB への副作用を変えずに、control API の内部実装を責務別サブモジュールへ分割する。先に現在の route 応答と状態更新を characterization test で固定し、その後に handler、helper、test fixture を小さく移動する。

## Acceptance Criteria

- 既存の `/api/v1/control` と project control API の route、HTTP status、response body は変更されない。
- change selection、global run、stop/dequeue、stats/logs の代表テストがリファクタ前後で同じ結果を返す。
- WebSocket への `RemoteStateUpdate::ChangeUpdate` 発火条件は変更されない。
- `cargo test server::api::control` と関連する server API テストが成功する。
- 公開 CLI/API、設定ファイル、永続 state の形式は変更されない。

## Explicit Completion Conditions

- `src/server/api/control.rs` または配下モジュールが責務別に分割され、各 handler の公開 route 登録は既存と同じままである。
- Characterization test が route status、response body、selection state、stop/dequeue 結果を固定している。
- `cargo test server::api::control` が成功し、必要に応じて `cargo test server::api` でも後退がない。

## Out of Scope

- API schema の変更。
- 認証方式の変更。
- WebUI 表示や UX の変更。
- DB schema の変更。
