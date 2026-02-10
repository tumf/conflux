## 1. Implementation
- [ ] 1.1 Git の未追跡ファイル判定前に `.git/info/exclude` を読み取り、`openspec/changes/*/approved` が無い場合は 1 行追加する（重複は追加しない）。完了確認: 追加前後で同一ファイルを読み取り、該当行が 1 回だけ存在することをテストで確認する。
- [ ] 1.2 未追跡ファイルの除外判定で `.gitignore` と `.git/info/exclude` の両方を反映する。完了確認: テスト用のリポジトリで `openspec/changes/*/approved` が未追跡に含まれないことを確認する。
- [ ] 1.3 追加・除外判定の単体テストを追加し、`cargo test` が成功することを確認する。
