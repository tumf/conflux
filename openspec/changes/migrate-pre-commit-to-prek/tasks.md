## 1. Documentation
- [ ] 1.1 README.md の Git Hooks セクションを prek 前提に更新し、`pre-commit uninstall` → `prek install` の移行手順と利用例を追加する。完了確認: `README.md` に `prek install`/`prek run` があり、`pre-commit install` が無いことを確認する。
- [ ] 1.2 README.ja.md に Git Hooks セクションを追加/更新し、README.md と同一のコマンド例に揃える。完了確認: `README.ja.md` の Git Hooks セクションでコマンド例が一致していることを確認する。
- [ ] 1.3 DEVELOPMENT.md の「Pre-commit checks」とフック導入手順を prek ベースに置換し、`prek run --all-files` と `make openapi` の自動ステージングを説明する。完了確認: `DEVELOPMENT.md` に `pre-commit install` が無く、prek 手順が記載されていることを確認する。

## 2. Validation
- [ ] 2.1 ドキュメント内に旧手順が残っていないことを検索で確認する。完了確認: `rg -n "pre-commit install|pre-commit run" README.md README.ja.md DEVELOPMENT.md` が該当なしであることを確認する。
