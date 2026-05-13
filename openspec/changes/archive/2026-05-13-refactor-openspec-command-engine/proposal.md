---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs:14
  - src/openspec_cmd.rs:487
  - src/openspec_cmd.rs:687
  - src/openspec_cmd.rs:776
  - src/openspec_cmd.rs:891
  - src/openspec_cmd.rs:1370
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/cflx-proposal-validation/spec.md
---

# OpenSpec コマンドエンジンの責務を分離する

**Change Type**: implementation

## Problem / Context

`src/openspec_cmd.rs` は 3299 行あり、spec promotion engine、change listing/showing、strict validation、archive 実行、dependency status、task validation、CLI rendering、テストが単一ファイルに集約されています。これは `cflx openspec validate` や archive の安全性に関わる中心モジュールであり、責務が密集しているため変更時の影響範囲が読みにくくなっています。

証拠:

- `src/openspec_cmd.rs:14` から spec promotion engine が始まる。
- `src/openspec_cmd.rs:487` から change validation が始まる。
- `src/openspec_cmd.rs:687` から spec delta validation が始まる。
- `src/openspec_cmd.rs:776` から archive 実行が始まる。
- `src/openspec_cmd.rs:891` から canonical spec update が始まる。
- `src/openspec_cmd.rs:1370` から task content validation が始まる。

## Proposed Solution

OpenSpec コマンドの外部 CLI 挙動を維持したまま、内部責務を小さなサブモジュールまたは helper 群へ分割します。初期段階では public entrypoint (`cmd_list`、`cmd_show`、`cmd_validate`、`cmd_archive`) を維持し、promotion、validation、archive、rendering、dependency status の内部境界を明確にします。

## Acceptance Criteria

- `cflx openspec list --specs`、`show`、`validate --strict`、`archive` の CLI 出力と exit code contract は維持される。
- strict validation の必須チェック（proposal/tasks/spec delta/scenario/change type）は維持される。
- archive 前 validation と promotion simulation の順序は変わらない。
- promotion engine の no-op rejection、MODIFIED/REMOVED target validation は維持される。
- `cargo fmt`、関連ユニットテスト、既定テストスイートが成功する。

## Explicit Completion Conditions

- `src/openspec_cmd.rs` または配下モジュールで、promotion、validation、archive、rendering の責務境界がコード上分かる構造になっている。
- 既存の `openspec_cmd` テストに加え、CLI entrypoint の characterization test または既存テストの維持により出力/validation contract が確認されている。
- `cflx openspec validate <既存の妥当な変更> --strict` に相当するテストまたは手動確認が成功する。

## Out of Scope

- OpenSpec ファイル形式の変更。
- strict validation rule の追加・削除。
- archive destination naming の変更。
- Python skill helper の置換。
