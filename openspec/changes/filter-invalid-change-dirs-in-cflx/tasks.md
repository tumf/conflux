## Implementation Tasks

- [x] 1.1 `skills/cflx-workflow/scripts/cflx.py` の `OpenSpecManager` に `_is_valid_change_dir(change_dir: Path) -> bool` メソッドを追加（`proposal.md` の存在を必須条件とする）(verification: `python3 skills/cflx-workflow/scripts/cflx.py list` で `{specs` だけのゴミディレクトリが表示されないこと)
- [x] 1.2 `skills/cflx-workflow/scripts/cflx.py` の `list_changes()` で `_is_valid_change_dir()` を使い invalid ディレクトリを除外し、stderr に警告を出す (verification: テスト `test_list_changes_ignores_invalid_dir`)
- [x] 1.3 `skills/cflx-workflow/scripts/cflx.py` の `_find_change_dir()` で `_is_valid_change_dir()` を使い invalid ディレクトリを返さない (verification: テスト `test_find_change_dir_ignores_invalid`)
- [x] 1.4 `skills/cflx-proposal/scripts/cflx.py` に同じ修正を適用 (verification: `python3 skills/cflx-proposal/scripts/cflx.py list` で invalid ディレクトリが表示されないこと)
- [x] 1.5 `skills/tests/` に `test_cflx_list_ignores_invalid_change_dirs.py` を追加（proposal.md のないディレクトリが除外されること、警告が出ること、正常ディレクトリは表示されること）(verification: `python3 -m pytest skills/tests/test_cflx_list_ignores_invalid_change_dirs.py`)

## Future Work

- Rust 側の `list_changes_native()` (`src/openspec.rs`) にも同様のガードを入れるか検討
- ゴミディレクトリの発生原因（AI エージェントのファイル操作ミス）の根本対策
