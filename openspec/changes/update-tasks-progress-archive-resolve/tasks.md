## 1. Implementation
- [ ] 1.1 TUIの自動更新でarchive/resolving時のtasks進捗上書き条件を整理する
- [ ] 1.2 WebStateの自動更新/イベント更新でtasks進捗上書き条件を整理する
- [ ] 1.3 worktree/archived fallbackの進捗反映条件を定義し直す
- [ ] 1.4 TUI/Webの回帰テストを追加する（0/0上書き防止）

## 2. Validation
- [ ] 2.1 `cargo test`
- [ ] 2.2 `npx @fission-ai/openspec@latest validate update-tasks-progress-archive-resolve --strict`
