# Change: CLI 仕様の言語を英語に統一

## Why

`openspec/specs/cli/spec.md` は確定仕様だが、一部が日本語、一部が英語で混在している。プロジェクトのルールでは `openspec/specs/` 配下は公開仕様として英語で統一すべきである。

## What Changes

- cli/spec.md 内の日本語で書かれた要件を英語に翻訳
- 翻訳対象:
  - `### Requirement: サブコマンド構造` → `### Requirement: Subcommand Structure`
  - `### Requirement: run サブコマンド` → `### Requirement: run Subcommand`
  - `### Requirement: デフォルトTUI起動` → `### Requirement: Default TUI Launch`
  - その他日本語シナリオの英訳

## Impact

- Affected specs: cli
- Affected code: なし（仕様ドキュメントのみ）
