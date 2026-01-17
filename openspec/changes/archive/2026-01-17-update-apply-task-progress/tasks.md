## 1. プロンプト強化
- [x] 1.1 applyのシステムプロンプトに、各タスク完了直後のtasks.md更新を必須化する指示を追加する
- [x] 1.2 applyの終了前にtasks.mdが実作業と一致していることを確認する指示を追加する
- [x] 1.3 タスクの分割・具体化を行った場合は同時にtasks.mdを更新する指示を追加する

## 2. 検証
- [x] 2.1 specの変更内容と合致するように文言を確認する
- [x] 2.2 npx @fission-ai/openspec@latest validate update-apply-task-progress --strict を実行する
