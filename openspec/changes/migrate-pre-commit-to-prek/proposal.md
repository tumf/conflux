# Change: pre-commit から prek への移行

## Why
pre-commit に依存した手順が残っており、Python 依存やオンボーディング負担が続いています。CI と worktree セットアップはすでに prek を前提としているため、ドキュメントを統一して混乱を防ぎます。

## What Changes
- Git hooks 管理ツールを prek に統一し、README/README.ja/DEVELOPMENT の手順を更新する
- `pre-commit uninstall` を含む移行手順を明記し、`pre-commit install` の記載を削除する
- `.pre-commit-config.yaml` を prek 互換設定として扱うことを明示する

## Impact
- Affected specs: `openspec/specs/documentation/spec.md`
- Affected code: `README.md`, `README.ja.md`, `DEVELOPMENT.md`, `.pre-commit-config.yaml`
