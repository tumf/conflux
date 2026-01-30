# Change: Archive ループの分割

## Why
`execute_archive_loop` が長く、フック実行・コマンド実行・検証・履歴記録が混在しています。責務分割により見通しと安全性を向上させます。

## What Changes
- archive ループ内のフェーズ（フック／実行／検証／履歴）をヘルパー関数に抽出する
- 既存のリトライ・履歴・検証の挙動を維持する

## Impact
- Affected specs: `code-maintenance`
- Affected code: `src/execution/archive.rs`
