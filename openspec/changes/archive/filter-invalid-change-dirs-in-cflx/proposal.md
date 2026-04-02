---
change_type: implementation
priority: medium
dependencies: []
references:
  - skills/cflx-workflow/scripts/cflx.py
  - skills/cflx-proposal/scripts/cflx.py
  - skills/tests/test_cflx_workflow_no_delta_marker.py
---
# Change: cflx.py の change 列挙で無効ディレクトリを除外する

**Change Type**: implementation

## Why

`openspec/changes/` 配下に archive 後の壊れた残骸ディレクトリ（例: `{specs` だけが残ったディレクトリ）が存在すると、`cflx.py list` が active change として誤表示する。
原因は `list_changes()` / `_find_change_dir()` がディレクトリの存在のみで change と判定しており、`proposal.md` の有無を確認していないため。

## What Changes

- `skills/cflx-workflow/scripts/cflx.py` と `skills/cflx-proposal/scripts/cflx.py` の `OpenSpecManager` に valid change directory の判定ロジックを追加
- `list_changes()` が valid でないディレクトリを列挙しないようにする
- `_find_change_dir()` が valid でないディレクトリを返さないようにする
- invalid ディレクトリを検出した場合、stderr に警告を出す
- テストを追加

## Impact

- Affected specs: cflx-proposal-validation
- Affected code: `skills/cflx-workflow/scripts/cflx.py`, `skills/cflx-proposal/scripts/cflx.py`, `skills/tests/`
