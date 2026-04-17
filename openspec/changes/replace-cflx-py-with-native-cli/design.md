# Design: cflx.py helper を native CLI へ移す

## Goals

- skill 配布は維持したまま、skill-local Python helper `scripts/cflx.py` を廃止する
- proposal / workflow skill が必要とする OpenSpec 操作を `cflx` バイナリ自身で提供する
- strict validation と archive promotion の振る舞いを Python helper から Rust 実装へ移し、runtime dependency を単純化する

## Non-Goals

- skill architecture 自体を廃止すること
- historical archive artifacts の一括書き換え
- top-level に `cflx list` / `cflx show` などを追加して既存 command namespace を拡散すること

## Proposed CLI Surface

`cflx.py` の責務は namespaced subcommand に集約する。

```text
cflx openspec list [--specs]
cflx openspec show <change-id> [--json] [--deltas-only]
cflx openspec validate [change-id] [--strict] [--evidence off|warn|error]
cflx openspec archive <change-id> --yes [--skip-specs]
```

### Why `cflx openspec`

- 既存の `cflx` top-level command set を汚さず、OpenSpec utility 群としてまとまりを保てる
- skill 文書の移行時に「旧 helper の用途」を直接対応付けやすい
- proposal / workflow / run skill の instructions でも command 意図が読みやすい

## Responsibility Mapping

| Existing helper responsibility | Native destination |
| --- | --- |
| change/spec listing | `src/openspec.rs` + new command handlers |
| change detail / deltas-only rendering | native OpenSpec command output layer |
| strict validation / evidence mode | native validation module reusing proposal parsing and task parsing |
| spec promotion merge engine | Rust port of `skills/shared/cflx_spec_promotion.py` |
| archive helper command | `cflx openspec archive` wired to native promotion/archive flow |

## Migration Strategy

1. Add native `cflx openspec` CLI surface first.
2. Port validation and promotion logic until native behavior covers current skill needs.
3. Update skill source / references / README / active proposal guidance to the new CLI.
4. Remove embedded `scripts/cflx.py` auxiliary files and Python requirement language.

This should land as one coherent change so bundled skills are never distributed with docs that reference commands absent from the binary.

## Compatibility Notes

- The proposal only removes the bundled helper file; it does not remove Conflux skills themselves.
- Active repo guidance that still needs to be executable (for example in-progress proposals under `openspec/changes/`) should be migrated as part of this change.
- Archived historical proposals are explicitly out of scope unless a later cleanup change targets them.

## Verification Plan

- CLI parse/help tests for the new namespace and flags
- Focused native validation tests that mirror representative `cflx.py validate` behavior
- Focused archive/spec-promotion tests for ADDED/MODIFIED/REMOVED merges and no-op detection
- Embedded-skill / install-skills tests proving bundled distributions no longer include `scripts/cflx.py`
- Manual spot-check that active skill docs now point to `cflx openspec ...`
