# Change: Fix server resolve_command prompt escaping during git sync

## Why
server mode の `POST /api/v1/projects/{id}/git/sync` で実行される `resolve_command` は、`src/server/api.rs` の独自 `{prompt}` 展開を使っているため、`"... '{prompt}' ..."` 形式の既存設定と multi-line prompt の組み合わせでコマンド文字列が壊れる。結果として shell が意図しない行を別コマンドとして解釈し、`resolve_command_failed` や exit code 127 を返す。

## What Changes
- server sync の `resolve_command` 実行経路を、他の agent command と同じ共通 placeholder 展開ルールに揃える
- `'{prompt}'` と `{prompt}` の両テンプレート形式を server mode でも互換サポートする
- multi-line prompt を含む `resolve_command` 実行の回帰テストを追加する
- server mode の `resolve_command` 仕様を shell-escaping / server-mode spec に明文化する

## Acceptance Criteria
- `resolve_command` が `opencode run --agent code --model kani/kani/deep '{prompt}'` の形式でも、`git/sync` 実行時にコマンド文字列が壊れず実行される
- `resolve_command` が `echo {prompt}` のようなクォートなしテンプレートでも従来どおり動作する
- multi-line prompt を含む `resolve_command` 実行が server mode のテストで検証される
- `git/sync` の `resolve_command` 展開仕様が既存の shell-escaping spec と矛盾しない

## Out of Scope
- `resolve_command` を shell 非経由の argv 実行へ全面移行すること
- launchd / PATH 設定そのものの改善

## Impact
- Affected specs: `shell-escaping`, `server-mode`
- Affected code: `src/server/api.rs`, `src/config/expand.rs` (利用のみの可能性), `src/shell_command.rs` (影響確認), server tests
