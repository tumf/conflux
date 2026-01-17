## 1. 変更一覧の proposal.md フィルタリング
- [x] 1.1 list_changes_native の一覧生成で proposal.md の存在確認を追加する
- [x] 1.2 list_changes_in_head の対象判定で proposal.md の存在確認を追加する
- [x] 1.3 TUI/WEB の変更一覧が proposal.md 欠落 change を除外することを確認する

## 2. 仕様とテストの更新
- [x] 2.1 cli spec に proposal.md 必須化の変更を追記する
- [x] 2.2 追加・更新したテストを整理する
- [x] 2.3 npx @fission-ai/openspec@latest validate update-change-list-proposal-filter --strict を実行する
