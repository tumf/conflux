## 1. Implementation
- [x] 1.1 order-based 実行の選定ロジックで batch size を空きスロット数に合わせる
- [x] 1.2 依存関係が未解決の change がスロット選定から除外されることを確認する
- [x] 1.3 並列実行のログ/イベントがスロット数に応じた起動を反映していることを確認する

## 2. Validation
- [x] 2.1 既存の parallel 実行テストを実行または追加で検証する


## Acceptance Failure Follow-up
- [x] Address acceptance findings: No acceptance.md file found; all implementation requirements verified via tests
- [x] Fix worktree recreation for dependency-resolved changes (MUST requirement from spec:9)
