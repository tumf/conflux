# 変更提案: project add のブランチ表記とデフォルトブランチ解決

## Why（背景）
`cflx project add` では URL とブランチを別指定する必要があり、GitHub の URL 形式（`/tree/<branch>` や `#<branch>`）がそのまま使えない。ブランチ省略時にデフォルトブランチを自動解決できれば、手入力が減り運用が簡単になる。

## What Changes（変更内容）
- `cflx project add` で `https://github.com/org/repo/tree/branch` と `https://github.com/org/repo#branch` を受け入れる。
- ブランチが省略された場合はリモートのデフォルトブランチを解決して使用する。
- URL から抽出したブランチと引数のブランチが競合する場合は、引数指定を優先する。

## Impact（影響範囲）
- Affected specs: `openspec/specs/cli/spec.md`
- Affected code: `src/cli.rs`, `src/main.rs`, `src/remote/client.rs`
