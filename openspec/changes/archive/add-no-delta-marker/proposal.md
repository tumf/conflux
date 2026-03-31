---
change_type: implementation
priority: medium
dependencies: []
references:
  - skills/cflx-workflow/scripts/cflx.py
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/specs/archive-promotion/spec.md
---

# Change: spec delta なしを `.no-delta` マーカーファイルで明示し archive を許容する

**Change Type**: implementation

## Why

`cflx.py archive` は内部で `validate --strict` を呼ぶが、strict モードは `specs/` ディレクトリにdelta が存在しない change を一律エラーにする。リファクタリング・ドキュメント修正・設定変更など仕様変更を伴わない change は spec delta が不要だが、現状では archive できない。一方、単純に strict チェックを緩めると spec delta の作り忘れを検出できなくなる。

## What Changes

- `specs/.no-delta` マーカーファイルを置くことで「この change は意図的に spec delta なし」と明示できるようにする
- strict validation は `.no-delta` が存在すれば spec delta なしを許容する
- `.no-delta` と spec delta ディレクトリが共存する場合は矛盾としてエラーにする
- `.no-delta` も spec delta もない場合は従来通りエラー（作り忘れ検出）

## Impact

- Affected specs: `cflx-proposal-validation`
- Affected code: `skills/cflx-workflow/scripts/cflx.py` (`_validate_change_dir`, `archive_change`)
