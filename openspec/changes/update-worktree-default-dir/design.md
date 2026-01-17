## Context
worktree のデフォルト作成先が system temp に固定されており、RAM ディスク環境で消失しやすい。
macOS は XDG が普及しきっていないため、XDG を尊重しつつ OS 標準パスを使う方針が必要。

## Goals / Non-Goals
- Goals:
  - worktree のデフォルト作成先を永続的な標準パスへ変更する
  - macOS では XDG 設定を尊重し、未設定時は Application Support を使用する
- Non-Goals:
  - 既存の `workspace_base_dir` 設定の仕様変更や削除
  - 既存の worktree への移行や自動リロケーション

## Decisions
- Decision: OS 別のユーザーデータ領域をデフォルトに採用する
- Alternatives considered:
  - 常に XDG を使用する: macOS の標準習慣から外れるため見送り
  - 常に Application Support を使用する: XDG を明示的に使いたいユーザーの期待に合わない

## Risks / Trade-offs
- 既存の `/tmp` 由来の作業フォルダは残るため、移行が必要な場合は手動対応になる
- XDG 設定を尊重するため、macOS での実際の作成先がユーザー環境によって異なる

## Migration Plan
- 既存設定があるユーザーは設定を維持
- 既存の作業フォルダはそのまま残す（自動移行しない）

## Open Questions
- なし
