# Change: proposal.md がない change を一覧から除外する

## Why
proposal.md が存在しない change が一覧に混在すると、変更内容の把握や依存関係解析の前提が崩れるため、一覧に含める条件を明確化します。

## What Changes
- change 一覧の生成で proposal.md が存在する change のみを対象にする
- proposal.md が存在しない change は一覧から除外する（archive 後の削除挙動は現状維持）

## Impact
- Affected specs: cli
- Affected code: src/openspec.rs, src/vcs/git/commands.rs, TUI/Web change list refresh
