---
change_type: hybrid
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/cflx-proposal-validation/spec.md
---

# Hide archived changes from `cflx openspec list`

**Change Type**: hybrid

## Premise / Context

- ユーザ要望は `cflx openspec list` で archived change を出さないことだった。
- 現状の native 実装は `OpenSpecManager::list_changes()` で active changes に加えて `openspec/changes/archive/` 配下も走査し、`cmd_list()` で `[ARCHIVED]` として表示している。
- 既存 canonical spec は `cflx openspec list --specs`、`show`、`validate`、`archive` の native surface を規定しているが、`list` が archived change を一覧表示すべきかは明示していない。
- `show` 系は archived change 解決を前提とする別要件があり、一覧非表示と archived detail 解決は切り分けて扱う方が既存フロー互換性を保ちやすい。

## Problem / Context

- `cflx openspec list` は change 候補の確認や proposal workflow の入口として使われるが、archive 済み change まで同列に表示されると pending/active change の探索ノイズが増える。
- 現状は `src/openspec_cmd.rs` の `list_changes()` が archive 配下を別途列挙し、`cmd_list()` が `[ARCHIVED]` を含む human-readable list を出力している。
- 一方で archived change を参照する needs は `cflx openspec show <change-id>` や TUI editor launch など個別フローで満たせるため、一覧表示に archived entry を含め続ける必然性は低い。

## Proposed Solution

- `cflx openspec list` の human-readable change list は active change のみを列挙し、`openspec/changes/archive/` 配下の archived change entry を含めない。
- 変更対象は `src/openspec_cmd.rs` の list path に限定し、`show` の archived resolution は維持する。
- canonical CLI spec に「list excludes archived changes」を明示し、archived change の詳細参照は `show` で継続可能なことを scenario で固定する。
- proposal validation と focused tests が active/archived の分離を回帰保護するよう、実装タスクに verification を紐付ける。

## Acceptance Criteria

- `cflx openspec list` は `openspec/changes/<change-id>` の active change のみを表示する。
- `openspec/changes/archive/<change-id>` または date-prefixed archive entry は `cflx openspec list` の change 一覧に表示されない。
- `cflx openspec show <change-id>` は archived change を引き続き解決できる。
- `src/openspec_cmd.rs` に対する focused tests と `cflx openspec validate hide-archived-openspec-list --strict` が通る。

## Out of Scope

- archived change を detail 系 API / TUI / editor launch から参照できるかどうかの仕様変更
- `cflx openspec list --specs` や JSON/detail 出力フォーマットの全面再設計
- archive 済み change を別コマンドで列挙する新機能追加
