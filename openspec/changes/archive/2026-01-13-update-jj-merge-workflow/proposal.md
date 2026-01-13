# Change: jj 並列マージの作業コピー運用を通常フローに合わせる

## Why
jj 並列マージ後に追加の作業コピーコミットが作成されるため、履歴上で分岐に見える状態が発生している。通常の `jj new` 運用に合わせて、マージ結果がそのまま作業コピーになる挙動に揃える。

## What Changes
- jj 並列マージ後に追加の working copy コミットを作成しないようにする
- `jj new --no-edit` によるマージ結果をそのまま作業コピーとして保持する
- 並列マージ後の `@` 位置がマージコミットに留まることを保証する

## Impact
- Affected specs: `openspec/specs/parallel-execution/spec.md`
- Affected code: `src/vcs/jj/mod.rs`, `src/parallel/executor.rs`
