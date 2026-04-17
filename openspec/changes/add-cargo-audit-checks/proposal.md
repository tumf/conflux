---
change_type: implementation
priority: medium
dependencies: []
references:
  - .github/workflows/ci.yml
  - Makefile
  - docs/guides/DEVELOPMENT.md
  - openspec/specs/release-workflow/spec.md
  - openspec/specs/documentation/spec.md
---
# Change: cargo audit を CI とローカル検証に追加

**Change Type**: implementation

## Premise / Context
- ユーザは `cargo audit` の導入を希望し、A 案として「CI + Makefile、pre-commit には入れない」を選択した。
- 現在の CI は dashboard build と `pre-commit run --all-files` を実行するが、依存脆弱性監査は実行していない。
- 現在の Makefile は `fmt` `lint` `test` `pre-commit` を `check` に含めているが、`audit` ターゲットは存在しない。
- 現在の開発ガイドは pre-commit / acceptance baseline を説明しているが、`cargo audit` の実行方法や pre-commit 非統合方針は記載していない。

## Why
既知の Rust advisory を CI と明示的なローカル総合チェックで検出できないため、脆弱な依存関係が main 系ブランチへ混入するまで気づけない状態です。`cargo audit` を標準検証フローへ追加し、依存脆弱性の検出を早めつつ、commit-time hook の体験は維持します。

## What Changes
- GitHub Actions の checks job に `cargo audit` 実行ステップを追加する
- Makefile に `audit` ターゲットを追加し、`check` に組み込む
- 開発ガイドに `make audit` / `cargo audit` の実行方法と pre-commit 非統合方針を記載する
- pre-commit / prek hook には `cargo audit` を追加しない方針を明文化する

## Acceptance Criteria
- CI が `cargo audit` を実行し、既知脆弱性がある場合は checks job が失敗する
- 開発者が `make audit` で依存監査を単独実行できる
- `make check` が `cargo audit` を含む総合チェックになる
- pre-commit / prek 実行では `cargo audit` が自動実行されない
- 開発ガイドに audit の実行方法と運用方針が反映される

## Out of Scope
- `cargo-deny` や `deny.toml` の導入
- advisory ignore リストや例外運用の追加
- 脆弱性解消のための依存バージョン更新

## Impact
- Affected specs: release-workflow, documentation
- Affected code: .github/workflows/ci.yml, Makefile, docs/guides/DEVELOPMENT.md
